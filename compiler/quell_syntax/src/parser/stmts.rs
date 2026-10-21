//! Blocks and statements (spec/grammar.md, section 4), and the top-level list of a file.

use quell_diagnostics::{Diagnostic, Span};

use super::exprs::{self, Expr};
use super::fixes::{indentation_at, wrap_in_parens};
use super::items::{self, Container, with_space_after};
use super::patterns;
use super::types::{self, MutRule};
use super::{Bracket, Ctx, Done, Parser, codes};
use crate::FileRole;
use crate::SyntaxKind::{self, *};

/// What the statement-list loop remembers about a statement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StmtInfo {
    kind: SyntaxKind,
    span: Span,
    /// The statement's last token is `}` (grammar.md, 4.10).
    ends_with_brace: bool,
    /// The statement is a `return`, `throw`, `break` or `continue` on its own.
    jump: bool,
}

/// A block that was read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockInfo {
    pub(crate) done: Done,
    /// When the block holds exactly one statement and it is a jump, where that jump is.
    pub(crate) only_jump: Option<Span>,
}

/// Whether the list that the opener with partner `close` holds ends here: at its closer, or,
/// for an opener that is never closed, where an enclosing construct goes on.
pub(crate) fn at_list_close(p: &Parser<'_>, close: Option<usize>) -> bool {
    at_list_close_at(p, close, p.pos())
}

pub(crate) fn at_list_close_at(p: &Parser<'_>, close: Option<usize>, pos: usize) -> bool {
    if let Some(close) = close {
        return pos >= close;
    }
    match p.kind_at(pos) {
        EOF | HOLE_END | FORMAT_SPEC | STRING_END => true,
        R_PAREN | R_BRACK | R_BRACE => p.partner_of(pos).is_some(),
        _ => false,
    }
}

/// `Block = "{" [ Statement { NEWLINE Statement } [ NEWLINE ] ] "}"` (4.1). Expects to be at
/// the `{`. Returns `None` only when the block is nested too deeply to read.
pub(crate) fn block(p: &mut Parser<'_>) -> Option<BlockInfo> {
    if !p.enter() {
        return None;
    }
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Block;
    let only_jump = statement_list(p, Some(open));
    p.bracket = saved;
    p.close_bracket(open, "the block");
    p.leave();
    Some(BlockInfo { done: p.complete(marker, BLOCK), only_jump })
}

/// The body of a keyword construct: a block whose `{` must be on the same line (lexical.md,
/// 12.6). `after` names what comes before the `{`, for the messages. When the body is missing
/// and the header ends with the binding form of `catch`, the message says how to use the name
/// as a fallback instead (grammar.md, 10.4).
pub(crate) fn body_block(
    p: &mut Parser<'_>,
    after: &str,
    header: Option<&Expr>,
) -> Option<BlockInfo> {
    if p.at_body_brace(after) {
        return block(p);
    }
    if p.speculating() {
        return None;
    }
    if let Some(name) = header.and_then(|header| header.catch_name) {
        let header = header.map_or(name, |header| header.done.span());
        binding_catch_in_header(p, header, name);
        return None;
    }
    if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`{`") {
        let at = Span::at(p.last_end());
        p.push_error(diagnostic.suggest("add the body in `{ }`", at, " { }"));
    }
    None
}

#[cold]
fn binding_catch_in_header(p: &mut Parser<'_>, header: Span, name: Span) {
    let name_text = p.text_of(name).to_string();
    let at = p.missing_span();
    p.push_error(
        Diagnostic::error("this needs a body in `{ }`, but the block was read as part of `catch`")
            .with_code(codes::BINDING_CATCH_IN_HEADER)
            .primary(at, "the body should start here")
            .secondary(name, format!("`catch {name_text} {{` reads `{name_text}` as the name of the error"))
            .suggest_edits(
                format!(
                    "to use `{name_text}` as the fallback value here, wrap the condition in parentheses"
                ),
                wrap_in_parens(header.start, name.end),
            ),
    );
}

/// The statements of a block (`open` is the position of its `{`) or, with `open` set to
/// `None`, the items and statements of a file. Returns the span of the only statement when
/// there is exactly one and it is a jump.
pub(crate) fn statement_list(p: &mut Parser<'_>, open: Option<usize>) -> Option<Span> {
    let top = open.is_none();
    let close = open.and_then(|open| p.partner_of(open));
    let mut previous: Option<StmtInfo> = None;
    let mut count = 0;
    loop {
        while p.at(NEWLINE) {
            p.bump();
        }
        if top {
            if p.at(EOF) {
                break;
            }
            if matches!(
                p.current(),
                R_PAREN | R_BRACK | R_BRACE | HOLE_END | FORMAT_SPEC | STRING_END
            ) {
                // A closer of nothing (the lexer reported it).
                p.bump_error();
                continue;
            }
        } else {
            if at_list_close(p, close) {
                break;
            }
            if matches!(p.current(), R_PAREN | R_BRACK | R_BRACE) && p.nth_partner(0).is_none() {
                p.bump_error();
                continue;
            }
        }
        let start = p.pos();
        let starts_with_minus = matches!(p.current(), MINUS | TILDE);
        let info = statement(p, top);
        if let (true, Some(previous), Some(info)) = (starts_with_minus, previous, info) {
            line_starts_with_minus(p, previous, info);
        }
        if p.pos() == start {
            // Nothing could be read here.
            if !p.already_reported_here() {
                p.error_expected(
                    codes::UNEXPECTED_TOKEN,
                    if top { "an item or a statement" } else { "a statement" },
                    "remove this, or complete the code around it",
                );
            }
            p.skip_to_safe_point();
            if p.pos() == start && !p.at(NEWLINE) && !p.at(EOF) {
                if !top && at_list_close(p, close) {
                    break;
                }
                p.bump_error();
            }
            previous = None;
            continue;
        }
        count += 1;
        previous = info;
        // After a statement comes a line break or the end of the block (4.1).
        if p.at(NEWLINE) || p.at(EOF) || p.at_soft_break() {
            continue;
        }
        if !top && at_list_close(p, close) {
            continue;
        }
        if top && matches!(p.current(), R_PAREN | R_BRACK | R_BRACE) {
            continue;
        }
        if can_start_statement(p) {
            statements_on_one_line(p, info.map(|info| info.span.start));
            continue;
        }
        if !p.already_reported_here() {
            p.error_expected(
                codes::UNEXPECTED_TOKEN,
                "the end of the line",
                "a statement ends at the end of its line; remove what follows it",
            );
        }
        p.skip_to_safe_point();
    }
    match previous {
        Some(info) if count == 1 && info.jump => Some(info.span),
        _ => None,
    }
}

/// Whether the current token can start a statement or an item.
pub(crate) fn can_start_statement(p: &Parser<'_>) -> bool {
    exprs::can_start_expr(p.current())
        || matches!(
            p.current(),
            LET_KW | VAR_KW | FOR_KW | WHILE_KW | REPEAT_KW | DEFER_KW | ASSERT_KW
        )
        || items::at_item_start(p, true)
}

#[cold]
fn statements_on_one_line(p: &mut Parser<'_>, statement_start: Option<u32>) {
    if p.already_reported_here() || p.speculating() {
        return;
    }
    let span = p.nth_span(0);
    let indent = indentation_at(p.text(), statement_start.unwrap_or(span.start)).to_string();
    let between = Span::new(p.last_end(), span.start);
    p.push_error(
        Diagnostic::error("put each statement on its own line")
            .with_code(codes::STATEMENTS_ON_ONE_LINE)
            .primary(span, "a second statement starts here")
            .note("Quell has no semicolons: a line break ends a statement")
            .suggest("start a new line here", between, format!("\n{indent}")),
    );
}

/// The warning for a statement that starts with `-` or `~` and is most likely the second half
/// of a split line (grammar.md, 4.10; lexical.md, 12.6).
fn line_starts_with_minus(p: &mut Parser<'_>, previous: StmtInfo, statement: StmtInfo) {
    let continues = matches!(previous.kind, EXPR_STMT | LET_STMT | GLOBAL_LET | ASSIGN_STMT);
    if !continues || previous.ends_with_brace || statement.kind != EXPR_STMT || p.speculating() {
        return;
    }
    let text = p.text();
    let here = indentation_at(text, statement.span.start).chars().count();
    let above = indentation_at(text, previous.span.start).chars().count();
    if here <= above {
        return;
    }
    let operator_span = Span::new(statement.span.start, statement.span.start + 1);
    let operator = p.text_of(operator_span).to_string();
    let value = p.text_of(statement.span).to_string();
    let compact: String = value.split_whitespace().collect::<Vec<_>>().join("");
    let example = if compact.len() <= 30 && !compact.contains('\n') {
        compact
    } else {
        format!("{operator}x")
    };
    let mut diagnostic = Diagnostic::warning(format!(
        "this line starts with `{operator}`, so it is a new statement"
    ))
    .with_code(codes::LINE_STARTS_WITH_MINUS)
    .primary(operator_span, "this starts a new statement")
    .secondary(previous.span, "this statement ends at the end of its line")
    .note("a line break ends a statement, unless the line ends with an operator");
    if operator == "-" {
        diagnostic = diagnostic.suggest_edits(
            "to continue the line above, end that line with `-`",
            vec![
                quell_diagnostics::Edit {
                    span: Span::at(previous.span.end),
                    replacement: " -".into(),
                },
                quell_diagnostics::Edit {
                    span: with_space_after(p, operator_span),
                    replacement: String::new(),
                },
            ],
        );
    }
    diagnostic = diagnostic.suggest_edits(
        format!("to make this value here on purpose, write `({example})`"),
        wrap_in_parens(statement.span.start, statement.span.end),
    );
    p.push_error(diagnostic);
}

/// `Statement = LetStmt | AssignStmt | ForStmt | WhileStmt | RepeatStmt | DeferStmt |
/// AssertStmt | ExprStmt` (4.2), or at the top level an item.
fn statement(p: &mut Parser<'_>, top: bool) -> Option<StmtInfo> {
    let start = p.nth_span(0).start;
    if items::at_item_start(p, top) {
        let kind = items::item(p, if top { Container::File } else { Container::Block });
        return Some(StmtInfo {
            kind,
            span: Span::new(start, p.last_end().max(start)),
            ends_with_brace: p.last_kind() == R_BRACE,
            jump: false,
        });
    }
    let marker = p.start();
    let kind = match p.current() {
        LET_KW | VAR_KW => {
            let_rest(p);
            LET_STMT
        }
        FOR_KW => {
            for_stmt(p);
            FOR_STMT
        }
        WHILE_KW => {
            p.bump();
            let header = exprs::header_expr(p, true);
            body_block(p, "the `while` condition", header.as_ref());
            WHILE_STMT
        }
        REPEAT_KW => {
            p.bump();
            // After `repeat`, `{` always opens the body (4.7).
            if p.at(L_BRACE) || (p.at(NEWLINE) && p.nth(1) == L_BRACE) {
                body_block(p, "`repeat`", None);
            } else {
                let header = exprs::header_expr(p, false);
                body_block(p, "the `repeat` count", header.as_ref());
            }
            REPEAT_STMT
        }
        DEFER_KW => {
            p.bump();
            body_block(p, "`defer`", None);
            DEFER_STMT
        }
        ASSERT_KW => {
            p.bump();
            exprs::expr(p, Ctx::default());
            if p.eat(COMMA) {
                exprs::expr(p, Ctx::default());
            }
            ASSERT_STMT
        }
        _ => {
            p.abandon(marker);
            return expr_statement(p, top);
        }
    };
    let done = p.complete(marker, kind);
    if top {
        statement_in_library(p, done.span());
    }
    Some(StmtInfo {
        kind,
        span: done.span(),
        ends_with_brace: p.last_kind() == R_BRACE,
        jump: false,
    })
}

/// An expression statement or an assignment (4.4, 4.10, 10.26).
fn expr_statement(p: &mut Parser<'_>, top: bool) -> Option<StmtInfo> {
    let marker = p.start();
    if p.at(RETURN_KW) && p.nth(1) == L_BRACE && !p.nth_line_break_before(1) {
        let jump = return_block(p);
        let done = p.complete(marker, EXPR_STMT);
        if top {
            statement_in_library(p, done.span());
        }
        return Some(StmtInfo { kind: EXPR_STMT, span: done.span(), ends_with_brace: true, jump });
    }
    let Some(lhs) = exprs::full_expr(p, Ctx::default()) else {
        if p.consumed_since(&marker) {
            p.complete(marker, EXPR_STMT);
        } else {
            p.abandon(marker);
        }
        return None;
    };
    let kind = if exprs::at_assign_op(p) {
        if !lhs.place {
            not_a_place(p, &lhs);
        }
        p.bump();
        exprs::expr(p, Ctx::default());
        ASSIGN_STMT
    } else if p.at(QUESTION_QUESTION) && p.nth(1) == EQ {
        coalesce_assignment(p, &lhs);
        ASSIGN_STMT
    } else {
        EXPR_STMT
    };
    let done = p.complete(marker, kind);
    if top {
        statement_in_library(p, done.span());
    }
    let jump = kind == EXPR_STMT
        && matches!(lhs.done.kind, RETURN_EXPR | THROW_EXPR | BREAK_EXPR | CONTINUE_EXPR);
    Some(StmtInfo { kind, span: done.span(), ends_with_brace: p.last_kind() == R_BRACE, jump })
}

/// `return { ... }` as a statement (6.19): reported, and read as `return ({ ... })`.
fn return_block(p: &mut Parser<'_>) -> bool {
    let marker = p.start();
    p.bump(); // `return`
    let open = p.nth_span(0);
    let block = block(p);
    let done = p.complete(marker, RETURN_EXPR);
    if let Some(block) = block {
        if !p.speculating() {
            p.push_error(
                Diagnostic::error("`return` before `{` returns no value")
                    .with_code(codes::RETURN_BLOCK)
                    .primary(open, "this block is not the value of `return`")
                    .note("`return` never takes a value that starts with `{`, so `?? return {` can open a loop's body")
                    .suggest_edits(
                        "to return the value of this block, write `return ({ ... })`",
                        wrap_in_parens(block.done.start, block.done.end),
                    ),
            );
        }
    }
    let _ = done;
    true
}

#[cold]
fn not_a_place(p: &mut Parser<'_>, lhs: &Expr) {
    if p.speculating() {
        return;
    }
    let op = p.nth_span(0);
    let mut diagnostic = Diagnostic::error("you can only assign to a variable, a field or an element")
        .with_code(codes::NOT_A_PLACE)
        .primary(lhs.done.span(), "this can't be given a new value")
        .note("a place is a variable, or a field or element reached from one, as in `self.health` or `ages[\"Kai\"]`");
    if p.at(EQ) {
        diagnostic = diagnostic.suggest("to compare the two values, use `==`", op, "==");
    } else {
        diagnostic = diagnostic.help("assign to a variable instead, and use it here");
    }
    p.push_error(diagnostic);
}

/// `cache ??= load()` (6.6): reported, and read as `cache = cache ?? load()`.
fn coalesce_assignment(p: &mut Parser<'_>, lhs: &Expr) {
    let start = p.nth_span(0);
    let marker = p.start();
    p.bump(); // `??`
    p.complete(marker, ERROR);
    let end = p.nth_span(0);
    p.bump(); // `=`
    if !p.speculating() {
        let place = p.text_of(lhs.done.span()).to_string();
        p.push_error(
            Diagnostic::error("Quell has no `??=`")
                .with_code(codes::COALESCE_ASSIGNMENT)
                .primary(start.to(end), "not an operator in Quell")
                .suggest(
                    format!("write `{place} = {place} ?? ...`"),
                    start.to(end),
                    format!("= {place} ??"),
                ),
        );
    }
    exprs::expr(p, Ctx::default());
}

#[cold]
fn statement_in_library(p: &mut Parser<'_>, span: Span) {
    if p.role != FileRole::Library || p.speculating() {
        return;
    }
    p.push_error(
        Diagnostic::error("code at the top level only runs in the entry file")
            .with_code(codes::STATEMENT_IN_LIBRARY)
            .primary(span, "a statement at the top level of a library file")
            .note("a library file holds only items: functions, types, imports, tests and top-level `let`")
            .help("move it into a function, or into a `test` block"),
    );
}

// ----------------------------------------------------------------------
// let, for (4.3, 4.5)
// ----------------------------------------------------------------------

/// The rest of `LetStmt` or `GlobalLet`, from `let` or `var`:
/// `BindingPattern [ ":" Type ] "=" Expr`.
pub(crate) fn let_rest(p: &mut Parser<'_>) {
    p.bump(); // `let` or `var`
    let pattern = patterns::binding_pattern(p);
    let mut ty = None;
    if p.eat(COLON) {
        ty = types::type_(p, MutRule::Forbidden);
    }
    if p.eat(EQ) {
        exprs::expr(p, Ctx::default());
    } else {
        no_starting_value(p, pattern, ty);
    }
    if p.at(ELSE_KW) {
        let_else(p);
    }
}

#[cold]
fn no_starting_value(p: &mut Parser<'_>, pattern: Option<Done>, ty: Option<Done>) {
    if p.speculating() {
        return;
    }
    let ends_here =
        matches!(p.current(), NEWLINE | EOF | R_BRACE | ELSE_KW) || p.nth_line_break_before(0);
    if !ends_here {
        // Something else follows: most likely the `=` is missing.
        if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`=`") {
            let at = Span::at(p.last_end());
            p.push_error(diagnostic.suggest("add `=`", at, " ="));
        }
        if exprs::can_start_expr(p.current()) {
            exprs::expr(p, Ctx::default());
        }
        return;
    }
    let name = pattern.map_or("this", |pattern| p.text_of(pattern.span()));
    let name = name.to_string();
    let at = Span::at(p.last_end());
    let diagnostic = Diagnostic::error(format!("give `{name}` a starting value"))
        .with_code(codes::NO_STARTING_VALUE)
        .primary(pattern.map_or(at, Done::span), "declared without a value")
        .note("every `let` and `var` needs a starting value");
    let default = ty.and_then(|ty| default_value(p.text_of(ty.span())));
    let diagnostic = match default {
        Some(value) => {
            diagnostic.suggest(format!("start it at `{value}`"), at, format!(" = {value}"))
        }
        None => diagnostic.help("add `= value` after the name or the type"),
    };
    p.push_error(diagnostic);
}

/// A sensible starting value for a type, for the fix of a `var` with no value.
fn default_value(ty: &str) -> Option<&'static str> {
    let ty = ty.trim();
    if ty.ends_with('?') {
        return Some("none");
    }
    let name = ty.split('<').next().unwrap_or(ty).trim();
    Some(match name {
        "Int" | "UInt" | "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32"
        | "UInt64" => "0",
        "Float" | "Float32" | "Float64" => "0.0",
        "String" => "\"\"",
        "Bool" => "false",
        "List" => "[]",
        "Map" => "[:]",
        _ => return None,
    })
}

/// `let x = maybe else { ... }` (grammar.md, 11): reported, with a fix to `??`.
fn let_else(p: &mut Parser<'_>) {
    let marker = p.start();
    let else_span = p.nth_span(0);
    p.bump();
    let block = if p.at(L_BRACE) { block(p) } else { None };
    p.complete(marker, ERROR);
    if p.speculating() {
        return;
    }
    let mut diagnostic = Diagnostic::error("Quell has no `let ... else`")
        .with_code(codes::LET_ELSE)
        .primary(else_span, "`else` after the value of a `let`");
    match block {
        Some(BlockInfo { done, only_jump: Some(jump) }) => {
            let jump_text = p.text_of(jump).to_string();
            diagnostic = diagnostic.suggest(
                format!("use `??`: `?? {jump_text}`"),
                Span::new(else_span.start, done.end),
                format!("?? {jump_text}"),
            );
        }
        _ => {
            diagnostic = diagnostic.help(
                "use `??` to leave early when the value is missing: `let x = maybe ?? return`",
            );
        }
    }
    p.push_error(diagnostic);
}

/// `ForStmt = "for" BindingPattern "in" HeaderExpr Block`.
fn for_stmt(p: &mut Parser<'_>) {
    p.bump();
    patterns::binding_pattern(p);
    expect_in(p);
    let header = exprs::header_expr(p, false);
    body_block(p, "the `for` header", header.as_ref());
}

/// The `in` of a `for` loop. `of` (from other languages) gets a fix.
pub(crate) fn expect_in(p: &mut Parser<'_>) {
    if p.eat(IN_KW) {
        return;
    }
    if p.at(LOWER_IDENT) && p.nth_text(0) == "of" {
        let span = p.nth_span(0);
        if !p.speculating() {
            p.push_error(
                Diagnostic::error("expected `in`, found `of`")
                    .with_code(codes::EXPECTED_TOKEN)
                    .primary(span, "Quell writes `for x in items`")
                    .suggest("write `in`", span, "in"),
            );
        }
        p.bump_error();
        return;
    }
    p.error_missing_token("in", "`in`");
}
