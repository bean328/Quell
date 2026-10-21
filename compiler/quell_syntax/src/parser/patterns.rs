//! Patterns (spec/grammar.md, section 8).

use quell_diagnostics::{Diagnostic, Span};

use super::exprs;
use super::fixes::{Case, to_upper_name};
use super::{Bracket, Done, Parser, codes};
use crate::SyntaxKind::*;

/// Whether a pattern can start with this token.
pub(crate) fn can_start_pattern(kind: crate::SyntaxKind) -> bool {
    matches!(
        kind,
        UNDERSCORE
            | UPPER_IDENT
            | LOWER_IDENT
            | MINUS
            | INT
            | FLOAT
            | CHAR
            | STRING_START
            | TRUE_KW
            | FALSE_KW
            | NONE_KW
            | L_PAREN
            | L_BRACK
    )
}

/// `Pattern = PatternAtom { "or" PatternAtom }`.
pub(crate) fn pattern(p: &mut Parser<'_>) -> Option<Done> {
    if !p.enter() {
        return None;
    }
    let marker = p.start();
    let first = pattern_atom(p);
    if first.is_some() && matches!(p.current(), OR_KW | PIPE) {
        while matches!(p.current(), OR_KW | PIPE) {
            if p.at(PIPE) {
                bar_between_patterns(p);
            }
            p.bump();
            if pattern_atom(p).is_none() {
                break;
            }
        }
        p.leave();
        return Some(p.complete(marker, OR_PAT));
    }
    p.abandon(marker);
    p.leave();
    first
}

#[cold]
fn bar_between_patterns(p: &mut Parser<'_>) {
    if p.speculating() {
        return;
    }
    let span = p.nth_span(0);
    p.push_error(
        Diagnostic::error("patterns are joined with `or`, not `|`")
            .with_code(codes::BAR_BETWEEN_PATTERNS)
            .primary(span, "`|` between two patterns")
            .suggest("write `or`", span, "or"),
    );
}

fn pattern_atom(p: &mut Parser<'_>) -> Option<Done> {
    let kind = p.current();
    match kind {
        UNDERSCORE => {
            let marker = p.start();
            p.bump();
            Some(p.complete(marker, WILDCARD_PAT))
        }
        LOWER_IDENT => {
            let dotted = p.nth(1) == DOT && p.nth(2).is_ident();
            let colons = p.nth(1) == COLON && p.nth(2) == COLON && p.nth_is_joined(2);
            if dotted || colons {
                return Some(option_pattern(p));
            }
            if p.nth(1) == L_PAREN && !p.nth_line_break_before(1) {
                lowercase_name_with_payload(p);
                return Some(option_pattern(p));
            }
            let marker = p.start();
            p.bump();
            Some(p.complete(marker, BIND_PAT))
        }
        UPPER_IDENT => Some(option_pattern(p)),
        MINUS | INT | FLOAT | CHAR => literal_or_range(p),
        STRING_START => {
            let marker = p.start();
            let string = exprs::string_literal(p);
            exprs::require_plain(p, &string, "a pattern");
            Some(p.complete(marker, LITERAL_PAT))
        }
        TRUE_KW | FALSE_KW | NONE_KW => {
            let marker = p.start();
            p.bump();
            Some(p.complete(marker, LITERAL_PAT))
        }
        L_PAREN => Some(paren_pattern(p)),
        L_BRACK => Some(list_pattern(p)),
        DOT if p.nth(1) == UPPER_IDENT && p.nth_is_joined(1) => {
            let marker = p.start();
            dot_option(p);
            option_path(p);
            payload(p);
            Some(p.complete(marker, OPTION_PAT))
        }
        _ => {
            if !p.already_reported_here() {
                p.error_expected(
                    codes::EXPECTED_PATTERN,
                    "a pattern",
                    "a pattern is a value to match, such as `0`, `\"add\"`, `Sunny`, `Rainy(mm)`, `(a, b)` or `_`",
                );
            }
            None
        }
    }
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
            Diagnostic::error("a pattern can't start with `.`")
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

#[cold]
fn lowercase_name_with_payload(p: &mut Parser<'_>) {
    if p.speculating() {
        return;
    }
    let span = p.nth_span(0);
    let name = p.nth_text(0).to_string();
    let diagnostic = Diagnostic::error("a new name can't have `( )`")
        .with_code(codes::LOWERCASE_NAME_WITH_PAYLOAD)
        .primary(span, "a lowercase name in a pattern is a new name")
        .note("in a pattern, a name that starts with a capital letter is an enum option, and a lowercase name binds the value");
    let diagnostic = match to_upper_name(&name) {
        Some(fixed) => diagnostic.suggest(
            format!("option names start with a capital letter: write `{fixed}`"),
            span,
            fixed,
        ),
        None => diagnostic.help("option names start with a capital letter"),
    };
    p.push_error(diagnostic);
}

/// `RefPattern = RefPath [ "(" SubPattern { "," SubPattern } [ "," ] ")" ]`.
fn option_pattern(p: &mut Parser<'_>) -> Done {
    let marker = p.start();
    option_path(p);
    payload(p);
    p.complete(marker, OPTION_PAT)
}

/// `RefPath = UPPER_IDENT | IDENT "." IDENT { "." IDENT }`.
fn option_path(p: &mut Parser<'_>) {
    p.bump();
    loop {
        if p.at(DOT) && p.nth(1).is_ident() {
            p.bump();
            p.bump();
        } else if p.at(COLON) && p.nth(1) == COLON && p.nth_is_joined(1) && p.nth(2).is_ident() {
            exprs::double_colon(p);
            p.bump();
        } else {
            break;
        }
    }
}

fn payload(p: &mut Parser<'_>) {
    if !p.at(L_PAREN) || p.nth_line_break_before(0) {
        return;
    }
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    exprs::delimited(p, open, "field pattern", sub_pattern);
    p.bracket = saved;
    p.close_bracket(open, "the option's fields");
    p.complete(marker, PAYLOAD_LIST);
}

/// `SubPattern = [ LOWER_IDENT ":" ] Pattern`.
fn sub_pattern(p: &mut Parser<'_>) -> bool {
    if p.current().is_ident() && p.nth(1) == COLON && !(p.nth(2) == COLON && p.nth_is_joined(2)) {
        let marker = p.start();
        let name = p.nth_span(0);
        p.check_name(name, Case::Lower, "field names");
        p.bump();
        p.bump();
        pattern(p);
        p.complete(marker, PAYLOAD_FIELD);
        return true;
    }
    pattern(p).is_some()
}

/// A literal pattern or a range pattern (8.1).
fn literal_or_range(p: &mut Parser<'_>) -> Option<Done> {
    let bound = literal_bound(p)?;
    if !matches!(p.current(), DOT_DOT_LT | DOT_DOT_EQ) {
        return Some(bound);
    }
    let marker = bound.precede(p);
    p.bump();
    if matches!(p.current(), MINUS | INT | FLOAT | CHAR) {
        literal_bound(p);
    } else {
        p.error_expected(
            codes::EXPECTED_PATTERN,
            "the end of the range",
            "a range pattern needs both ends, as in `1..=5`",
        );
    }
    Some(p.complete(marker, RANGE_PAT))
}

/// `[ "-" ] ( INT | FLOAT ) | CHAR`.
fn literal_bound(p: &mut Parser<'_>) -> Option<Done> {
    let marker = p.start();
    if p.eat(MINUS) {
        if matches!(p.current(), INT | FLOAT) {
            p.bump();
        } else {
            p.error_expected(
                codes::EXPECTED_PATTERN,
                "a number after `-`",
                "write a number, as in `-1`",
            );
        }
    } else {
        p.bump();
    }
    Some(p.complete(marker, LITERAL_PAT))
}

/// `ParenPattern = "(" Pattern ")" | "(" Pattern "," Pattern { "," Pattern } [ "," ] ")"`.
fn paren_pattern(p: &mut Parser<'_>) -> Done {
    let marker = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    let (count, trailing) = exprs::delimited(p, open, "pattern", |p| pattern(p).is_some());
    p.bracket = saved;
    let close_span = p.nth_span(0);
    p.close_bracket(open, "the parentheses");
    let kind = match (count, trailing) {
        (0, _) => {
            if p.partner_of(open).is_some() {
                exprs::unit_or_one_tuple(p, open_span.to(close_span), None, "a pattern");
            }
            TUPLE_PAT
        }
        (1, Some(comma)) => {
            exprs::unit_or_one_tuple(p, comma, Some(comma), "a pattern");
            PAREN_PAT
        }
        (1, None) => PAREN_PAT,
        _ => TUPLE_PAT,
    };
    p.complete(marker, kind)
}

/// `ListPattern = "[" [ Pattern { "," Pattern } [ "," ] ] "]"`.
fn list_pattern(p: &mut Parser<'_>) -> Done {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    exprs::delimited(p, open, "pattern", |p| pattern(p).is_some());
    p.bracket = saved;
    p.close_bracket(open, "the list pattern");
    p.complete(marker, LIST_PAT)
}

/// `BindingPattern = LOWER_IDENT | "_" | "(" BindingPattern "," BindingPattern { ","
/// BindingPattern } [ "," ] ")"` (8.5), used by `let`, `var`, `for` and `parallel for`.
pub(crate) fn binding_pattern(p: &mut Parser<'_>) -> Option<Done> {
    if !p.enter() {
        return None;
    }
    let result = binding_pattern_inner(p);
    p.leave();
    result
}

fn binding_pattern_inner(p: &mut Parser<'_>) -> Option<Done> {
    let kind = p.current();
    let path = p.nth(1) == DOT || (p.nth(1) == COLON && p.nth(2) == COLON && p.nth_is_joined(2));
    let simple_name = kind.is_ident() && !path && p.nth(1) != L_PAREN;
    if simple_name {
        let marker = p.start();
        let name = p.nth_span(0);
        p.check_name(name, Case::Lower, "names of values");
        p.bump();
        return Some(p.complete(marker, BIND_PAT));
    }
    match kind {
        UNDERSCORE => {
            let marker = p.start();
            p.bump();
            Some(p.complete(marker, WILDCARD_PAT))
        }
        L_PAREN => {
            let marker = p.start();
            let open = p.pos();
            let open_span = p.nth_span(0);
            p.bump();
            let saved = p.bracket;
            p.bracket = Bracket::Other;
            let (count, trailing) =
                exprs::delimited(p, open, "name", |p| binding_pattern(p).is_some());
            p.bracket = saved;
            let close_span = p.nth_span(0);
            let closed = p.partner_of(open).is_some();
            p.close_bracket(open, "the parentheses");
            let kind = match (count, trailing) {
                (0, _) => {
                    if closed {
                        exprs::unit_or_one_tuple(p, open_span.to(close_span), None, "a pattern");
                    }
                    TUPLE_PAT
                }
                (1, Some(comma)) => {
                    exprs::unit_or_one_tuple(p, comma, Some(comma), "a pattern");
                    PAREN_PAT
                }
                (1, None) => {
                    if closed {
                        single_name_in_parentheses(p, open_span, close_span);
                    }
                    PAREN_PAT
                }
                _ => TUPLE_PAT,
            };
            Some(p.complete(marker, kind))
        }
        _ if can_start_pattern(kind) || kind == DOT => {
            let full = pattern(p)?;
            not_a_binding_pattern(p, full.span());
            Some(full)
        }
        _ => {
            if !p.already_reported_here() {
                p.error_expected(
                    codes::EXPECTED_NAME,
                    "a name",
                    "write the name of the new value, as in `let total = 0`",
                );
            }
            None
        }
    }
}

#[cold]
fn single_name_in_parentheses(p: &mut Parser<'_>, open: Span, close: Span) {
    if p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error("a single name doesn't go in parentheses here")
            .with_code(codes::NOT_A_BINDING_PATTERN)
            .primary(open.to(close), "parentheses around one name")
            .note("parentheses take a tuple apart, so they hold at least two names: `let (a, b) = pair`")
            .suggest_edits(
                "remove the parentheses",
                vec![
                    quell_diagnostics::Edit { span: open, replacement: String::new() },
                    quell_diagnostics::Edit { span: close, replacement: String::new() },
                ],
            ),
    );
}

#[cold]
fn not_a_binding_pattern(p: &mut Parser<'_>, span: Span) {
    if p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error("this pattern might not match")
            .with_code(codes::NOT_A_BINDING_PATTERN)
            .primary(span, "`let`, `var` and `for` need a pattern that always matches")
            .note("these take names, `_` and tuples of them, as in `let (low, high) = pair`")
            .help("use `match` to check a value against this pattern"),
    );
}
