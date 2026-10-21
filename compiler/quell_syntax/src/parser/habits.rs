//! Habits from other languages (Python above all, but also JavaScript, Rust, Go and Kotlin).
//!
//! None of this code is valid Quell, so recognizing it never changes how a valid program
//! parses. Each habit gets one error that says what Quell writes instead, with that as an exact
//! fix, and the parser then reads the code as if the fix had been applied (grammar.md, 9.1):
//!
//! | Written | Quell writes |
//! |---|---|
//! | `def f(x):`, `function f(x) {`, `func`, `fun` | `fn f(x) {` |
//! | a block after `:`, marked by indentation | the block in `{ }` |
//! | `class Point:` | `struct Point {` |
//! | `impl Point {` | `extend Point {` |
//! | `interface Shape {`, `protocol Shape {` | `trait Shape {` |
//! | `elif`, `elsif`, `elseif` | `else if` |
//! | `const x = 1` | `let x = 1` |
//! | `let mut x = 1` | `var x = 1` |
//! | `fn area(self)` | `fn area()`: `self` is never listed |
//! | `f"Hi {name}"`, `r"\d+"` | `"Hi {name}"`, `#"\d+"#` |
//! | `from math import sqrt, pi` | `import math.{sqrt, pi}` |
//! | `a === b`, `a !== b` | `a == b`, `a != b` |
//! | `new Point(1, 2)` | `Point(1, 2)` |
//! | `a if ok else b` | `if ok { a } else { b }` |
//!
//! A word such as `def` is only taken for a keyword when a name follows it on the same line,
//! which is never valid Quell, so `def`, `class` and the rest stay usable as names.

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::exprs::{self, Expr};
use super::layout;
use super::stmts::{self, BlockInfo};
use super::{Bracket, Parser, codes};
use crate::SyntaxKind::{self, *};

/// Words other languages use where Quell writes `fn`.
const FUNCTION_WORDS: [&str; 4] = ["def", "function", "func", "fun"];

/// Words other languages use where Quell writes `else if`.
const ELSE_IF_WORDS: [&str; 3] = ["elif", "elsif", "elseif"];

/// The Quell keyword that a word from another language at the current token stands for, when
/// a name follows it on the same line: `fn` for `def`, `struct` for `class`, and so on.
pub(crate) fn foreign_keyword(p: &Parser<'_>) -> Option<SyntaxKind> {
    if !p.at(LOWER_IDENT) || !p.nth(1).is_ident() || p.nth_line_break_before(1) {
        return None;
    }
    match p.nth_text(0) {
        word if FUNCTION_WORDS.contains(&word) => Some(FN_KW),
        "class" => Some(STRUCT_KW),
        "impl" => Some(EXTEND_KW),
        "interface" | "protocol" => Some(TRAIT_KW),
        "const" => Some(LET_KW),
        _ => None,
    }
}

/// Reports the word from another language at the current token and reads it as `keyword`.
pub(crate) fn foreign_word(p: &mut Parser<'_>, keyword: SyntaxKind) {
    let span = p.nth_span(0);
    let word = p.nth_text(0).to_string();
    if !p.speculating() {
        let quell = keyword.fixed_text().unwrap_or("fn");
        let (code, message, note) = match keyword {
            FN_KW => (
                codes::FUNCTION_KEYWORD,
                "Quell declares functions with `fn`",
                "a function looks like `fn add(a: Int, b: Int) -> Int { a + b }`",
            ),
            STRUCT_KW => (
                codes::CLASS_KEYWORD,
                "Quell declares types with `struct`",
                "a struct has fields and methods; traits describe what several types can do",
            ),
            EXTEND_KW => (
                codes::IMPL_KEYWORD,
                "Quell adds methods to a type with `extend`",
                "methods can also go right inside the `struct`",
            ),
            TRAIT_KW => (
                codes::INTERFACE_KEYWORD,
                "Quell describes what types can do with `trait`",
                "a trait lists the functions a type must have",
            ),
            _ => (
                codes::CONST_KEYWORD,
                "Quell declares a value that never changes with `let`",
                "`let` declares a value that can't change, `var` one that can",
            ),
        };
        p.push_error(
            Diagnostic::error(message)
                .with_code(code)
                .primary(span, format!("`{word}` is not a keyword in Quell"))
                .note(note)
                .suggest(format!("write `{quell}`"), span, quell),
        );
    }
    p.bump_remap(keyword);
}

/// `let mut x` (from Rust): `var x` declares a variable that can change. Expects to be at the
/// `mut`, right after `let` or `var` (whose span is `keyword`).
pub(crate) fn let_mut(p: &mut Parser<'_>, keyword: Span, is_var: bool) {
    let mut_span = p.nth_span(0);
    if !p.speculating() {
        let removal = super::items::with_space_after(p, mut_span);
        let diagnostic = Diagnostic::error("Quell declares a variable that can change with `var`")
            .with_code(codes::LET_MUT)
            .primary(mut_span, "`mut` can't follow `let` or `var`");
        let diagnostic = if is_var {
            diagnostic
                .note("`var` already makes the variable changeable")
                .suggest("remove `mut`", removal, "")
        } else {
            diagnostic.note("`let` declares a value that can't change").suggest(
                "write `var`",
                Span::new(keyword.start, removal.end),
                "var ",
            )
        };
        p.push_error(diagnostic);
    }
    p.bump_error();
}

/// `fn area(self)` (from Python and Rust): a method has no `self` parameter; inside a method,
/// `self` is always there. `before` is the token before `self` (`(` or `,`). Reads `self` and
/// any type after it into an error node.
pub(crate) fn self_parameter(p: &mut Parser<'_>, before: Span, before_kind: SyntaxKind) {
    let self_span = p.nth_span(0);
    let marker = p.start();
    p.bump();
    if p.eat(COLON) {
        super::types::type_(p, super::types::MutRule::Allowed);
    }
    let done = p.complete(marker, ERROR);
    if p.speculating() {
        return;
    }
    let removal = if p.at(COMMA) {
        Span::new(self_span.start, p.nth_span(1).start)
    } else if before_kind == COMMA {
        Span::new(before.start, done.end)
    } else {
        Span::new(self_span.start, done.end)
    };
    p.push_error(
        Diagnostic::error("a method doesn't list `self` as a parameter")
            .with_code(codes::SELF_PARAMETER)
            .primary(self_span, "`self` is always there inside a method")
            .note(
                "inside a method, `self` is the value the method was called on; a `mut fn` may \
                 change it",
            )
            .suggest("remove `self` from the parameters", removal, ""),
    );
}

/// Whether `elif` (or `elsif`, `elseif`) follows an `if` branch here, at the start of the next
/// line or on the same line, with a condition after it.
pub(crate) fn at_else_if_word(p: &Parser<'_>) -> bool {
    let n = usize::from(p.at(NEWLINE));
    p.nth(n) == LOWER_IDENT
        && ELSE_IF_WORDS.contains(&p.nth_text(n))
        && !p.nth_line_break_before(n + 1)
        && exprs::can_start_expr(p.nth(n + 1))
}

/// Reports `elif` and reads it as `else`, so the `if` branch after it is read as `else if`.
pub(crate) fn else_if_word(p: &mut Parser<'_>) {
    if p.at(NEWLINE) {
        p.bump();
    }
    let span = p.nth_span(0);
    let word = p.nth_text(0).to_string();
    if !p.speculating() {
        p.push_error(
            Diagnostic::error(format!("Quell writes `else if`, not `{word}`"))
                .with_code(codes::ELSE_IF_WORD)
                .primary(span, format!("`{word}` is not a keyword in Quell"))
                .suggest("write `else if`", span, "else if"),
        );
    }
    p.bump_remap(ELSE_KW);
}

/// A block written the Python way, with `:` and indented lines (or the rest of the line after
/// the `:`), where the body of `after` should start. Reported once, with a fix that writes the
/// block in `{ }`, and read as a block up to where the indentation ends it. Returns `None`,
/// consuming nothing, when there is no `:` here or no such block after it.
pub(crate) fn colon_block(p: &mut Parser<'_>, after: &str) -> Option<BlockInfo> {
    if !p.at(COLON) {
        return None;
    }
    let colon = p.pos();
    let (stop, last) = layout::indented_block_end(p.toks(), p.text(), colon)?;
    if !p.enter() {
        return None;
    }
    report_colon_block(p, after, colon, last);
    p.set_list_end(colon, stop);
    let marker = p.start();
    p.bump(); // `:`
    let saved = p.bracket;
    p.bracket = Bracket::Block;
    let only_jump = stmts::statement_list(p, Some(colon));
    p.bracket = saved;
    p.leave();
    Some(BlockInfo { done: p.complete(marker, BLOCK), only_jump })
}

/// Like [`colon_block`], for the body of a `struct`, `enum`, `trait` or `extend` written with
/// `:` and indented lines, as in Python's `class Point:`. Reports it and gives the `:` a list
/// end; the caller reads the members. Returns whether there is such a body here.
pub(crate) fn colon_members(p: &mut Parser<'_>, after: &str) -> bool {
    if !p.at(COLON) {
        return false;
    }
    let colon = p.pos();
    let Some((stop, last)) = layout::indented_block_end(p.toks(), p.text(), colon) else {
        return false;
    };
    report_colon_block(p, after, colon, last);
    p.set_list_end(colon, stop);
    true
}

fn report_colon_block(p: &mut Parser<'_>, after: &str, colon: usize, last: usize) {
    if p.speculating() {
        return;
    }
    let edits = layout::colon_block_fix(p.text(), p.toks(), colon, last);
    p.push_error(
        Diagnostic::error("Quell puts a block in `{ }`, not after `:`")
            .with_code(codes::COLON_BLOCK)
            .primary(p.nth_span(0), format!("the block after {after} starts here"))
            .note(
                "indentation has no meaning in Quell: `{` and `}` show where a block starts and \
                 ends",
            )
            .suggest_edits("write the block in `{ }`", edits),
    );
}

/// The fix for a `:` where a body should start with no indented block after it.
pub(crate) fn colon_instead_of_brace(p: &mut Parser<'_>) {
    if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`{`") {
        p.push_error(
            diagnostic
                .note("Quell puts a block in `{ }`; indentation has no meaning")
                .help("write the block in `{ }`, as in `if ready { start() }`"),
        );
    }
}

/// Whether a string prefix from another language starts here: `f"..."` or `r"..."`, with no
/// space before the quote. A name directly followed by a string is never valid Quell.
pub(crate) fn at_string_prefix(p: &Parser<'_>) -> bool {
    p.at(LOWER_IDENT)
        && matches!(p.nth_text(0), "f" | "r")
        && p.nth(1) == STRING_START
        && p.nth_is_joined(1)
}

/// `f"Hi {name}"` and `r"\d+"`: reported, and the string is read without the prefix.
pub(crate) fn string_prefix(p: &mut Parser<'_>) -> Expr {
    let prefix = p.nth_span(0);
    let raw = p.nth_text(0) == "r";
    let string_end = p.nth_partner(1).map(|end| p.span_at(end).end);
    p.bump_error();
    let string = exprs::string_literal(p);
    if !p.speculating() {
        let diagnostic = if raw {
            let diagnostic = Diagnostic::error("Quell writes a raw string as `#\"...\"#`")
                .with_code(codes::STRING_PREFIX)
                .primary(prefix, "`r` before a string")
                .note("in a raw string, `\\` and `{` are plain characters");
            match string_end {
                Some(end) => diagnostic.suggest_edits(
                    "write `#\"...\"#`",
                    vec![
                        Edit { span: prefix, replacement: "#".to_string() },
                        Edit { span: Span::at(end), replacement: "#".to_string() },
                    ],
                ),
                None => diagnostic.help("write `#\"...\"#`"),
            }
        } else {
            Diagnostic::error("Quell strings don't need `f` to hold values")
                .with_code(codes::STRING_PREFIX)
                .primary(prefix, "`f` before a string")
                .note("every string can hold values in `{ }`, as in `\"Hi {name}\"`")
                .suggest("remove the `f`", prefix, "")
        };
        p.push_error(diagnostic);
    }
    Expr::new(string.done)
}

/// Whether `from module import names` (from Python) starts here.
pub(crate) fn at_from_import(p: &Parser<'_>) -> bool {
    if !(p.at(LOWER_IDENT) && p.nth_text(0) == "from" && p.nth(1).is_ident()) {
        return false;
    }
    // `from a.b.c import`, all on one line.
    let mut n = 2;
    while p.nth(n) == DOT && p.nth(n + 1).is_ident() && n < 64 {
        n += 2;
    }
    p.nth(n) == IMPORT_KW && !(1..=n).any(|k| p.nth_line_break_before(k))
}

/// `from math import sqrt, pi`: reported, with the fix `import math.{sqrt, pi}`, and read as
/// that import.
pub(crate) fn from_import(p: &mut Parser<'_>) {
    let from = p.nth_span(0);
    p.bump_error();
    let path = p.start();
    let path_start = p.nth_span(0).start;
    p.bump();
    while p.at(DOT) && p.nth(1).is_ident() {
        p.bump();
        p.bump();
    }
    p.complete(path, MODULE_PATH);
    let path_end = p.last_end();
    p.bump_remap(IMPORT_KW);
    let list = p.start();
    let mut names = Vec::new();
    while p.current().is_ident() && !p.nth_line_break_before(0) {
        names.push(p.nth_text(0).to_string());
        p.bump();
        if !(p.at(COMMA) && p.nth(1).is_ident()) {
            break;
        }
        p.bump();
    }
    let list_end = p.last_end();
    p.complete(list, IMPORT_LIST);
    if p.speculating() {
        return;
    }
    let module = p.text_of(Span::new(path_start, path_end)).to_string();
    let diagnostic = Diagnostic::error("Quell imports names with `import module.{names}`")
        .with_code(codes::FROM_IMPORT)
        .primary(from, "`from` is not a keyword in Quell");
    let diagnostic = if names.is_empty() {
        diagnostic.help(format!("write `import {module}.{{name}}`"))
    } else {
        let fixed = format!("import {module}.{{{}}}", names.join(", "));
        diagnostic.suggest(format!("write `{fixed}`"), Span::new(from.start, list_end), fixed)
    };
    p.push_error(diagnostic);
}

/// `===` and `!==` (from JavaScript), which lex as `==` or `!=` followed by `=`. Expects to be
/// right after the `==` or `!=`, at the `=`.
pub(crate) fn at_triple_equals(p: &Parser<'_>) -> bool {
    p.at(EQ) && p.split_is_zero() && p.last_kind_is_equality_joined()
}

/// Reports the extra `=` of `===` or `!==` and skips it.
pub(crate) fn triple_equals(p: &mut Parser<'_>) {
    let extra = p.nth_span(0);
    let operator = p.last_token_span().to(extra);
    let written = p.text_of(operator).to_string();
    let quell = &written[..2];
    if !p.speculating() {
        p.push_error(
            Diagnostic::error(format!("Quell writes `{quell}`, not `{written}`"))
                .with_code(codes::TRIPLE_EQUALS)
                .primary(operator, format!("`{written}` is not an operator in Quell"))
                .note("`==` and `!=` compare values, and never convert one type to another")
                .suggest(format!("write `{quell}`"), extra, ""),
        );
    }
    p.bump_error();
}

/// Whether `new Point(...)` (from Java, JavaScript and C#) starts here.
pub(crate) fn at_new(p: &Parser<'_>) -> bool {
    p.at(LOWER_IDENT)
        && p.nth_text(0) == "new"
        && p.nth(1) == UPPER_IDENT
        && !p.nth_line_break_before(1)
}

/// Reports `new` and skips it, so the value after it is read as a call.
pub(crate) fn new_keyword(p: &mut Parser<'_>) {
    let span = p.nth_span(0);
    if !p.speculating() {
        let removal = super::items::with_space_after(p, span);
        p.push_error(
            Diagnostic::error("Quell creates values without `new`")
                .with_code(codes::NEW_KEYWORD)
                .primary(span, "`new` is not a keyword in Quell")
                .note("a value is created by calling its type, as in `Point(x: 1.0, y: 2.0)`")
                .suggest("remove `new`", removal, ""),
        );
    }
    p.bump_error();
}

/// Whether `value if condition else other` (from Python) continues the expression here: an
/// `if` on the same line right after a complete value, with an `else` later on the line.
pub(crate) fn at_python_conditional(p: &Parser<'_>) -> bool {
    if !p.at(IF_KW) || p.nth_line_break_before(0) {
        return false;
    }
    let mut pos = p.pos() + 1;
    for _ in 0..1000 {
        match p.kind_at(pos) {
            ELSE_KW => return !p.line_break_before_at(pos),
            NEWLINE | EOF | COMMA | L_BRACE | R_BRACE | R_PAREN | R_BRACK | HOLE_END
            | FORMAT_SPEC | STRING_END => return false,
            L_PAREN | L_BRACK | STRING_START => match p.partner_of(pos) {
                Some(close) => pos = close + 1,
                None => return false,
            },
            _ => pos += 1,
        }
    }
    false
}

/// `value if condition else other`: reported, with a fix that writes it as an `if`.
pub(crate) fn python_conditional(p: &mut Parser<'_>, value: Expr) -> Expr {
    let marker = value.done.precede(p);
    let if_span = p.nth_span(0);
    p.bump(); // `if`
    let condition = exprs::header_expr(p, true);
    let else_span = p.nth_span(0);
    p.eat(ELSE_KW);
    let other = exprs::full_expr(p, super::Ctx::default());
    let done = p.complete(marker, ERROR);
    if !p.speculating() {
        let diagnostic = Diagnostic::error("Quell writes this as `if condition { a } else { b }`")
            .with_code(codes::PYTHON_CONDITIONAL)
            .primary(if_span.to(else_span), "`if` after a value")
            .note("`if` gives a value, so it does the job of `a if condition else b`");
        let diagnostic = match (condition, other) {
            (Some(condition), Some(other)) => {
                let text = |span: Span| p.text_of(span).to_string();
                let fixed = format!(
                    "if {} {{ {} }} else {{ {} }}",
                    text(condition.done.span()),
                    text(value.done.span()),
                    text(other.done.span())
                );
                diagnostic.suggest(format!("write `{fixed}`"), done.span(), fixed)
            }
            _ => diagnostic.help("write `if condition { a } else { b }`"),
        };
        p.push_error(diagnostic);
    }
    Expr::new(done)
}
