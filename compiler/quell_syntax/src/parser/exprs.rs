//! Expressions (spec/grammar.md, sections 5 and 6), with Pratt parsing for the operators.
//!
//! Binding powers follow the precedence table of section 5: level `n` has binding power
//! `18 - n`, so tighter operators have higher numbers. The special cases listed after the table
//! are marked "special case" below.

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::fixes::{Case, wrap_in_parens};
use super::items::with_space_after;
use super::stmts;
use super::types::{self, MutRule};
use super::{Bracket, Ctx, Done, Parser, codes};
use crate::SyntaxKind::{self, *};

// Binding powers.
const BP_OR: u8 = 2;
const BP_AND: u8 = 3;
/// The operand of `not`: comparisons and everything tighter.
const BP_NOT: u8 = 4;
const BP_COMPARE: u8 = 5;
const BP_IS: u8 = 6;
const BP_CATCH: u8 = 7;
const BP_COALESCE: u8 = 8;
const BP_RANGE: u8 = 9;
/// The operand of a prefix operator.
const BP_PREFIX: u8 = 16;

/// The left binding power of a binary operator.
fn infix_bp(kind: SyntaxKind) -> Option<u8> {
    Some(match kind {
        OR_KW => BP_OR,
        AND_KW => BP_AND,
        EQ_EQ | BANG_EQ | LT | LT_EQ | GT | GT_EQ => BP_COMPARE,
        IS_KW => BP_IS,
        CATCH_KW => BP_CATCH,
        QUESTION_QUESTION => BP_COALESCE,
        DOT_DOT_LT | DOT_DOT_EQ => BP_RANGE,
        PIPE => 10,
        CARET => 11,
        AMP => 12,
        LT_LT | GT_GT => 13,
        PLUS | MINUS => 14,
        STAR | SLASH | DIV_KW | PERCENT => 15,
        _ => return None,
    })
}

/// Whether `kind` is a binary operator of levels 3 to 9 (`*` to `..=`): not allowed at the top
/// of a fallback (grammar.md, 5).
fn is_arithmetic(kind: SyntaxKind) -> bool {
    infix_bp(kind).is_some_and(|bp| bp >= BP_RANGE)
}

/// The tokens that can start an expression (grammar.md, 6.11).
pub(crate) fn can_start_expr(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        UPPER_IDENT
            | LOWER_IDENT
            | INT
            | FLOAT
            | CHAR
            | STRING_START
            | UNDERSCORE
            | L_PAREN
            | L_BRACK
            | L_BRACE
            | MINUS
            | TILDE
            | SELF_KW
            | TRUE_KW
            | FALSE_KW
            | NONE_KW
            | NOT_KW
            | TRY_KW
            | SPAWN_KW
            | MUT_KW
            | IF_KW
            | MATCH_KW
            | GROUP_KW
            | UNSAFE_KW
            | RETURN_KW
            | THROW_KW
            | BREAK_KW
            | CONTINUE_KW
    )
}

fn is_jump(kind: SyntaxKind) -> bool {
    matches!(kind, RETURN_KW | THROW_KW | BREAK_KW | CONTINUE_KW)
}

/// The assignment operators (4.4).
fn is_assign_op(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        EQ | PLUS_EQ
            | MINUS_EQ
            | STAR_EQ
            | SLASH_EQ
            | PERCENT_EQ
            | AMP_EQ
            | PIPE_EQ
            | CARET_EQ
            | LT_LT_EQ
            | GT_GT_EQ
    )
}

pub(crate) fn at_assign_op(p: &Parser<'_>) -> bool {
    is_assign_op(p.current())
}

/// What the parser knows about an expression it read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Expr {
    pub(crate) done: Done,
    /// The expression is a place (4.4): a variable, or a field or element reached from one.
    pub(crate) place: bool,
    /// The expression ends with the binding form of `catch`: where the name is (10.4).
    pub(crate) catch_name: Option<Span>,
}

impl Expr {
    fn new(done: Done) -> Expr {
        Expr { done, place: false, catch_name: None }
    }
}

/// Which operand an expression is, for the warning about a line that starts with `.`
/// (grammar.md, 10.17).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// Anything else: a plain chain never gets the warning.
    Plain,
    /// The right operand of a binary operator. `skip_number` is set when the operand is a
    /// number literal followed by `.`, which has its own warning.
    Rhs { skip_number: bool },
    /// The operand of a prefix `-` or `~`.
    Prefix { skip_number: bool },
}

/// How an assignment right after an expression is reported (4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterValue {
    /// A value: `let a = b = 1` is an error.
    Value,
    /// The body of a `match` arm or a lambda: the assignment is wrapped in a block.
    Body,
}

// ----------------------------------------------------------------------
// Entry points
// ----------------------------------------------------------------------

/// `Expr`, where an expression is a value: an assignment after it is an error.
pub(crate) fn expr(p: &mut Parser<'_>, ctx: Ctx) -> Option<Expr> {
    expr_after(p, ctx, AfterValue::Value)
}

/// `Expr` as the body of a `match` arm or a lambda.
fn body_expr(p: &mut Parser<'_>, ctx: Ctx) -> Option<Expr> {
    expr_after(p, ctx, AfterValue::Body)
}

fn expr_after(p: &mut Parser<'_>, ctx: Ctx, after: AfterValue) -> Option<Expr> {
    let value = full_expr(p, ctx);
    if at_assign_op(p) {
        if let Some(value) = value {
            return Some(misplaced_assignment(p, value, after));
        }
    }
    value
}

/// `Expr = LambdaExpr | JumpExpr | OrExpr` (6.1), with no check of what follows.
pub(crate) fn full_expr(p: &mut Parser<'_>, ctx: Ctx) -> Option<Expr> {
    let saved = p.expr_start;
    p.expr_start = p.nth_span(0).start;
    let result = if at_lambda(p) {
        lambda(p)
    } else if is_jump(p.current()) {
        Some(jump(p, ctx, JumpPos::Full).expr)
    } else {
        expr_bp(p, 0, ctx, Role::Plain)
    };
    p.expr_start = saved;
    result
}

/// `HeaderExpr = OrExpr` (6.1): the expression right before a body block. `cond` is set for
/// the condition of `if` and `while`, where `=` is reported (4.11).
pub(crate) fn header_expr(p: &mut Parser<'_>, cond: bool) -> Option<Expr> {
    let saved = p.expr_start;
    p.expr_start = p.nth_span(0).start;
    let ctx = Ctx { header: true, cond, guard: false };
    let result = expr_bp(p, 0, ctx, Role::Plain);
    p.expr_start = saved;
    if result.is_none() && !p.already_reported_here() {
        p.error_expected(
            codes::EXPECTED_EXPRESSION,
            "an expression",
            "write the expression before the `{`",
        );
    }
    result
}

/// Whether a lambda starts here (6.1): `x =>`, `mut x =>`, or `(` whose matching `)` is
/// followed by `=>`.
pub(crate) fn at_lambda(p: &Parser<'_>) -> bool {
    match p.current() {
        UPPER_IDENT | LOWER_IDENT | UNDERSCORE => p.nth(1) == FAT_ARROW,
        MUT_KW => {
            matches!(p.nth(1), UPPER_IDENT | LOWER_IDENT | UNDERSCORE) && p.nth(2) == FAT_ARROW
        }
        L_PAREN => p.nth_partner(0).is_some_and(|close| p.kind_at(close + 1) == FAT_ARROW),
        _ => false,
    }
}

// ----------------------------------------------------------------------
// Pratt parsing
// ----------------------------------------------------------------------

/// An expression whose operators all bind at least as tightly as `min_bp`.
pub(crate) fn expr_bp(p: &mut Parser<'_>, min_bp: u8, ctx: Ctx, role: Role) -> Option<Expr> {
    if !p.enter() {
        return None;
    }
    let result = expr_bp_inner(p, min_bp, ctx, role);
    p.leave();
    result
}

fn expr_bp_inner(p: &mut Parser<'_>, min_bp: u8, ctx: Ctx, role: Role) -> Option<Expr> {
    let mut lhs = operand(p, min_bp, ctx, role)?;
    let depth = p.depth();
    // The last non-associative operator in this chain (levels 9, 12 and 13), with the right
    // operand it had, for special case 1.
    let mut non_assoc: Option<(u8, Span)> = None;
    // One error for a whole chain such as `a < b < c < d`.
    let mut chain_reported = false;
    loop {
        let mut kind = p.current();
        if kind == QUESTION {
            if min_bp == 0 && ternary_ahead(p) {
                lhs = ternary(p, lhs);
                continue;
            }
            break;
        }
        if kind == EQ && ctx.cond && min_bp <= BP_COMPARE {
            // `if x = 5` (4.11): read as `==`.
            assignment_in_condition(p);
            kind = EQ_EQ;
        }
        if matches!(kind, PLUS | MINUS) && at_increment(p) {
            lhs = increment(p, lhs);
            continue;
        }
        if kind == QUESTION_QUESTION && p.nth(1) == EQ {
            break; // `??=`, reported by the statement
        }
        let Some(bp) = infix_bp(kind) else { break };
        if bp < min_bp {
            break;
        }
        if kind == MINUS && p.nth_line_break_before(0) && !p.at_soft_break() && forgotten_comma(p) {
            break;
        }
        // Special case 1: a second non-associative operator of the same level.
        if let Some((level, right)) = non_assoc {
            if level == bp && !chain_reported {
                chained_non_associative(p, &lhs, right, bp);
                chain_reported = true;
            }
        }
        if !p.enter() {
            break;
        }
        let mut right = None;
        lhs = match kind {
            CATCH_KW => catch_rest(p, lhs, ctx),
            QUESTION_QUESTION => coalesce_rest(p, lhs, ctx),
            IS_KW => is_rest(p, lhs),
            _ => {
                let (whole, rhs) = binary_rest(p, lhs, kind, bp, ctx);
                right = rhs;
                whole
            }
        };
        non_assoc = match bp {
            BP_COMPARE | BP_IS | BP_RANGE => Some((bp, right.unwrap_or(lhs.done.span()))),
            _ => None,
        };
        if lhs.catch_name.is_some() && lhs.done.kind == CATCH_EXPR {
            // Special case 5: the binding form of `catch` ends its operand.
            lhs = after_binding_catch(p, lhs, ctx);
        }
    }
    p.set_depth(depth);
    Some(lhs)
}

/// A binary operator and its right operand. Returns the new node and the right operand's span.
fn binary_rest(
    p: &mut Parser<'_>,
    lhs: Expr,
    kind: SyntaxKind,
    bp: u8,
    ctx: Ctx,
) -> (Expr, Option<Span>) {
    let marker = lhs.done.precede(p);
    let range = matches!(kind, DOT_DOT_LT | DOT_DOT_EQ);
    p.bump();
    if matches!(kind, GT | GT_GT) && p.at(NEWLINE) {
        // lexical.md, 12.5: a line that ends with a comparison or shift continues.
        p.bump();
    }
    // A range whose right end is a number literal followed by `.` (6.7).
    let literal =
        range && matches!(p.current(), INT | FLOAT) && matches!(p.nth(1), DOT | QUESTION_DOT);
    let literal_span = p.nth_span(0);
    let rhs = expr_bp(p, bp + 1, ctx, Role::Rhs { skip_number: literal });
    let done = p.complete(marker, if range { RANGE_EXPR } else { BINARY_EXPR });
    if literal && rhs.is_some() {
        range_end_method(p, done, literal_span);
    }
    let whole = Expr { done, place: false, catch_name: rhs.and_then(|rhs| rhs.catch_name) };
    (whole, rhs.map(|rhs| rhs.done.span()))
}

#[cold]
fn chained_non_associative(p: &mut Parser<'_>, lhs: &Expr, right: Span, bp: u8) {
    if p.speculating() {
        return;
    }
    let op = p.nth_span(0);
    let diagnostic = if bp == BP_COMPARE {
        let middle = p.text_of(right).to_string();
        Diagnostic::error("comparisons can't be chained")
            .with_code(codes::CHAINED_COMPARISON)
            .primary(op, "a second comparison")
            .secondary(lhs.done.span(), "this is already a comparison")
            .suggest(
                format!(
                    "compare twice and join the comparisons with `and`: `... and {middle} ...`"
                ),
                Span::at(right.end),
                format!(" and {middle}"),
            )
    } else {
        let what = if bp == BP_RANGE { "ranges can't be chained" } else { "`is` can't be chained" };
        Diagnostic::error(what)
            .with_code(codes::CHAINED_RANGE_OR_IS)
            .primary(op, "a second operator of the same kind")
            .secondary(lhs.done.span(), "this comes first")
            .suggest_edits(
                "wrap one side in parentheses",
                wrap_in_parens(lhs.done.start, lhs.done.end),
            )
    };
    p.push_error(diagnostic);
}

#[cold]
fn assignment_in_condition(p: &mut Parser<'_>) {
    if p.speculating() {
        return;
    }
    let span = p.nth_span(0);
    p.push_error(
        Diagnostic::error("`=` gives a variable a new value. To compare, use `==`.")
            .with_code(codes::ASSIGNMENT_IN_CONDITION)
            .primary(span, "this is an assignment, not a comparison")
            .suggest("write `==`", span, "=="),
    );
}

// ----------------------------------------------------------------------
// Operands: prefix operators, lambdas and jumps in the wrong place
// ----------------------------------------------------------------------

/// A prefix expression or a postfix expression (levels 1, 2 and 14).
fn operand(p: &mut Parser<'_>, min_bp: u8, ctx: Ctx, role: Role) -> Option<Expr> {
    let kind = p.current();
    if is_jump(kind) {
        // Special case 3: a jump is a whole expression (or the right side of `??` or `catch`).
        let jump = jump(p, ctx, JumpPos::Misplaced(min_bp)).expr;
        needs_parentheses(p, jump.done.span(), "a jump");
        return Some(jump);
    }
    if !ctx.guard && at_lambda(p) {
        let lambda = lambda(p)?;
        needs_parentheses(p, lambda.done.span(), "a lambda");
        return Some(lambda);
    }
    if matches!(kind, NOT_KW | MINUS | TILDE | TRY_KW | SPAWN_KW) {
        // A prefix operator counts as a level of nesting of its own.
        if !p.enter() {
            return None;
        }
        let result = match kind {
            NOT_KW => not_expr(p, min_bp, ctx),
            MINUS | TILDE => prefix_expr(p, ctx),
            TRY_KW => {
                let marker = p.start();
                p.bump();
                expr_bp(p, BP_PREFIX, ctx, Role::Plain);
                Expr::new(p.complete(marker, TRY_EXPR))
            }
            _ => spawn_expr(p, ctx),
        };
        p.leave();
        return Some(result);
    }
    match kind {
        MUT_KW => {
            misplaced_mut(p);
            operand(p, min_bp, ctx, role)
        }
        _ => {
            let primary = primary(p)?;
            Some(postfix_loop(p, primary, ctx, role))
        }
    }
}

#[cold]
fn needs_parentheses(p: &mut Parser<'_>, span: Span, what: &str) {
    if p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error(format!("{what} can't be part of a larger expression"))
            .with_code(codes::NEEDS_PARENTHESES)
            .primary(span, format!("{what} here would take everything after it"))
            .note("lambdas and `return`, `throw`, `break` and `continue` are only allowed as a whole expression, or after `??` and `catch`")
            .suggest_edits("wrap this in parentheses", wrap_in_parens(span.start, span.end)),
    );
}

#[cold]
fn misplaced_mut(p: &mut Parser<'_>) {
    let span = p.nth_span(0);
    if !p.speculating() {
        p.push_error(
            Diagnostic::error("`mut` can't go here")
                .with_code(codes::MISPLACED_MUT)
                .primary(span, "`mut` before a value")
                .note("`mut` marks a changeable argument, as in `bump(mut c)`, a changeable parameter type, or a lambda parameter")
                .suggest("remove `mut`", with_space_after(p, span), ""),
        );
    }
    p.bump_error();
}

/// `NotExpr = "not" NotExpr | CompareExpr` (6.2). Special case 2: `not` right after an
/// operator of levels 3 to 13 or a prefix operator is an error, and is read as if it were in
/// parentheses.
fn not_expr(p: &mut Parser<'_>, min_bp: u8, ctx: Ctx) -> Expr {
    let misplaced = min_bp > BP_NOT;
    let marker = p.start();
    p.bump();
    expr_bp(p, if misplaced { min_bp } else { BP_NOT }, ctx, Role::Plain);
    let done = p.complete(marker, PREFIX_EXPR);
    if misplaced && !p.speculating() {
        let text = p.text_of(done.span()).to_string();
        p.push_error(
            Diagnostic::error("`not` can't come right after an operator")
                .with_code(codes::NOT_AFTER_OPERATOR)
                .primary(done.span(), "`not` binds more loosely than the operator before it")
                .note("`not` applies to a whole comparison, as in `not a == b`")
                .suggest_edits(
                    format!("wrap it in parentheses: `({text})`"),
                    wrap_in_parens(done.start, done.end),
                ),
        );
    }
    Expr::new(done)
}

/// `-x` and `~x` (6.9).
fn prefix_expr(p: &mut Parser<'_>, ctx: Ctx) -> Expr {
    let marker = p.start();
    let minus = p.at(MINUS);
    p.bump();
    // `-5.abs()` is `-(5.abs())`: warned about below.
    let literal =
        minus && matches!(p.current(), INT | FLOAT) && matches!(p.nth(1), DOT | QUESTION_DOT);
    let literal_span = p.nth_span(0);
    let operand = expr_bp(p, BP_PREFIX, ctx, Role::Prefix { skip_number: literal });
    let done = p.complete(marker, PREFIX_EXPR);
    if literal && operand.is_some() {
        negative_literal_method(p, done, literal_span);
    }
    Expr::new(done)
}

#[cold]
fn negative_literal_method(p: &mut Parser<'_>, done: Done, literal: Span) {
    if p.speculating() {
        return;
    }
    let number = p.text_of(literal).to_string();
    let rest = p.text_of(Span::new(literal.end, done.end)).trim().to_string();
    p.push_error(
        Diagnostic::warning(format!("this is `-({number}{rest})`, not a method call on -{number}"))
            .with_code(codes::METHOD_ON_NEGATIVE_LITERAL)
            .primary(
                Span::new(done.start, literal.end),
                "the method applies to the number before `-`",
            )
            .note("methods bind more tightly than `-`")
            .suggest_edits(
                format!("to call the method on -{number}, write `(-{number})`"),
                wrap_in_parens(done.start, literal.end),
            )
            .suggest_edits(
                "to keep this meaning, add parentheses",
                wrap_in_parens(literal.start, done.end),
            ),
    );
}

#[cold]
fn range_end_method(p: &mut Parser<'_>, range: Done, literal: Span) {
    if p.speculating() {
        return;
    }
    let number = p.text_of(literal).to_string();
    let start = p.text_of(Span::new(range.start, literal.start)).trim_end().to_string();
    let rest = p.text_of(Span::new(literal.end, range.end)).trim().to_string();
    p.push_error(
        Diagnostic::warning(format!("this is `{start}({number}{rest})`"))
            .with_code(codes::METHOD_ON_RANGE_END)
            .primary(
                Span::new(literal.start, range.end),
                "the method applies only to the end of the range",
            )
            .suggest_edits(
                format!("wrap the range in parentheses: `({start}{number}){rest}`"),
                wrap_in_parens(range.start, literal.end),
            )
            .suggest_edits(
                "to keep this meaning, add parentheses",
                wrap_in_parens(literal.start, range.end),
            ),
    );
}

/// `SpawnExpr = "spawn" ( Block | PostfixExpr )` (6.9).
fn spawn_expr(p: &mut Parser<'_>, ctx: Ctx) -> Expr {
    let marker = p.start();
    p.bump();
    if p.at_body_brace("`spawn`") {
        stmts::block(p);
    } else if let Some(operand) = expr_bp(p, BP_PREFIX, ctx, Role::Plain) {
        if operand.done.kind != CALL_EXPR && !p.speculating() {
            let span = operand.done.span();
            p.push_error(
                Diagnostic::error("`spawn` needs a function call or a block")
                    .with_code(codes::SPAWN_WITHOUT_CALL)
                    .primary(span, "this is not a call")
                    .note("`spawn` starts a task that runs the call or the block")
                    .suggest_edits(
                        "to run this in a task, put it in a block",
                        vec![
                            Edit { span: Span::at(span.start), replacement: "{ ".into() },
                            Edit { span: Span::at(span.end), replacement: " }".into() },
                        ],
                    ),
            );
        }
    }
    Expr::new(p.complete(marker, SPAWN_EXPR))
}

// ----------------------------------------------------------------------
// catch, ?? and is (6.4 to 6.6)
// ----------------------------------------------------------------------

/// `catch` and its right side: the binding form `catch name { ... }` or a fallback.
fn catch_rest(p: &mut Parser<'_>, lhs: Expr, ctx: Ctx) -> Expr {
    if lhs.done.kind == TRY_EXPR {
        try_before_catch(p, lhs.done);
    }
    let marker = lhs.done.precede(p);
    p.bump(); // `catch`
    let name_next = matches!(p.current(), LOWER_IDENT | UNDERSCORE);
    let brace_next = p.nth(1) == L_BRACE;
    // `catch err` with `{` on the next line, outside brackets (10.3).
    let brace_below = !ctx.header && p.nth(1) == NEWLINE && p.nth(2) == L_BRACE;
    if name_next && (brace_next || brace_below) {
        let name = p.nth_span(0);
        p.bump();
        if p.at(NEWLINE) || p.nth_line_break_before(0) {
            catch_brace_on_next_line(p, name);
            if p.at(NEWLINE) {
                p.bump();
            }
        }
        stmts::block(p);
        let done = p.complete(marker, CATCH_EXPR);
        return Expr { done, place: false, catch_name: Some(name) };
    }
    let rhs_start = p.checkpoint();
    let rhs = fallback(p, ctx);
    let done = p.complete(marker, CATCH_EXPR);
    after_fallback(p, done, rhs_start, rhs, "catch");
    Expr::new(done)
}

/// What may follow the binding form of `catch` (special case 5): another `catch`, an
/// operator of level 12 or looser, or the end of the expression.
fn after_binding_catch(p: &mut Parser<'_>, lhs: Expr, ctx: Ctx) -> Expr {
    let kind = p.current();
    let postfix = matches!(kind, DOT | QUESTION_DOT)
        || (matches!(kind, L_PAREN | L_BRACK) && !p.nth_line_break_before(0));
    let tight = infix_bp(kind).is_some_and(|bp| bp >= BP_COALESCE)
        && !(kind == MINUS && p.nth_line_break_before(0));
    if !postfix && !tight {
        return lhs;
    }
    if !p.speculating() {
        let span = p.nth_span(0);
        let text = p.text_of(lhs.done.span()).to_string();
        let short = if text.len() <= 40 && !text.contains('\n') { text } else { "...".to_string() };
        p.push_error(
            Diagnostic::error("the `catch` with a block must be wrapped in parentheses here")
                .with_code(codes::OPERATOR_AFTER_BINDING_CATCH)
                .primary(span, "this applies to the whole `catch`")
                .secondary(lhs.done.span(), "the binding form of `catch` ends here")
                .suggest_edits(
                    format!("wrap the `catch` in parentheses: `({short})`"),
                    wrap_in_parens(lhs.done.start, lhs.done.end),
                ),
        );
    }
    let lhs = Expr { catch_name: None, ..lhs };
    if postfix { postfix_loop(p, lhs, ctx, Role::Plain) } else { lhs }
}

#[cold]
fn catch_brace_on_next_line(p: &mut Parser<'_>, name: Span) {
    if p.speculating() {
        return;
    }
    let brace = if p.at(NEWLINE) { p.nth_span(1) } else { p.nth_span(0) };
    let name_text = p.text_of(name).to_string();
    let between = Span::new(name.end, brace.start);
    let only_space = p.text_of(between).chars().all(char::is_whitespace);
    let diagnostic = Diagnostic::error(format!("put `{{` on the same line as `catch {name_text}`"))
        .with_code(codes::CATCH_BRACE_ON_NEXT_LINE)
        .primary(brace, "this block is on the next line")
        .secondary(name, "this name");
    let diagnostic = if only_space {
        diagnostic.suggest("move the `{` up", between, " ")
    } else {
        diagnostic.help("move the `{` to the end of the line with `catch`")
    };
    p.push_error(diagnostic);
}

#[cold]
fn try_before_catch(p: &mut Parser<'_>, try_expr: Done) {
    if p.speculating() {
        return;
    }
    let try_span = Span::new(try_expr.start, try_expr.start + 3);
    p.push_error(
        Diagnostic::warning("`try` passes the error up, so this `catch` never runs")
            .with_code(codes::TRY_BEFORE_CATCH)
            .primary(try_span, "the error leaves here")
            .note("`try load() catch 0` is `(try load()) catch 0`")
            .suggest("remove `try`", with_space_after(p, try_span), ""),
    );
}

/// `??` and its right side.
fn coalesce_rest(p: &mut Parser<'_>, lhs: Expr, ctx: Ctx) -> Expr {
    let marker = lhs.done.precede(p);
    p.bump(); // `??`
    let rhs_start = p.checkpoint();
    let rhs = fallback(p, ctx);
    let done = p.complete(marker, BINARY_EXPR);
    after_fallback(p, done, rhs_start, rhs, "??");
    Expr::new(done)
}

/// The right side of `??`, or the fallback of `catch`.
#[derive(Clone, Copy, Debug)]
struct Fallback {
    expr: Expr,
    /// For a jump with a value: the parser state where the value starts.
    jump_value: Option<super::Checkpoint>,
}

/// The right side of `??`, or the fallback of `catch`: a jump, or a `CoalesceExpr` with no
/// arithmetic at its top (special case 4). `??` is right-associative.
fn fallback(p: &mut Parser<'_>, ctx: Ctx) -> Option<Fallback> {
    if is_jump(p.current()) {
        let jump = jump(p, ctx, JumpPos::Fallback);
        return Some(Fallback { expr: jump.expr, jump_value: jump.value });
    }
    let operand = expr_bp(p, BP_PREFIX, ctx, Role::Rhs { skip_number: false })?;
    if p.at(QUESTION_QUESTION) && p.nth(1) != EQ {
        if !p.enter() {
            return Some(Fallback { expr: operand, jump_value: None });
        }
        let rest = coalesce_rest(p, operand, ctx);
        p.leave();
        return Some(Fallback { expr: rest, jump_value: None });
    }
    Some(Fallback { expr: operand, jump_value: None })
}

/// The checks after a fallback: arithmetic after it (special case 4) and a jump value followed
/// by a looser operator (6.6).
fn after_fallback(
    p: &mut Parser<'_>,
    whole: Done,
    rhs_start: super::Checkpoint,
    rhs: Option<Fallback>,
    operator: &str,
) {
    let Some(Fallback { expr: rhs, jump_value }) = rhs else { return };
    let kind = p.current();
    if let Some(value) = jump_value {
        let looser =
            matches!(kind, CATCH_KW | IS_KW | AND_KW | OR_KW) || infix_bp(kind) == Some(BP_COMPARE);
        if looser {
            unclear_jump_value(p, whole, rhs, value);
        }
        return;
    }
    if matches!(rhs.done.kind, BREAK_EXPR | CONTINUE_EXPR | RETURN_EXPR | THROW_EXPR) {
        return;
    }
    if !is_arithmetic(kind) || (kind == MINUS && p.nth_line_break_before(0)) || p.speculating() {
        return;
    }
    if matches!(kind, PLUS | MINUS) && at_increment(p) {
        return;
    }
    let ctx = Ctx::default();
    let end = p.measure(rhs_start, |p| {
        expr_bp(p, BP_COALESCE, ctx, Role::Plain);
    });
    let op = p.nth_span(0);
    let left = p.text_of(whole.span()).to_string();
    let mut diagnostic = Diagnostic::error("a fallback with arithmetic needs parentheses")
        .with_code(codes::FALLBACK_WITH_ARITHMETIC)
        .primary(op, format!("is this applied before or after `{operator}`?"))
        .secondary(whole.span(), "the fallback")
        .note(format!(
            "`{operator}` binds more loosely than `{}`, but reading from left to right suggests the opposite",
            p.text_of(op)
        ))
        .suggest_edits(
            format!("to use the fallback first, write `({left}) ...`"),
            wrap_in_parens(whole.start, whole.end),
        );
    if let Some(end) = end {
        if end > rhs.done.end {
            diagnostic = diagnostic.suggest_edits(
                "to make the whole calculation the fallback, wrap it in parentheses",
                wrap_in_parens(rhs.done.start, end),
            );
        }
    }
    p.push_error(diagnostic);
}

#[cold]
fn unclear_jump_value(p: &mut Parser<'_>, whole: Done, jump: Expr, value: super::Checkpoint) {
    if p.speculating() {
        return;
    }
    let op = p.nth_span(0);
    let jump_text = p.text_of(jump.done.span()).to_string();
    let keyword = jump_text.split_whitespace().next().unwrap_or("return").to_string();
    let value_start = p.span_at(value.pos).start;
    let ctx = Ctx::default();
    // Where the value would end if it took everything after it.
    let end = p.measure(value, |p| {
        full_expr(p, ctx);
    });
    let op_text = p.text_of(op).to_string();
    let mut diagnostic = Diagnostic::error(format!(
        "this is unclear: is `{op_text}` part of the value of `{keyword}`?"
    ))
    .with_code(codes::UNCLEAR_JUMP_VALUE)
    .primary(op, format!("this comes after the value of `{keyword}`"))
    .secondary(jump.done.span(), format!("`{keyword}` takes only this value"))
    .suggest_edits(
        format!("to leave early, write `(... {jump_text})` and keep the rest outside"),
        wrap_in_parens(whole.start, whole.end),
    );
    if let Some(end) = end {
        if end > jump.done.end {
            diagnostic = diagnostic.suggest_edits(
                format!("to {keyword} the whole value, wrap it in parentheses"),
                wrap_in_parens(value_start, end),
            );
        }
    }
    p.push_error(diagnostic);
}

/// `is` and the type after it (6.4), with the messages for `is not` and `is none`.
fn is_rest(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let is_span = p.nth_span(0);
    p.bump();
    if p.at(NOT_KW) {
        let not_span = p.nth_span(0);
        p.bump_error();
        let ty = types::type_(p, MutRule::Forbidden);
        if let (Some(ty), false) = (ty, p.speculating()) {
            let lhs_text = p.text_of(lhs.done.span()).to_string();
            let ty_text = p.text_of(ty.span()).to_string();
            p.push_error(
                Diagnostic::error("Quell has no `is not`")
                    .with_code(codes::IS_NOT)
                    .primary(is_span.to(not_span), "`is` is followed by a type")
                    .suggest_edits(
                        format!("write `not ({lhs_text} is {ty_text})`"),
                        vec![
                            Edit { span: Span::at(lhs.done.start), replacement: "not (".into() },
                            Edit {
                                span: with_space_after(p, not_span),
                                replacement: String::new(),
                            },
                            Edit { span: Span::at(ty.end), replacement: ")".into() },
                        ],
                    ),
            );
        }
    } else if p.at(NONE_KW) {
        let none_span = p.nth_span(0);
        p.bump_error();
        if !p.speculating() {
            let lhs_text = p.text_of(lhs.done.span()).to_string();
            p.push_error(
                Diagnostic::error("`is` checks a type, and `none` is a value")
                    .with_code(codes::IS_NONE)
                    .primary(is_span.to(none_span), "`none` is not a type")
                    .suggest(
                        format!("to check for a missing value, write `{lhs_text} == none`"),
                        is_span,
                        "==",
                    ),
            );
        }
    } else {
        types::type_(p, MutRule::Forbidden);
    }
    Expr::new(p.complete(marker, IS_EXPR))
}

// ----------------------------------------------------------------------
// Rejected syntax made of valid tokens (grammar.md, 11)
// ----------------------------------------------------------------------

/// Whether a `?` here starts a ternary `a ? b : c`: a `:` follows later on the line.
fn ternary_ahead(p: &Parser<'_>) -> bool {
    let mut pos = p.pos() + 1;
    for _ in 0..1000 {
        match p.kind_at(pos) {
            COLON => return true,
            NEWLINE | EOF | COMMA | HOLE_END | FORMAT_SPEC | STRING_END | R_PAREN | R_BRACK
            | R_BRACE => return false,
            L_PAREN | L_BRACK | L_BRACE | STRING_START => match p.partner_of(pos) {
                Some(close) => pos = close + 1,
                None => return false,
            },
            _ => pos += 1,
        }
    }
    false
}

/// `cond ? a : b`: reported, with a fix that writes it as an `if`.
fn ternary(p: &mut Parser<'_>, cond: Expr) -> Expr {
    let marker = cond.done.precede(p);
    let question = p.nth_span(0);
    p.bump();
    let yes = full_expr(p, Ctx::default());
    let colon = p.nth_span(0);
    let no = if p.eat(COLON) { full_expr(p, Ctx::default()) } else { None };
    let done = p.complete(marker, ERROR);
    if !p.speculating() {
        let mut diagnostic = Diagnostic::error("Quell has no `? :` operator")
            .with_code(codes::TERNARY)
            .primary(question, "a `?` after a value")
            .note("`if` gives a value, so it does the same job");
        if let (Some(yes), Some(no)) = (yes, no) {
            diagnostic = diagnostic.suggest_edits(
                "write it as an `if`: `if ok { a } else { b }`",
                vec![
                    Edit { span: Span::at(cond.done.start), replacement: "if ".into() },
                    Edit {
                        span: Span::new(cond.done.end, yes.done.start),
                        replacement: " { ".into(),
                    },
                    Edit {
                        span: Span::new(yes.done.end, no.done.start),
                        replacement: " } else { ".into(),
                    },
                    Edit { span: Span::at(no.done.end), replacement: " }".into() },
                ],
            );
        } else {
            let _ = colon;
            diagnostic = diagnostic.help("write it as an `if`: `if ok { a } else { b }`");
        }
        p.push_error(diagnostic);
    }
    Expr::new(done)
}

/// A Rust-style `value?`: reported, with a fix that uses `try`.
fn question_mark(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let question = p.nth_span(0);
    p.bump();
    let done = p.complete(marker, ERROR);
    if !p.speculating() {
        p.push_error(
            Diagnostic::error("Quell has no `?` operator after a value")
                .with_code(codes::QUESTION_MARK_OPERATOR)
                .primary(question, "a `?` after a value")
                .note("`?` is only used at the end of a type, as in `String?`")
                .suggest_edits(
                    "use `try value` to pass the error up",
                    vec![
                        Edit { span: Span::at(lhs.done.start), replacement: "try ".into() },
                        Edit { span: question, replacement: String::new() },
                    ],
                )
                .help("or use `value ?? fallback` for a value that might be missing"),
        );
    }
    Expr::new(done)
}

/// Two `+` or two `-` written together right after an operand, before a line break or a
/// closing bracket (grammar.md, 11).
fn at_increment(p: &Parser<'_>) -> bool {
    let kind = p.current();
    p.nth(1) == kind
        && p.nth_is_joined(1)
        && (p.nth_line_break_before(2)
            || matches!(p.nth(2), NEWLINE | EOF | R_PAREN | R_BRACK | R_BRACE))
}

fn increment(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let first = p.nth_span(0);
    let plus = p.at(PLUS);
    p.bump();
    let second = p.nth_span(0);
    p.bump();
    let done = p.complete(marker, ERROR);
    if !p.speculating() {
        let (written, op) = if plus { ("++", "+=") } else { ("--", "-=") };
        let space = if lhs.done.end == first.start { " " } else { "" };
        let place = p.text_of(lhs.done.span()).to_string();
        p.push_error(
            Diagnostic::error(format!("Quell has no `{written}` operator"))
                .with_code(codes::INCREMENT_DECREMENT)
                .primary(first.to(second), "not an operator in Quell")
                .suggest(
                    format!("write `{place} {op} 1`"),
                    first.to(second),
                    format!("{space}{op} 1"),
                ),
        );
    }
    Expr::new(done)
}

#[cold]
pub(crate) fn double_colon(p: &mut Parser<'_>) {
    let first = p.nth_span(0);
    let second = p.nth_span(1);
    let marker = p.start();
    p.bump();
    p.bump();
    p.complete(marker, ERROR);
    if !p.speculating() {
        p.push_error(
            Diagnostic::error("paths use `.`, not `::`")
                .with_code(codes::DOUBLE_COLON)
                .primary(first.to(second), "`::` is not used in Quell")
                .suggest("write `.`", first.to(second), "."),
        );
    }
}

// ----------------------------------------------------------------------
// Postfix operators (6.10)
// ----------------------------------------------------------------------

/// The postfix loop: calls, members, indexes, and the rejected forms that look like them.
pub(crate) fn postfix_loop(p: &mut Parser<'_>, mut lhs: Expr, ctx: Ctx, role: Role) -> Expr {
    let depth = p.depth();
    let start = lhs.done.start;
    // The first `.` or `?.` that starts a line: its span and where the operand before it ended.
    let mut line_dot: Option<(Span, u32)> = None;
    loop {
        let kind = p.current();
        match kind {
            L_PAREN | L_BRACK => {
                if p.at_soft_break() {
                    break;
                }
                if p.nth_line_break_before(0) && forgotten_comma(p) {
                    break;
                }
                if !p.enter() {
                    break;
                }
                lhs = if kind == L_PAREN { call(p, lhs) } else { index(p, lhs) };
            }
            DOT | QUESTION_DOT => {
                if line_dot.is_none() && p.nth_line_break_before(0) {
                    line_dot = Some((p.nth_span(0), lhs.done.end));
                }
                if !p.enter() {
                    break;
                }
                lhs = member(p, lhs);
            }
            COLON if p.nth(1) == COLON && p.nth_is_joined(1) => {
                if !p.enter() {
                    break;
                }
                let marker = lhs.done.precede(p);
                double_colon(p);
                member_name(p);
                lhs = Expr::new(p.complete(marker, MEMBER_EXPR));
            }
            L_BRACE if !ctx.header && at_struct_literal(p) => {
                if !p.enter() {
                    break;
                }
                lhs = struct_literal(p, lhs);
            }
            QUESTION => {
                if ternary_ahead(p) {
                    break;
                }
                lhs = question_mark(p, lhs);
            }
            _ => break,
        }
    }
    p.set_depth(depth);
    if let Some((dot, receiver_end)) = line_dot {
        let skip =
            matches!(role, Role::Rhs { skip_number: true } | Role::Prefix { skip_number: true });
        if role != Role::Plain && !skip {
            dot_on_right_operand(p, start, dot, receiver_end, lhs.done.end, role);
        }
    }
    lhs
}

#[cold]
fn dot_on_right_operand(
    p: &mut Parser<'_>,
    operand_start: u32,
    dot: Span,
    receiver_end: u32,
    chain_end: u32,
    role: Role,
) {
    if p.speculating() {
        return;
    }
    let short = |text: &str| -> String {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.len() <= 40 { text } else { "...".to_string() }
    };
    let member = short(p.text_of(Span::new(dot.start, chain_end)));
    let receiver = short(p.text_of(Span::new(operand_start, receiver_end)));
    let what = match role {
        Role::Prefix { .. } => "the operand of the prefix operator",
        _ => "the right side of the operator",
    };
    let whole_start = p.expr_start.min(operand_start);
    p.push_error(
        Diagnostic::warning(format!("this `{member}` applies only to `{receiver}`"))
            .with_code(codes::DOT_ON_RIGHT_OPERAND)
            .primary(
                dot,
                format!("a line that starts with `.` continues {what}, not the whole line"),
            )
            .secondary(Span::new(operand_start, receiver_end), "it applies to this")
            .suggest_edits(
                "to apply it to the whole expression, wrap the expression in parentheses",
                wrap_in_parens(whole_start, receiver_end),
            )
            .suggest_edits(
                "to keep this meaning, wrap the right side in parentheses",
                wrap_in_parens(operand_start, chain_end),
            ),
    );
}

/// The forgotten-comma check (6.12): a binary `-` or a call or index `(` or `[` is the first
/// token on its line inside `( )` or `[ ]`. In a list or map literal it is an error and the
/// caller stops the element here (returns `true`); elsewhere it is a warning.
fn forgotten_comma(p: &mut Parser<'_>) -> bool {
    match p.bracket {
        Bracket::Block => false,
        Bracket::List => {
            forgotten_comma_error(p);
            p.comma_forgiven = p.pos();
            true
        }
        Bracket::Other => {
            if !p.speculating() {
                let at = Span::at(p.last_end());
                let span = p.nth_span(0);
                p.push_error(
                    Diagnostic::warning("did you forget a comma at the end of the line above?")
                        .with_code(codes::FORGOTTEN_COMMA_WARNING)
                        .primary(span, "this continues the line above")
                        .note("inside `( )` and `[ ]` line breaks are ignored, so these two lines are one expression")
                        .suggest("to make two values, add a comma", at, ",")
                        .help("to continue the line on purpose, put the operator at the end of the line above"),
                );
            }
            false
        }
    }
}

/// The forgotten-comma error, with a fix that adds the comma at the end of the line above.
pub(crate) fn forgotten_comma_error(p: &mut Parser<'_>) {
    if p.speculating() || p.already_reported_here() {
        return;
    }
    let at = Span::at(p.last_end());
    let span = if p.at(NEWLINE) { p.nth_span(1) } else { p.nth_span(0) };
    p.push_error(
        Diagnostic::error("did you forget a comma at the end of the line above?")
            .with_code(codes::FORGOTTEN_COMMA)
            .primary(span, "this starts a new element")
            .suggest("add a comma", at, ","),
    );
}

/// A missing comma between two elements: on the same line an "expected `,`" error, at the
/// start of a line the forgotten-comma error.
pub(crate) fn missing_comma(p: &mut Parser<'_>) {
    if p.nth_line_break_before(0) {
        forgotten_comma_error(p);
        return;
    }
    if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`,`") {
        let at = Span::at(p.last_end());
        p.push_error(diagnostic.suggest("add a comma", at, ","));
    }
}

fn call(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    arg_list(p);
    Expr::new(p.complete(marker, CALL_EXPR))
}

/// `CallSuffix = "(" [ ArgList ] ")"`.
pub(crate) fn arg_list(p: &mut Parser<'_>) {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    delimited(p, open, "an argument", arg);
    p.bracket = saved;
    p.close_bracket(open, "the argument list");
    p.complete(marker, ARG_LIST);
}

/// `Arg = [ IDENT ":" ] ( LambdaExpr | "mut" Place | Expr )`, tried in this order (6.10).
fn arg(p: &mut Parser<'_>) -> bool {
    let marker = p.start();
    if p.current().is_ident() && p.nth(1) == COLON && !(p.nth(2) == COLON && p.nth_is_joined(2)) {
        p.bump();
        p.bump();
    } else if p.current().is_ident() && p.nth(1) == EQ {
        named_argument_with_equals(p);
    }
    let value = if at_lambda(p) {
        lambda(p)
    } else if p.at(MUT_KW) {
        let mut_span = p.nth_span(0);
        p.bump();
        let value = expr(p, Ctx::default());
        if let Some(value) = value {
            if !value.place {
                mut_argument_not_a_place(p, mut_span, value.done.span());
            }
        }
        value
    } else {
        expr(p, Ctx::default())
    };
    if value.is_none() && !p.consumed_since(&marker) {
        p.abandon(marker);
        return false;
    }
    p.complete(marker, ARG);
    true
}

#[cold]
fn named_argument_with_equals(p: &mut Parser<'_>) {
    let name = p.nth_span(0);
    let eq = p.nth_span(1);
    p.bump();
    if !p.speculating() {
        let name_text = p.text_of(name).to_string();
        p.push_error(
            Diagnostic::error("named arguments use `:`, not `=`")
                .with_code(codes::NAMED_ARGUMENT_WITH_EQUALS)
                .primary(eq, "an assignment can't go inside a call")
                .suggest(format!("write `{name_text}: ...`"), Span::new(name.end, eq.end), ":"),
        );
    }
    p.bump();
}

#[cold]
fn mut_argument_not_a_place(p: &mut Parser<'_>, mut_span: Span, value: Span) {
    if p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error("only a variable, a field or an element can be passed with `mut`")
            .with_code(codes::MUT_ARGUMENT_NOT_A_PLACE)
            .primary(value, "this is not a variable, a field or an element")
            .secondary(mut_span, "`mut` lets the function change what is passed")
            .suggest("remove `mut`", with_space_after(p, mut_span), ""),
    );
}

/// The elements of a comma-separated list in brackets, up to (not including) the closer of
/// the opener at `open`. Returns the number of elements and the span of the comma after the
/// last one, if there is one.
pub(crate) fn delimited(
    p: &mut Parser<'_>,
    open: usize,
    what: &str,
    mut element: impl FnMut(&mut Parser<'_>) -> bool,
) -> (usize, Option<Span>) {
    let close = p.partner_of(open);
    let mut count = 0;
    let mut trailing = None;
    loop {
        if stmts::at_list_close(p, close) || p.at(NEWLINE) {
            break;
        }
        // In a bracket that is never closed (the lexer reported it), stop where the list
        // clearly ended: at a token that can't start an element, or at a `{` (a body).
        if close.is_none() && (p.at(L_BRACE) || !can_start_element(p.current())) {
            break;
        }
        let start = p.pos();
        if element(p) {
            count += 1;
            trailing = None;
        } else if p.pos() == start {
            skip_element(p, close);
            if p.pos() == start {
                break;
            }
        }
        if p.at(COMMA) {
            trailing = Some(p.nth_span(0));
            p.bump();
            continue;
        }
        if stmts::at_list_close(p, close) || p.at(NEWLINE) {
            break;
        }
        if close.is_none() && p.at_soft_break() {
            break;
        }
        if p.comma_forgiven == p.pos() {
            continue;
        }
        if can_start_expr(p.current()) {
            missing_comma(p);
            if p.pos() == start {
                break;
            }
            continue;
        }
        if !p.already_reported_here() {
            p.error_expected(
                codes::EXPECTED_TOKEN,
                "`,`",
                &format!("separate each {what} from the next with a comma"),
            );
        }
        skip_element(p, close);
        if p.pos() == start {
            break;
        }
    }
    (count, trailing)
}

/// Whether an element of a comma-separated list (an expression, a parameter, a type or a
/// pattern) can start with this token.
fn can_start_element(kind: SyntaxKind) -> bool {
    can_start_expr(kind) || kind.is_keyword() || kind == DOT
}

/// Skips to the next comma or the end of the list.
fn skip_element(p: &mut Parser<'_>, close: Option<usize>) {
    let marker = p.start();
    while !p.at(COMMA) && !stmts::at_list_close(p, close) && !p.at_safe_point() {
        p.skip_one_balanced();
    }
    if p.consumed_since(&marker) {
        p.complete(marker, ERROR);
    } else {
        p.abandon(marker);
    }
}

fn index(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    if !stmts::at_list_close(p, p.partner_of(open)) || p.partner_of(open).is_none() {
        if expr(p, Ctx::default()).is_none() && !p.already_reported_here() {
            p.error_expected(codes::EXPECTED_EXPRESSION, "an index", "write the index in `[ ]`");
        }
    } else {
        p.error_expected(
            codes::EXPECTED_EXPRESSION,
            "an index",
            "write the index in `[ ]`, as in `scores[0]`",
        );
    }
    p.bracket = saved;
    p.close_bracket(open, "the index");
    Expr { done: p.complete(marker, INDEX_EXPR), place: lhs.place, catch_name: None }
}

/// `"." MemberName [ GenericArgs ]` or `"?." MemberName [ GenericArgs ]`.
fn member(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let optional = p.at(QUESTION_DOT);
    p.bump();
    let generic = member_name(p);
    let done = p.complete(marker, MEMBER_EXPR);
    Expr { done, place: lhs.place && !optional && !generic, catch_name: None }
}

/// The name after `.` or `?.` (a keyword is a name here), then optional generic arguments.
/// Returns whether generic arguments were read.
fn member_name(p: &mut Parser<'_>) -> bool {
    let kind = p.current();
    if kind.is_ident() || kind.is_keyword() {
        p.bump();
        return types::try_generic_args(p);
    }
    if matches!(kind, INT | FLOAT) {
        let span = p.nth_span(0);
        if !p.speculating() {
            p.push_error(
                Diagnostic::error("tuple elements can't be reached by number")
                    .with_code(codes::NUMBERED_FIELD)
                    .primary(span, "a number after `.`")
                    .help("take the tuple apart instead: `let (first, second) = pair`"),
            );
        }
        p.bump_error();
        return false;
    }
    p.error_expected(
        codes::EXPECTED_NAME,
        "a name after `.`",
        "write the name of a field or method",
    );
    false
}

/// The four tokens of a struct literal with braces, on one line: an uppercase name that ended
/// the operand, `{`, a name and `:` (grammar.md, 11).
fn at_struct_literal(p: &Parser<'_>) -> bool {
    p.last_kind() == UPPER_IDENT
        && !p.nth_line_break_before(0)
        && p.nth(1).is_ident()
        && !p.nth_line_break_before(1)
        && p.nth(2) == COLON
        && !p.nth_line_break_before(2)
}

/// `Point { x: 1.0, y: 2.0 }`: reported, and read as `Point(x: 1.0, y: 2.0)`.
fn struct_literal(p: &mut Parser<'_>, lhs: Expr) -> Expr {
    let marker = lhs.done.precede(p);
    let args = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    let close = p.partner_of(open);
    let mut edits = vec![Edit {
        span: Span::new(lhs.done.end, open_span.end + spaces_after(p, open_span.end)),
        replacement: "(".to_string(),
    }];
    loop {
        while p.at(NEWLINE) {
            edits.push(Edit { span: Span::at(p.last_end()), replacement: ",".to_string() });
            p.bump();
        }
        if stmts::at_list_close(p, close) {
            break;
        }
        let start = p.pos();
        arg(p);
        if p.eat(COMMA) || p.at(NEWLINE) {
            continue;
        }
        if stmts::at_list_close(p, close) {
            break;
        }
        skip_element(p, close);
        if p.pos() == start {
            break;
        }
    }
    p.bracket = saved;
    if let Some(close) = close {
        let close_span = p.span_at(close);
        let before =
            p.text()[..close_span.start as usize].trim_end_matches([' ', '\t']).len() as u32;
        let before = if p.text()[before as usize..close_span.start as usize].is_empty()
            || p.text()[..before as usize].ends_with('\n')
        {
            close_span.start
        } else {
            before
        };
        edits.push(Edit { span: Span::new(before, close_span.end), replacement: ")".to_string() });
    }
    p.close_bracket(open, "the struct literal");
    p.complete(args, ARG_LIST);
    let done = p.complete(marker, CALL_EXPR);
    if !p.speculating() {
        let name = p.text_of(lhs.done.span()).to_string();
        let diagnostic = Diagnostic::error("values are created with parentheses, not braces")
            .with_code(codes::STRUCT_LITERAL_WITH_BRACES)
            .primary(open_span, "`{` after a type name")
            .note("braces in Quell always open a block");
        let diagnostic = if close.is_some() {
            diagnostic.suggest_edits(format!("write `{name}(...)`"), edits)
        } else {
            diagnostic.help(format!("write `{name}(field: value, ...)`"))
        };
        p.push_error(diagnostic);
    }
    Expr::new(done)
}

fn spaces_after(p: &Parser<'_>, offset: u32) -> u32 {
    let rest = &p.text()[offset as usize..];
    (rest.len() - rest.trim_start_matches([' ', '\t']).len()) as u32
}

// ----------------------------------------------------------------------
// Primary expressions (6.12 to 6.17)
// ----------------------------------------------------------------------

/// `Primary = Literal | STRING | NameExpr | "self" | ParenExpr | ListExpr | Block | IfExpr |
/// MatchExpr | ParallelFor | GroupExpr | UnsafeExpr`. Returns `None`, after reporting, when no
/// expression starts here; nothing is consumed then.
fn primary(p: &mut Parser<'_>) -> Option<Expr> {
    let kind = p.current();
    if matches!(kind, STRING_START | L_PAREN | L_BRACK | IF_KW | MATCH_KW) {
        // Brackets and the keyword constructs count as a level of nesting of their own (a
        // block counts in `stmts::block`).
        if !p.enter() {
            return None;
        }
        let result = match kind {
            STRING_START => Expr::new(string_literal(p).done),
            L_PAREN => paren_expr(p),
            L_BRACK => list_expr(p),
            IF_KW => if_expr(p),
            _ => match_expr(p),
        };
        p.leave();
        return Some(result);
    }
    let marker = p.start();
    let (kind, place) = match kind {
        INT | FLOAT | CHAR | TRUE_KW | FALSE_KW | NONE_KW => {
            p.bump();
            (LITERAL, false)
        }
        LOWER_IDENT
            if p.nth_text(0) == "parallel" && p.nth(1) == FOR_KW && !p.nth_line_break_before(1) =>
        {
            p.bump();
            p.bump();
            super::patterns::binding_pattern(p);
            stmts::expect_in(p);
            let header = header_expr(p, false);
            stmts::body_block(p, "the `for` header", header.as_ref());
            (PARALLEL_FOR_EXPR, false)
        }
        UPPER_IDENT | LOWER_IDENT => {
            p.bump();
            let generic = types::try_generic_args(p);
            (NAME_EXPR, kind == LOWER_IDENT && !generic)
        }
        SELF_KW => {
            p.bump();
            (SELF_EXPR, true)
        }
        L_BRACE => {
            p.abandon(marker);
            return stmts::block(p).map(|block| Expr::new(block.done));
        }
        GROUP_KW | UNSAFE_KW => {
            let what = if kind == GROUP_KW { "`group`" } else { "`unsafe`" };
            p.bump();
            stmts::body_block(p, what, None);
            (if kind == GROUP_KW { GROUP_EXPR } else { UNSAFE_EXPR }, false)
        }
        DOT if p.nth(1) == UPPER_IDENT && p.nth_is_joined(1) => {
            dot_option(p);
            p.bump();
            (NAME_EXPR, false)
        }
        UNDERSCORE => {
            let span = p.nth_span(0);
            if !p.speculating() {
                p.push_error(
                    Diagnostic::error("`_` is not a value")
                        .with_code(codes::UNDERSCORE_AS_VALUE)
                        .primary(span, "`_` here")
                        .note("`_` is only used in patterns and as an unused lambda parameter, as in `_ => 0`")
                        .help("write a value here"),
                );
            }
            p.bump();
            (ERROR, false)
        }
        _ => {
            p.abandon(marker);
            if !p.already_reported_here() {
                p.error_expected(codes::EXPECTED_EXPRESSION, "an expression", "write a value here");
            }
            return None;
        }
    };
    let done = p.complete(marker, kind);
    Some(Expr { done, place, catch_name: None })
}

#[cold]
fn dot_option(p: &mut Parser<'_>) {
    let dot = p.nth_span(0);
    let name = p.nth_text(1).to_string();
    let marker = p.start();
    p.bump();
    p.complete(marker, ERROR);
    if !p.speculating() {
        p.push_error(
            Diagnostic::error("an expression can't start with `.`")
                .with_code(codes::DOT_OPTION)
                .primary(dot, "a `.` before an option name")
                .note("Quell has no `.Option` shorthand")
                .suggest(
                    format!("enum options are written by name: `{name}`, or `Type.{name}`"),
                    dot,
                    "",
                ),
        );
    }
}

/// `ParenExpr = "(" Expr ")" | "(" Expr "," Expr { "," Expr } [ "," ] ")"`.
fn paren_expr(p: &mut Parser<'_>) -> Expr {
    let marker = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    let saved_start = p.expr_start;
    let (count, trailing) = delimited(p, open, "element", |p| expr(p, Ctx::default()).is_some());
    p.expr_start = saved_start;
    p.bracket = saved;
    let close_span = p.nth_span(0);
    p.close_bracket(open, "the parentheses");
    let kind = match (count, trailing) {
        (0, _) => {
            if p.partner_of(open).is_some() {
                unit_or_one_tuple(p, open_span.to(close_span), None, "a value");
            }
            TUPLE_EXPR
        }
        (1, Some(comma)) => {
            unit_or_one_tuple(p, comma, Some(comma), "a value");
            PAREN_EXPR
        }
        (1, None) => PAREN_EXPR,
        _ => TUPLE_EXPR,
    };
    Expr::new(p.complete(marker, kind))
}

/// `()` and `(x,)` (6.12, 7.1, 8.1): the unit value and one-element tuples are not in v0.1.
#[cold]
pub(crate) fn unit_or_one_tuple(p: &mut Parser<'_>, span: Span, comma: Option<Span>, what: &str) {
    if p.speculating() {
        return;
    }
    let diagnostic = match comma {
        None => Diagnostic::error(format!("`()` is not {what} in Quell"))
            .with_code(codes::UNIT_OR_ONE_ELEMENT_TUPLE)
            .primary(span, "empty parentheses")
            .note("Quell has no unit value; a function that gives nothing has no `->`")
            .help("leave the parentheses out, or put something inside them"),
        Some(comma) => Diagnostic::error(format!("a one-element tuple is not {what} in Quell"))
            .with_code(codes::UNIT_OR_ONE_ELEMENT_TUPLE)
            .primary(comma, "this comma makes a tuple of one element")
            .note("a tuple has at least two elements")
            .suggest("remove the comma", comma, ""),
    };
    p.push_error(diagnostic);
}

/// `ListExpr`: `[]`, `[:]`, a list, or a map (6.12).
fn list_expr(p: &mut Parser<'_>) -> Expr {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::List;
    let saved_start = p.expr_start;
    let close = p.partner_of(open);
    let mut map: Option<bool> = None;
    if p.at(COLON) && (close == Some(p.pos() + 1) || p.nth(1) == R_BRACK) {
        p.bump();
        map = Some(true);
    } else {
        delimited(p, open, "element", |p| list_element(p, &mut map));
    }
    p.expr_start = saved_start;
    p.bracket = saved;
    p.close_bracket(open, "the list");
    Expr::new(p.complete(marker, if map == Some(true) { MAP_EXPR } else { LIST_EXPR }))
}

/// One element of a list, or one `key: value` entry of a map. `map` is decided by the first
/// element.
fn list_element(p: &mut Parser<'_>, map: &mut Option<bool>) -> bool {
    let Some(key) = expr(p, Ctx::default()) else { return false };
    let is_entry = p.at(COLON);
    let first = map.is_none();
    if first {
        *map = Some(is_entry);
    }
    if is_entry {
        let marker = key.done.precede(p);
        let colon = p.nth_span(0);
        p.bump();
        expr(p, Ctx::default());
        if *map == Some(false) {
            let done = p.complete(marker, ERROR);
            list_and_map_mixed(p, colon, done.span(), true);
        } else {
            p.complete(marker, MAP_ENTRY);
        }
    } else if *map == Some(true) {
        list_and_map_mixed(p, key.done.span(), key.done.span(), false);
    }
    true
}

#[cold]
fn list_and_map_mixed(p: &mut Parser<'_>, at: Span, element: Span, is_entry: bool) {
    if p.speculating() {
        return;
    }
    let label =
        if is_entry { "a `key: value` entry in a list" } else { "a value without a key in a map" };
    p.push_error(
        Diagnostic::error("a literal can't mix list elements and map entries")
            .with_code(codes::LIST_AND_MAP_MIXED)
            .primary(at, label)
            .secondary(element, "this element")
            .note("the first element decides: `key: value` makes a map, a plain value makes a list")
            .help("make every element a plain value (a list) or a `key: value` entry (a map)"),
    );
}

/// `IfExpr = "if" HeaderExpr Block [ "else" ( Block | IfExpr ) ]` (6.15). An `if` in the
/// `else` branch is read here directly, without postfix operators.
fn if_expr(p: &mut Parser<'_>) -> Expr {
    let marker = p.start();
    p.bump();
    if matches!(p.current(), LET_KW | VAR_KW) {
        if_let(p);
    } else {
        let header = header_expr(p, true);
        stmts::body_block(p, "the `if` condition", header.as_ref());
    }
    if p.at(ELSE_KW) {
        p.bump();
        if p.at(IF_KW) {
            if p.enter() {
                if_expr(p);
                p.leave();
            }
        } else {
            stmts::body_block(p, "`else`", None);
        }
    }
    Expr::new(p.complete(marker, IF_EXPR))
}

/// `if let x = maybe { ... }` (grammar.md, 11): reported, and read as `if maybe != none`.
fn if_let(p: &mut Parser<'_>) {
    let marker = p.start();
    let let_span = p.nth_span(0);
    p.bump();
    super::patterns::binding_pattern(p);
    if p.eat(COLON) {
        types::type_(p, MutRule::Forbidden);
    }
    let value = if p.eat(EQ) { header_expr(p, false) } else { None };
    p.complete(marker, ERROR);
    if !p.speculating() {
        let mut diagnostic = Diagnostic::error("Quell has no `if let`")
            .with_code(codes::IF_LET)
            .primary(let_span, "`let` after `if`")
            .note("after `if maybe != none`, Quell knows the value is there");
        if let Some(value) = value {
            diagnostic = diagnostic.suggest_edits(
                "check it with a normal `if`: `if maybe != none { ... }`",
                vec![
                    Edit {
                        span: Span::new(let_span.start, value.done.start),
                        replacement: String::new(),
                    },
                    Edit { span: Span::at(value.done.end), replacement: " != none".into() },
                ],
            );
        } else {
            diagnostic = diagnostic.help("check it with a normal `if`: `if maybe != none { ... }`");
        }
        p.push_error(diagnostic);
    }
    stmts::body_block(p, "the `if` condition", None);
}

/// `MatchExpr = "match" HeaderExpr "{" [ MatchArm { ArmSep MatchArm } [ ArmSep ] ] "}"`.
fn match_expr(p: &mut Parser<'_>) -> Expr {
    let marker = p.start();
    p.bump();
    let header = header_expr(p, false);
    if p.at_body_brace("the `match` value") {
        arm_list(p);
    } else if !p.speculating() {
        if let Some(name) = header.and_then(|header| header.catch_name) {
            let _ = name;
        }
        if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`{`") {
            let at = Span::at(p.last_end());
            p.push_error(diagnostic.suggest("add the arms in `{ }`", at, " { }"));
        }
    }
    Expr::new(p.complete(marker, MATCH_EXPR))
}

fn arm_list(p: &mut Parser<'_>) {
    if !p.enter() {
        return;
    }
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Block;
    let close = p.list_end(open);
    loop {
        while p.at(NEWLINE) && !stmts::at_list_close(p, close) {
            p.bump();
        }
        if stmts::at_list_close(p, close) {
            break;
        }
        let start = p.pos();
        match_arm(p);
        if p.eat(COMMA) || p.at(NEWLINE) || stmts::at_list_close(p, close) || p.at_soft_break() {
            continue;
        }
        if p.pos() == start {
            p.skip_to_safe_point();
            if p.pos() == start && !stmts::at_list_close(p, close) && !p.at(NEWLINE) {
                p.bump_error();
            }
            continue;
        }
        if super::patterns::can_start_pattern(p.current()) {
            if !p.already_reported_here() && !p.speculating() {
                let span = p.nth_span(0);
                let between = Span::new(p.last_end(), span.start);
                p.push_error(
                    Diagnostic::error("two `match` arms on one line need a comma between them")
                        .with_code(codes::MISSING_SEPARATOR)
                        .primary(span, "a new arm starts here")
                        .suggest("separate the arms with `,`", between, ", "),
                );
            }
            continue;
        }
        if !p.already_reported_here() {
            p.error_expected(
                codes::UNEXPECTED_TOKEN,
                "the end of the arm",
                "put each arm on its own line",
            );
        }
        p.skip_to_safe_point();
    }
    p.bracket = saved;
    p.close_bracket(open, "the `match` arms");
    p.complete(marker, MATCH_ARM_LIST);
    p.leave();
}

/// `MatchArm = Pattern [ "if" OrExpr ] "=>" Expr`.
fn match_arm(p: &mut Parser<'_>) {
    let marker = p.start();
    if super::patterns::pattern(p).is_none() && !p.consumed_since(&marker) {
        p.abandon(marker);
        return;
    }
    if p.at(IF_KW) {
        let guard = p.start();
        p.bump();
        let saved = p.expr_start;
        p.expr_start = p.nth_span(0).start;
        let ctx = Ctx { header: false, cond: true, guard: true };
        if expr_bp(p, 0, ctx, Role::Plain).is_none() && !p.already_reported_here() {
            p.error_expected(
                codes::EXPECTED_EXPRESSION,
                "a condition",
                "write the guard's condition after `if`",
            );
        }
        p.expr_start = saved;
        p.complete(guard, MATCH_GUARD);
    }
    if p.eat(FAT_ARROW) {
        if body_expr(p, Ctx::default()).is_none() && !p.already_reported_here() {
            p.error_expected(
                codes::EXPECTED_EXPRESSION,
                "the arm's value",
                "write the value after `=>`",
            );
        }
    } else {
        p.error_missing_token("=>", "`=>`");
        if can_start_expr(p.current()) && !p.nth_line_break_before(0) {
            body_expr(p, Ctx::default());
        }
    }
    p.complete(marker, MATCH_ARM);
}

// ----------------------------------------------------------------------
// Strings (6.13)
// ----------------------------------------------------------------------

/// A string literal that was read: its node and, for each hole, the spans of its `{` and `}`.
#[derive(Debug)]
pub(crate) struct StringInfo {
    pub(crate) done: Done,
    holes: Vec<(Span, Span)>,
    /// A hole holds another string, so its braces can't simply be doubled.
    nested: bool,
}

/// `STRING`: the tokens from `STRING_START` to its `STRING_END`, with the expression of each
/// hole parsed.
pub(crate) fn string_literal(p: &mut Parser<'_>) -> StringInfo {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let close = p.partner_of(open);
    let mut holes = Vec::new();
    let mut nested = false;
    loop {
        if close.is_some_and(|close| p.pos() >= close) {
            break;
        }
        match p.current() {
            STRING_TEXT | STRING_ESCAPE | STRING_INDENT | STRING_LINE_BREAK => p.bump(),
            HOLE_START => {
                let (open_span, close_span, inner) = hole(p);
                holes.push((open_span, close_span));
                nested |= inner;
            }
            EOF | STRING_END => break,
            _ => p.bump_error(),
        }
    }
    if p.at(STRING_END) {
        p.bump();
    }
    StringInfo { done: p.complete(marker, STRING_LITERAL), holes, nested }
}

/// One hole: `{`, exactly one expression, an optional format spec, `}`.
fn hole(p: &mut Parser<'_>) -> (Span, Span, bool) {
    let marker = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    let close = p.partner_of(open);
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    let saved_start = p.expr_start;
    let inner_strings_before = count_strings(p, open, close);
    if !matches!(p.current(), HOLE_END | FORMAT_SPEC | EOF) {
        // The lexer reported an empty hole, so only a real expression is parsed here.
        if let Some(value) = expr(p, Ctx::default()) {
            if !matches!(p.current(), HOLE_END | FORMAT_SPEC | EOF) {
                several_values(p, value, close);
            }
        }
    }
    p.expr_start = saved_start;
    p.bracket = saved;
    // Anything left before the end of the hole.
    if let Some(close) = close {
        while p.pos() < close && !p.at(FORMAT_SPEC) {
            let start = p.pos();
            p.skip_to_safe_point();
            if p.pos() == start {
                p.bump_error();
            }
        }
    }
    p.eat(FORMAT_SPEC);
    let close_span = p.nth_span(0);
    if p.at(HOLE_END) {
        p.bump();
    }
    p.complete(marker, HOLE);
    (open_span, close_span, inner_strings_before)
}

/// Whether the hole from `open` to `close` holds a string literal.
fn count_strings(p: &Parser<'_>, open: usize, close: Option<usize>) -> bool {
    let Some(close) = close else { return false };
    (open + 1..close).any(|pos| p.kind_at(pos) == STRING_START)
}

#[cold]
fn several_values(p: &mut Parser<'_>, first: Expr, close: Option<usize>) {
    if p.speculating() || p.already_reported_here() {
        return;
    }
    let rest = p.nth_span(0);
    let end = close.map_or(rest.end, |close| p.span_at(close).start);
    let mut diagnostic = Diagnostic::error("only one value can go inside `{ }`")
        .with_code(codes::SEVERAL_VALUES_IN_HOLE)
        .primary(Span::new(rest.start, end.max(rest.start)), "more code after the value")
        .secondary(first.done.span(), "the value");
    if can_start_expr(p.current()) && !matches!(p.current(), L_BRACE) {
        diagnostic = diagnostic.suggest_edits(
            "give each value its own `{ }`",
            vec![
                Edit { span: Span::at(first.done.end), replacement: "}".into() },
                Edit { span: Span::at(rest.start), replacement: "{".into() },
            ],
        );
    } else {
        diagnostic =
            diagnostic.help("remove the extra code, or join the values into one expression");
    }
    p.push_error(diagnostic);
}

/// Reports the holes of a string that must be plain: a test name, an `import c` header, a
/// pattern (3.9, 3.10, 8.1).
pub(crate) fn require_plain(p: &mut Parser<'_>, string: &StringInfo, what: &str) {
    if string.holes.is_empty() || p.speculating() {
        return;
    }
    let (first_open, first_close) = string.holes[0];
    let mut diagnostic = Diagnostic::error(format!("{what} can't contain `{{ }}` values"))
        .with_code(codes::HOLE_IN_PLAIN_STRING)
        .primary(first_open.to(first_close), "a `{ }` value")
        .note("this string is read when the program is compiled, so it must be plain text");
    if string.nested {
        diagnostic =
            diagnostic.help("remove the `{ }` value, or write `{{` and `}}` for literal braces");
    } else {
        let edits = string
            .holes
            .iter()
            .flat_map(|&(open, close)| {
                [
                    Edit { span: open, replacement: "{{".into() },
                    Edit { span: close, replacement: "}}".into() },
                ]
            })
            .collect();
        diagnostic = diagnostic.suggest_edits("for literal braces, write `{{` and `}}`", edits);
    }
    p.push_error(diagnostic);
}

// ----------------------------------------------------------------------
// Lambdas and jumps (6.18, 6.19)
// ----------------------------------------------------------------------

/// `LambdaExpr = LambdaParams "=>" Expr`.
fn lambda(p: &mut Parser<'_>) -> Option<Expr> {
    if !p.enter() {
        return None;
    }
    let marker = p.start();
    let mut names = Vec::new();
    if p.at(L_PAREN) {
        let list = p.start();
        let open = p.pos();
        p.bump();
        let saved = p.bracket;
        p.bracket = Bracket::Other;
        delimited(p, open, "parameter", |p| lambda_param(p, &mut names));
        p.bracket = saved;
        p.close_bracket(open, "the lambda's parameters");
        p.complete(list, LAMBDA_PARAM_LIST);
    } else {
        lambda_param(p, &mut names);
    }
    let body_start = p.pos();
    if p.eat(FAT_ARROW) {
        if body_expr(p, Ctx::default()).is_none() && !p.already_reported_here() {
            p.error_expected(
                codes::EXPECTED_EXPRESSION,
                "the lambda's body",
                "write the lambda's value after `=>`",
            );
        }
    } else {
        p.error_missing_token("=>", "`=>`");
    }
    let done = p.complete(marker, LAMBDA_EXPR);
    for name in names {
        lambda_param_name(p, name, body_start);
    }
    p.leave();
    Some(Expr::new(done))
}

/// `LambdaParam = [ "mut" ] LambdaName [ ":" Type ]`.
fn lambda_param(p: &mut Parser<'_>, names: &mut Vec<Span>) -> bool {
    if !matches!(p.current(), MUT_KW | UPPER_IDENT | LOWER_IDENT | UNDERSCORE) {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a parameter name",
            "a lambda parameter is a name, as in `(a, b) => a + b`",
        );
        return false;
    }
    let marker = p.start();
    p.eat(MUT_KW);
    let name = p.nth_span(0);
    if p.current().is_ident() {
        names.push(name);
        p.bump();
    } else if !p.eat(UNDERSCORE) {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a parameter name",
            "write the parameter's name after `mut`",
        );
    }
    if p.eat(COLON) {
        types::type_(p, MutRule::Lambda { name });
    }
    p.complete(marker, LAMBDA_PARAM);
    true
}

/// The naming check for a lambda parameter. Its fix renames the parameter and every use of it
/// in the lambda's body, as in `item => item.name` (6.18).
fn lambda_param_name(p: &mut Parser<'_>, name: Span, body_start: usize) {
    let text = p.text_of(name).to_string();
    if !crate::is_upper_name(&text) || p.speculating() {
        return;
    }
    let Some(fixed) = super::fixes::to_lower_name(&text) else {
        p.check_name(name, Case::Lower, "lambda parameter names");
        return;
    };
    let mut edits = vec![Edit { span: name, replacement: fixed.clone() }];
    let mut pos = body_start;
    while pos < p.pos() {
        let span = p.span_at(pos);
        let is_use = p.kind_at(pos) == UPPER_IDENT
            && p.text_of(span) == text
            && !(pos > 0 && matches!(p.kind_at(pos - 1), DOT | QUESTION_DOT))
            && p.kind_at(pos + 1) != COLON;
        if is_use {
            edits.push(Edit { span, replacement: fixed.clone() });
        }
        pos += 1;
    }
    p.push_error(
        Diagnostic::error("lambda parameter names start with a lowercase letter")
            .with_code(codes::NAME_NEEDS_LOWERCASE)
            .primary(name, "this name starts with a capital letter")
            .note("only types, enum options and generic parameters start with a capital")
            .suggest_edits(format!("write `{fixed}`"), edits),
    );
}

/// Where a jump is: a whole expression, the right side of `??` or `catch`, or somewhere it is
/// not allowed (with the binding power of that place).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JumpPos {
    Full,
    Fallback,
    Misplaced(u8),
}

/// A jump that was read, and where its value starts.
#[derive(Clone, Copy, Debug)]
struct Jump {
    expr: Expr,
    value: Option<super::Checkpoint>,
}

/// `JumpExpr = "return" [ Expr ] | "throw" Expr | "break" | "continue"` (6.19), and
/// `FallbackJump` (6.6), whose value stops at the level of `??`.
fn jump(p: &mut Parser<'_>, ctx: Ctx, pos: JumpPos) -> Jump {
    let marker = p.start();
    let kind = p.current();
    let keyword = p.nth_span(0);
    p.bump();
    let takes_value = can_start_expr(p.current()) && !p.at(L_BRACE);
    let mut value_start = None;
    match kind {
        RETURN_KW | THROW_KW => {
            if takes_value {
                value_start = Some(p.checkpoint());
                let next_line = p.nth_line_break_before(0);
                let value = match pos {
                    JumpPos::Full => expr(p, ctx),
                    JumpPos::Fallback => expr_bp(p, BP_COALESCE, ctx, Role::Plain),
                    JumpPos::Misplaced(bp) => expr_bp(p, bp.max(BP_COALESCE), ctx, Role::Plain),
                };
                if let (true, Some(value)) = (next_line, value) {
                    jump_value_on_next_line(p, keyword, value);
                }
            } else if kind == THROW_KW && !p.already_reported_here() {
                p.error_expected(
                    codes::EXPECTED_EXPRESSION,
                    "the error to throw",
                    "`throw` needs a value, as in `throw Empty`",
                );
            }
        }
        _ => {
            if takes_value && !p.nth_line_break_before(0) {
                let value = p.start();
                expr_bp(p, BP_COALESCE, ctx, Role::Plain);
                let done = p.complete(value, ERROR);
                jump_with_value(p, keyword, done);
            }
        }
    }
    let node = match kind {
        RETURN_KW => RETURN_EXPR,
        THROW_KW => THROW_EXPR,
        BREAK_KW => BREAK_EXPR,
        _ => CONTINUE_EXPR,
    };
    Jump { expr: Expr::new(p.complete(marker, node)), value: value_start }
}

#[cold]
fn jump_with_value(p: &mut Parser<'_>, keyword: Span, value: Done) {
    if p.speculating() {
        return;
    }
    let word = p.text_of(keyword).to_string();
    p.push_error(
        Diagnostic::error(format!("`{word}` doesn't take a value"))
            .with_code(codes::JUMP_WITH_VALUE)
            .primary(value.span(), "a value after `{word}`".replace("{word}", &word))
            .note("loops in Quell give no value, and loop labels are not part of the language")
            .suggest("remove the value", Span::new(keyword.end, value.end), ""),
    );
}

#[cold]
fn jump_value_on_next_line(p: &mut Parser<'_>, keyword: Span, value: Expr) {
    if p.speculating() {
        return;
    }
    let word = p.text_of(keyword).to_string();
    let value_text = p.text_of(value.done.span()).to_string();
    let shown = if value_text.len() <= 30 && !value_text.contains('\n') {
        format!("`{value_text}`")
    } else {
        "the value".to_string()
    };
    let between = Span::new(keyword.end, value.done.start);
    let only_space = p.text_of(between).chars().all(char::is_whitespace);
    let diagnostic =
        Diagnostic::warning(format!("this `{word}` takes {shown} from the next line as its value"))
            .with_code(codes::JUMP_VALUE_ON_NEXT_LINE)
            .primary(keyword, "the value is on the next line")
            .secondary(value.done.span(), "this is the value")
            .note("inside `( )` and `[ ]` line breaks are ignored");
    let diagnostic = if only_space {
        diagnostic.suggest("put them on one line to make that clear", between, " ")
    } else {
        diagnostic.help("put them on one line to make that clear")
    };
    p.push_error(diagnostic);
}

/// An assignment where a value was expected (4.4).
fn misplaced_assignment(p: &mut Parser<'_>, value: Expr, after: AfterValue) -> Expr {
    let op = p.nth_span(0);
    if after == AfterValue::Body {
        // Read as if the braces were there: a block holding the assignment.
        let block = value.done.precede(p);
        let assign = value.done.precede(p);
        p.bump();
        let rhs = expr(p, Ctx::default());
        p.complete(assign, ASSIGN_STMT);
        let done = p.complete(block, BLOCK);
        if !p.speculating() {
            let end = rhs.map_or(op.end, |rhs| rhs.done.end);
            p.push_error(
                Diagnostic::error("an assignment needs braces here")
                    .with_code(codes::ASSIGNMENT_AS_BODY)
                    .primary(op, "an assignment as the value of `=>`")
                    .note("an assignment is a statement, so it can't be the value of a `match` arm or a lambda")
                    .suggest_edits(
                        "put it in a block: `=> { ... }`",
                        vec![
                            Edit { span: Span::at(value.done.start), replacement: "{ ".into() },
                            Edit { span: Span::at(end), replacement: " }".into() },
                        ],
                    ),
            );
        }
        return Expr::new(done);
    }
    let marker = value.done.precede(p);
    p.bump();
    let rhs = expr(p, Ctx::default());
    let done = p.complete(marker, ERROR);
    if !p.speculating() {
        let mut diagnostic = Diagnostic::error("an assignment can't be used as a value")
            .with_code(codes::ASSIGNMENT_AS_VALUE)
            .primary(op, "an assignment inside a value")
            .note("an assignment is a statement: it gives no value and can't be nested");
        if p.text_of(op) == "=" {
            diagnostic = diagnostic.suggest("to compare the two values, use `==`", op, "==");
        }
        let _ = rhs;
        p.push_error(diagnostic.help("do the assignment on its own line first"));
    }
    Expr::new(done)
}
