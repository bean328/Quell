//! Types (spec/grammar.md, section 7), generic parameters (3.4), and the speculative reading
//! of generic arguments in expressions (6.11).

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::fixes::Case;
use super::items::with_space_after;
use super::{Done, NESTING_LIMIT, Parser, codes};
use crate::SyntaxKind::{self, *};

/// Whether `mut` may start a type here (7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MutRule {
    /// A function parameter's type: `counter: mut Counter`.
    Allowed,
    /// Anywhere else.
    Forbidden,
    /// A lambda parameter's type, whose `mut` goes before the name (6.18).
    Lambda { name: Span },
}

/// `Type = TypeAtom [ "?" ]`, and `ParamType = [ "mut" ] Type` when `mut` is allowed.
pub(crate) fn type_(p: &mut Parser<'_>, rule: MutRule) -> Option<Done> {
    if !p.enter() {
        return None;
    }
    let result = type_inner(p, rule);
    p.leave();
    result
}

fn type_inner(p: &mut Parser<'_>, rule: MutRule) -> Option<Done> {
    if p.at(MUT_KW) {
        let mut_span = p.nth_span(0);
        if rule == MutRule::Allowed {
            let marker = p.start();
            p.bump();
            type_(p, MutRule::Forbidden);
            return Some(p.complete(marker, MUT_TYPE));
        }
        misplaced_mut(p, mut_span, rule);
        p.bump_error();
    }
    let atom = type_atom(p)?;
    let ty = if p.at(QUESTION) {
        let marker = atom.precede(p);
        p.bump();
        let done = p.complete(marker, OPTIONAL_TYPE);
        if matches!(p.current(), QUESTION | QUESTION_QUESTION) && !p.nth_line_break_before(0) {
            // Remove the spaces before the second `?` too.
            let second = p.nth_span(0);
            let removal = Span::new(p.last_end(), second.end);
            optional_twice(p, second, removal, "");
            p.bump_error();
        }
        done
    } else if p.at(QUESTION_QUESTION) && !p.nth_line_break_before(0) {
        // `String??` lexes as `??` (7.1).
        let marker = atom.precede(p);
        let span = p.nth_span(0);
        optional_twice(p, span, span, "?");
        p.bump();
        p.complete(marker, OPTIONAL_TYPE)
    } else {
        atom
    };
    Some(ty)
}

#[cold]
fn misplaced_mut(p: &mut Parser<'_>, mut_span: Span, rule: MutRule) {
    if p.speculating() {
        return;
    }
    let removal = with_space_after(p, mut_span);
    let diagnostic = match rule {
        MutRule::Lambda { name } => {
            let name_text = p.text_of(name).to_string();
            Diagnostic::error("in a lambda, `mut` goes before the parameter's name")
                .with_code(codes::MUT_IN_LAMBDA_TYPE)
                .primary(mut_span, "`mut` in a lambda parameter's type")
                .note("function parameters write `list: mut List<Int>`, lambdas write `(mut list: List<Int>) =>`")
                .suggest_edits(
                    format!("write `mut {name_text}: ...`"),
                    vec![
                        Edit { span: Span::at(name.start), replacement: "mut ".into() },
                        Edit { span: removal, replacement: String::new() },
                    ],
                )
        }
        _ => Diagnostic::error("`mut` is only for function parameters")
            .with_code(codes::MUT_IN_TYPE)
            .primary(mut_span, "`mut` in a type")
            .note("`mut` marks a parameter that the function may change, as in `counter: mut Counter`")
            .suggest("remove `mut`", removal, ""),
    };
    p.push_error(diagnostic);
}

#[cold]
fn optional_twice(p: &mut Parser<'_>, span: Span, removal: Span, replacement: &str) {
    if p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error("a type can only be optional once")
            .with_code(codes::OPTIONAL_TWICE)
            .primary(span, "a second `?`")
            .note("a value is either there or missing; a second `?` adds nothing")
            .suggest("write one `?`", removal, replacement.to_string()),
    );
}

/// `TypeAtom = TypePath | "any" TypePath | "type" | ParenType`. `any` and `type` are
/// contextual keywords, decided by the next token (7.1).
fn type_atom(p: &mut Parser<'_>) -> Option<Done> {
    let kind = p.current();
    if kind.is_ident() {
        let text = p.nth_text(0);
        if text == "any" && p.nth(1).is_ident() {
            let marker = p.start();
            p.bump();
            type_path(p);
            return Some(p.complete(marker, ANY_TYPE));
        }
        if text == "type" && !matches!(p.nth(1), DOT | LT) {
            let marker = p.start();
            p.bump();
            return Some(p.complete(marker, TYPE_TYPE));
        }
        return Some(type_path(p));
    }
    if kind == L_PAREN {
        return Some(paren_type(p));
    }
    p.error_expected(
        codes::EXPECTED_TYPE,
        "a type",
        "write a type, such as `Int` or `List<String>`",
    );
    None
}

/// `TypePath = IDENT { "." IDENT } [ GenericArgs ]`. In a type, `<` always starts generic
/// arguments.
pub(crate) fn type_path(p: &mut Parser<'_>) -> Done {
    let marker = p.start();
    p.bump();
    loop {
        if p.at(DOT) && p.nth(1).is_ident() {
            p.bump();
            p.bump();
        } else if p.at(COLON) && p.nth(1) == COLON && p.nth_is_joined(1) && p.nth(2).is_ident() {
            super::exprs::double_colon(p);
            p.bump();
        } else {
            break;
        }
    }
    if p.at(LT) && p.split_is_zero() {
        generic_arg_list(p);
    }
    p.complete(marker, PATH_TYPE)
}

/// `ParenType = "(" [ Type { "," Type } [ "," ] ] ")" [ "->" Type ]`.
fn paren_type(p: &mut Parser<'_>) -> Done {
    let marker = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    let (count, trailing) =
        super::exprs::delimited(p, open, "type", |p| type_(p, MutRule::Forbidden).is_some());
    let close_span = p.nth_span(0);
    p.close_bracket(open, "the parentheses");
    if p.at(THIN_ARROW) {
        p.bump();
        type_(p, MutRule::Forbidden);
        return p.complete(marker, FN_TYPE);
    }
    let kind = match (count, trailing) {
        (0, _) => {
            if p.partner_of(open).is_some() {
                super::exprs::unit_or_one_tuple(p, open_span.to(close_span), None, "a type");
            }
            TUPLE_TYPE
        }
        (1, Some(comma)) => {
            super::exprs::unit_or_one_tuple(p, comma, Some(comma), "a type");
            PAREN_TYPE
        }
        (1, None) => PAREN_TYPE,
        _ => TUPLE_TYPE,
    };
    p.complete(marker, kind)
}

/// `GenericArgs = "<" Type { "," Type } [ "," ] ">"`. Returns the first and last argument.
pub(crate) fn generic_arg_list(p: &mut Parser<'_>) -> (Option<Done>, Option<Done>, usize) {
    let marker = p.start();
    p.bump(); // `<`
    let mut first = None;
    let mut last = None;
    let mut count = 0;
    loop {
        if p.at_closing_angle() {
            break;
        }
        let start = p.pos();
        let Some(ty) = type_(p, MutRule::Forbidden) else { break };
        count += 1;
        first.get_or_insert(ty);
        last = Some(ty);
        if p.eat(COMMA) {
            continue;
        }
        if p.at_closing_angle() || p.pos() == start {
            break;
        }
        break;
    }
    if count == 0 && !p.already_reported_here() {
        p.error_expected(
            codes::EXPECTED_TYPE,
            "a type",
            "write the type arguments between `<` and `>`",
        );
    }
    close_angle(p);
    p.complete(marker, GENERIC_ARG_LIST);
    (first, last, count)
}

/// The `>` that closes generic arguments or parameters (7.3).
fn close_angle(p: &mut Parser<'_>) {
    if p.at_closing_angle() {
        p.bump_closing_angle();
    } else if let Some(diagnostic) =
        p.expected(codes::EXPECTED_TOKEN, "`>` to close the generic arguments")
    {
        let at = Span::at(p.last_end());
        p.push_error(diagnostic.suggest("add `>`", at, ">"));
    }
}

/// `GenericParams = "<" GenericParam { "," GenericParam } [ "," ] [ NEWLINE ] ">"` with
/// `GenericParam = UPPER_IDENT [ ":" Requirement { "and" Requirement } ]` (3.4).
pub(crate) fn generic_param_list(p: &mut Parser<'_>) {
    let marker = p.start();
    p.bump(); // `<`
    let mut count = 0;
    loop {
        if p.at_closing_angle() {
            break;
        }
        if p.at(NEWLINE) && matches!(p.nth(1), GT | GT_GT | GT_EQ | GT_GT_EQ) {
            p.bump();
            break;
        }
        if !p.current().is_ident() {
            if !p.already_reported_here() {
                p.error_expected(
                    codes::EXPECTED_NAME,
                    "a generic parameter name",
                    "generic parameters are written like `<T>` or `<T: Compare>`",
                );
            }
            break;
        }
        let param = p.start();
        let name = p.nth_span(0);
        p.check_name(name, Case::Upper, "generic parameter names");
        p.bump();
        if p.eat(COLON) {
            loop {
                if type_(p, MutRule::Forbidden).is_none() {
                    break;
                }
                if !p.eat(AND_KW) {
                    break;
                }
            }
        }
        p.complete(param, GENERIC_PARAM);
        count += 1;
        if p.eat(COMMA) {
            continue;
        }
        if p.at_closing_angle()
            || (p.at(NEWLINE) && matches!(p.nth(1), GT | GT_GT | GT_EQ | GT_GT_EQ))
        {
            continue;
        }
        if p.current().is_ident() {
            super::exprs::missing_comma(p);
            continue;
        }
        break;
    }
    if count == 0 && !p.already_reported_here() {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a generic parameter name",
            "write the parameters between `<` and `>`, as in `<T>`",
        );
    }
    close_angle(p);
    p.complete(marker, GENERIC_PARAM_LIST);
}

// ----------------------------------------------------------------------
// Generic arguments in expressions (6.11)
// ----------------------------------------------------------------------

/// At a `<` right after a name in an expression (or after the member name that follows `.` or
/// `?.`), reads generic arguments if the three conditions of 6.11 hold, and returns whether it
/// did. Otherwise nothing is consumed and the `<` is less-than.
pub(crate) fn try_generic_args(p: &mut Parser<'_>) -> bool {
    if !p.at(LT) || !p.split_is_zero() {
        return false;
    }
    let lt = p.pos();
    // Conditions 1 and 2: a complete `GenericArgs` whose closing `>` uses up its token.
    let (end, split) = match p.scan_generic_args(lt, 0) {
        Ok(end) => end,
        Err(Scan::NoMatch) => return false,
        Err(Scan::TooDeep) => {
            // Too deep to tell: report it once and skip the rest.
            p.enter_beyond_limit();
            return true;
        }
    };
    if split != 0 {
        return false;
    }
    // Condition 3: what follows is `(`, `{`, or a token that can't start an expression.
    let next = p.kind_at(end);
    if !(next == L_PAREN || next == L_BRACE || !super::exprs::can_start_expr(next)) {
        return false;
    }
    let over_lines = (lt + 1..end).any(|pos| p.line_break_before_at(pos));
    let name = p.last_token_span();
    let (first, last, count) = generic_arg_list(p);
    if over_lines {
        generic_arguments_over_lines(p, name, first, last, count);
    }
    true
}

#[cold]
fn generic_arguments_over_lines(
    p: &mut Parser<'_>,
    name: Span,
    first: Option<Done>,
    last: Option<Done>,
    count: usize,
) {
    if p.speculating() {
        return;
    }
    let name_text = p.text_of(name).to_string();
    let mut diagnostic = Diagnostic::warning(format!(
        "this reads as generic arguments of `{name_text}`, not as comparisons"
    ))
    .with_code(codes::GENERIC_ARGUMENTS_OVER_LINES)
    .primary(name, "`<` and the `>` that closes it are on different lines")
    .note("a `<` followed by types, a comma and a `>` is read as generic arguments");
    let call_end =
        (p.at(L_PAREN)).then(|| p.nth_partner(0)).flatten().map(|close| p.span_at(close).end);
    match (first, last, call_end) {
        (Some(first), Some(last), Some(call_end)) if count == 2 => {
            diagnostic = diagnostic.suggest_edits(
                "to compare, wrap each comparison in parentheses",
                vec![
                    Edit { span: Span::at(name.start), replacement: "(".into() },
                    Edit { span: Span::at(first.end), replacement: ")".into() },
                    Edit { span: Span::at(last.start), replacement: "(".into() },
                    Edit { span: Span::at(call_end), replacement: ")".into() },
                ],
            );
        }
        _ => {
            diagnostic = diagnostic.help("to compare, wrap each comparison in parentheses");
        }
    }
    p.push_error(diagnostic);
}

/// A position in the scan: a token and how many bytes of it are used (for split `>>`).
type ScanPos = (usize, u32);

/// Why a scan found no generic arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scan {
    /// The tokens are not generic arguments.
    NoMatch,
    /// The types are nested too deeply to tell.
    TooDeep,
}

type ScanResult = Result<ScanPos, Scan>;

impl Parser<'_> {
    /// The kind at a scan position, with a split `>>`, `>=` or `>>=` read as what is left.
    fn scan_kind(&self, (pos, split): ScanPos) -> SyntaxKind {
        let kind = self.kind_at(pos);
        match (kind, split) {
            (_, 0) => kind,
            (GT_GT, 1) => GT,
            (GT_EQ, 1) | (GT_GT_EQ, 2) => EQ,
            (GT_GT_EQ, 1) => GT_EQ,
            _ => kind,
        }
    }

    /// Where `GenericArgs` starting at the `<` at `lt` end, if they parse, using the type
    /// grammar only, with no side effects. The result for each `<` is remembered, so input
    /// like `a < (b < (c < d))` costs linear time (6.11).
    pub(crate) fn scan_generic_args(&mut self, lt: usize, depth: u32) -> ScanResult {
        if let Some(&result) = self.generic_memo.get(&lt) {
            return result.ok_or(Scan::NoMatch);
        }
        let result = self.scan_generic_args_uncached(lt, depth);
        match result {
            Ok(end) => {
                self.generic_memo.insert(lt, Some(end));
            }
            Err(Scan::NoMatch) => {
                self.generic_memo.insert(lt, None);
            }
            // Not a property of the tokens alone: not remembered.
            Err(Scan::TooDeep) => {}
        }
        result
    }

    fn scan_generic_args_uncached(&mut self, lt: usize, depth: u32) -> ScanResult {
        if depth >= NESTING_LIMIT {
            return Err(Scan::TooDeep);
        }
        let mut at = (lt + 1, 0);
        loop {
            at = self.scan_type(at, depth + 1)?;
            match self.scan_kind(at) {
                COMMA => {
                    at = (at.0 + 1, 0);
                    if is_closing_angle(self.scan_kind(at)) {
                        return Ok(self.scan_close(at));
                    }
                }
                kind if is_closing_angle(kind) => return Ok(self.scan_close(at)),
                _ => return Err(Scan::NoMatch),
            }
        }
    }

    /// Uses one `>` at `at`.
    fn scan_close(&self, (pos, split): ScanPos) -> ScanPos {
        let tok = self.span_at_raw(pos);
        if tok.start + split + 1 >= tok.end { (pos + 1, 0) } else { (pos, split + 1) }
    }

    fn scan_type(&mut self, at: ScanPos, depth: u32) -> ScanResult {
        if depth >= NESTING_LIMIT {
            return Err(Scan::TooDeep);
        }
        let mut at = self.scan_type_atom(at, depth)?;
        if self.scan_kind(at) == QUESTION {
            at = (at.0 + 1, 0);
            if matches!(self.scan_kind(at), QUESTION | QUESTION_QUESTION) {
                return Err(Scan::NoMatch);
            }
        }
        Ok(at)
    }

    fn scan_type_atom(&mut self, at: ScanPos, depth: u32) -> ScanResult {
        if at.1 != 0 {
            return Err(Scan::NoMatch);
        }
        let kind = self.scan_kind(at);
        if kind.is_ident() {
            let text = self.text_of(self.span_at_raw(at.0));
            let next = self.kind_at(at.0 + 1);
            if text == "any" && next.is_ident() {
                return self.scan_type_path((at.0 + 1, 0), depth);
            }
            if text == "type" && !matches!(next, DOT | LT) {
                return Ok((at.0 + 1, 0));
            }
            return self.scan_type_path(at, depth);
        }
        if kind != L_PAREN {
            return Err(Scan::NoMatch);
        }
        // `(` [ Type { "," Type } [ "," ] ] `)` [ `->` Type ]
        let mut at = (at.0 + 1, 0);
        let mut count = 0;
        let mut trailing = false;
        while self.scan_kind(at) != R_PAREN {
            at = self.scan_type(at, depth + 1)?;
            count += 1;
            trailing = false;
            match self.scan_kind(at) {
                COMMA => {
                    at = (at.0 + 1, 0);
                    trailing = true;
                }
                R_PAREN => {}
                _ => return Err(Scan::NoMatch),
            }
        }
        at = (at.0 + 1, 0);
        if self.scan_kind(at) == THIN_ARROW {
            return self.scan_type((at.0 + 1, 0), depth + 1);
        }
        // `()` and `(Int,)` are not types.
        if count == 0 || (count == 1 && trailing) {
            return Err(Scan::NoMatch);
        }
        Ok(at)
    }

    fn scan_type_path(&mut self, at: ScanPos, depth: u32) -> ScanResult {
        let mut at = (at.0 + 1, 0);
        while self.scan_kind(at) == DOT && self.kind_at(at.0 + 1).is_ident() {
            at = (at.0 + 2, 0);
        }
        if self.scan_kind(at) == LT {
            return self.scan_generic_args(at.0, depth + 1);
        }
        Ok(at)
    }
}

fn is_closing_angle(kind: SyntaxKind) -> bool {
    matches!(kind, GT | GT_GT | GT_EQ | GT_GT_EQ)
}
