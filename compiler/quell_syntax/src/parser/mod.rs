//! The Quell parser: tokens to a lossless syntax tree (spec/grammar.md and
//! docs/decisions/0005-front-end-architecture.md).
//!
//! # How it works
//!
//! The parser is a hand-written recursive descent parser, with Pratt parsing (precedence
//! climbing) for expressions. It reads the tokens of a [`Lexed`] file, skipping trivia, and
//! emits a flat list of [`Event`]s: start a node, add a token, finish a node, report an error.
//! The tree builder (`builder.rs`) then turns the events into a rowan `GreenNode`, putting the
//! trivia back between the tokens, so the tree's text is the input, byte for byte.
//!
//! A [`Marker`] starts a node. It is later completed with a kind, or abandoned. A completed
//! node can be wrapped in a new parent with [`Done::precede`], which is how left-recursive
//! constructs (`a + b`, `f(x)`, `a.b`) are built without knowing in advance that they come.
//!
//! The grammar is split by topic: `items.rs` (files and items), `stmts.rs` (blocks and
//! statements), `exprs.rs` (expressions), `types.rs` (types and the speculative reading of
//! generic arguments) and `patterns.rs`. `fixes.rs` holds the helpers that write fixes.
//!
//! # Errors and recovery
//!
//! The parser never stops at the first error. It reports the error with a fix and carries on
//! as if the fix had been applied where the spec says so (grammar.md, 9.1). Code it can't read
//! is wrapped in an `ERROR` node, up to a safe point: the next significant line break at the
//! current level, a closing bracket that belongs to an enclosing construct, or the end of a
//! string hole. Because the lexer records the partner of every bracket, a construct in brackets
//! always knows where it ends, so a mistake inside never spills out of it. A bracket that is
//! never closed was already reported by the lexer, at the opening bracket, with a fix; the
//! parser does not report it again.
//!
//! # Limits
//!
//! Nesting deeper than [`NESTING_LIMIT`] is reported (Q0106) and the rest of the construct is
//! skipped without recursion, so the parser can't overflow the stack. Every loop either
//! consumes a token or stops, and a fuel counter makes the parser treat the rest of the file as
//! ended if a bug ever made it stand still. Speculation (generic arguments, section 6.11) is
//! remembered per `<` token, so the whole parse takes linear time.

mod builder;
pub(crate) mod codes;
mod exprs;
mod fixes;
mod items;
mod patterns;
mod stmts;
mod types;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::HashMap;

use quell_diagnostics::{Diagnostic, Span};
use rowan::GreenNode;

use crate::SyntaxKind::{self, *};
use crate::{FileRole, Lexed};

pub(crate) use builder::build_tree;

/// How deeply constructs may nest. Each bracket, block, string, `if`, `match`, prefix
/// operator, type and pattern counts as a level, so does each expression inside one, and so
/// does each operator in a chain (`a + b + c` nests the tree two levels deep). So `((x))` is
/// four levels deep, and code can nest brackets about 128 deep. The limit keeps a debug build
/// parsing code at this depth well inside a 2 MiB stack (it needs about 0.6 MiB); a test checks
/// that on a thread with exactly that stack.
pub const NESTING_LIMIT: u32 = 256;

/// Lookahead queries allowed at one position before the parser counts as stuck.
const FUEL: u32 = 4096;

/// No partner, in [`Tok::partner`].
const NONE: u32 = u32::MAX;

/// Parses a lexed file into a green tree and the parser's diagnostics. The lexer's
/// diagnostics are not included.
pub(crate) fn parse_lexed(lexed: &Lexed<'_>, role: FileRole) -> (GreenNode, Vec<Diagnostic>) {
    let mut parser = Parser::new(lexed, role);
    items::file(&mut parser);
    let unclosed = std::mem::take(&mut parser.unclosed);
    let (green, diagnostics) = build_tree(lexed, parser.events);
    let mut lexer_diagnostics = lexed.diagnostics().to_vec();
    better_closing_fixes(lexed.text(), &mut lexer_diagnostics, &unclosed);
    lexer_diagnostics.extend(diagnostics);
    (green, lexer_diagnostics)
}

/// The lexer reports a bracket that is never closed with a fix that adds the closer where the
/// lexer noticed it was missing: before a closer that doesn't match, or at the end of the file.
/// The parser knows better where the bracketed construct ended (usually the end of its line),
/// so it moves the fix there.
fn better_closing_fixes(text: &str, diagnostics: &mut [Diagnostic], unclosed: &[(u32, u32)]) {
    for diagnostic in diagnostics.iter_mut() {
        if diagnostic.code != Some(crate::lexer::codes::UNCLOSED_BRACKET) {
            continue;
        }
        let Some(open) = diagnostic.main_label().map(|label| label.span.start) else { continue };
        let Some(&(_, end)) = unclosed.iter().find(|(start, _)| *start == open) else { continue };
        let closer = match text.as_bytes().get(open as usize) {
            Some(b'(') => ")",
            Some(b'[') => "]",
            _ => "}",
        };
        for help in &mut diagnostic.helps {
            if let Some(edit) = help.edits.first_mut() {
                edit.span = Span::at(end);
                edit.replacement = closer.to_string();
            }
        }
    }
}

// ----------------------------------------------------------------------
// Events and markers
// ----------------------------------------------------------------------

/// One step of building the tree.
#[derive(Debug)]
pub(crate) enum Event {
    /// Start a node. `kind` is `None` for a marker that was abandoned. `forward_parent` is the
    /// distance to the `Start` event of a node that must wrap this one (0 for none): this is how
    /// [`Done::precede`] wraps a node that is already complete.
    Start { kind: Option<SyntaxKind>, forward_parent: u32 },
    /// Add the next token, up to byte offset `end` (less than the whole token when a `>>` is
    /// split to close generic arguments). Trivia before it is added by the builder.
    Token { kind: SyntaxKind, end: u32 },
    /// Finish the innermost node.
    Finish,
    /// Report an error or a warning.
    Error(Box<Diagnostic>),
}

/// The start of a node that is not yet complete. It must be completed or abandoned.
#[must_use]
#[derive(Debug)]
pub(crate) struct Marker {
    event: u32,
    start: u32,
}

/// A complete node: where its `Start` event is, its kind and its text range.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Done {
    event: u32,
    pub(crate) kind: SyntaxKind,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

impl Done {
    /// Starts a new node that will wrap this one.
    pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
        let marker = Marker { event: p.events.len() as u32, start: self.start };
        p.events.push(Event::Start { kind: None, forward_parent: 0 });
        if let Some(Event::Start { forward_parent, .. }) = p.events.get_mut(self.event as usize) {
            *forward_parent = marker.event - self.event;
        }
        marker
    }

    pub(crate) fn span(self) -> Span {
        Span::new(self.start, self.end.max(self.start))
    }
}

// ----------------------------------------------------------------------
// The token stream the grammar reads
// ----------------------------------------------------------------------

/// A token the grammar reads (no trivia).
#[derive(Clone, Copy, Debug)]
struct Tok {
    kind: SyntaxKind,
    start: u32,
    end: u32,
    /// The position of the matching bracket in the parser's token list, or [`NONE`].
    partner: u32,
    /// A line break (of any kind) came before this token.
    line_break_before: bool,
    /// A line break came before this token, but the innermost open bracket is a `(` or `[`
    /// that is never closed. In valid code this break would very likely have ended a
    /// statement, so the parser treats it as a place where a statement may end.
    soft_break: bool,
}

/// The innermost bracket the parser is in, for the forgotten-comma check (grammar.md, 6.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bracket {
    /// Not in brackets, or in a block, where line breaks end statements.
    Block,
    /// A list or map literal: a forgotten comma is an error.
    List,
    /// Parentheses, call arguments, indexing and the like: a forgotten comma is a warning.
    Other,
}

/// Where an expression is, for the rules that depend on it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Ctx {
    /// The expression right before a body block (a `HeaderExpr`, grammar.md, 6.1).
    pub(crate) header: bool,
    /// The condition of `if` or `while`, or a match guard: `=` there is reported (4.11).
    pub(crate) cond: bool,
    /// A match guard: a name followed by `=>` is never a lambda here (10.5).
    pub(crate) guard: bool,
}

/// What a saved parser state needs to go back to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Checkpoint {
    pos: usize,
    split: u32,
    events: usize,
    last_end: u32,
    last_kind: SyntaxKind,
    last_error_at: u32,
    depth: u32,
}

pub(crate) struct Parser<'t> {
    text: &'t str,
    toks: Vec<Tok>,
    /// The current token, as an index into `toks`.
    pos: usize,
    /// How many bytes of the current token were already used (a split `>>`, section 7.3).
    split: u32,
    pub(crate) events: Vec<Event>,
    fuel: Cell<u32>,
    pub(crate) role: FileRole,
    /// How deeply nested the current construct is (see [`NESTING_LIMIT`]).
    depth: u32,
    /// For each `<` token tried as the start of generic arguments in an expression, the result:
    /// where the arguments end (position and split), or `None` if they don't parse.
    pub(crate) generic_memo: HashMap<usize, Option<(usize, u32)>>,
    /// The innermost bracket (grammar.md, 6.12).
    pub(crate) bracket: Bracket,
    /// The start and end of the last token added, and its kind.
    last_start: u32,
    last_end: u32,
    last_kind: SyntaxKind,
    /// The start of the token the last error pointed at, so the same place isn't reported
    /// twice by constructs that fail one inside the other.
    last_error_at: u32,
    /// More than 0 while a speculative parse runs: it reports nothing and is undone.
    speculating: u32,
    /// A position where a forgotten comma in a list was already reported, so the list reads
    /// the next element as if the comma were there.
    pub(crate) comma_forgiven: usize,
    /// The start of the innermost full expression, for the fix of the warning about a line
    /// that starts with `.` (grammar.md, 10.17).
    pub(crate) expr_start: u32,
    /// For each bracket that is never closed: where it starts and where its construct ended.
    unclosed: Vec<(u32, u32)>,
}

impl std::fmt::Debug for Parser<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser").field("pos", &self.pos).field("depth", &self.depth).finish()
    }
}

impl<'t> Parser<'t> {
    fn new(lexed: &Lexed<'t>, role: FileRole) -> Parser<'t> {
        let text = lexed.text();
        let tokens = lexed.tokens();
        // Map the lexer's indexes to positions in `toks`, to translate bracket partners.
        let mut index_of = vec![NONE; tokens.len()];
        let mut toks = Vec::with_capacity(tokens.len() / 2 + 1);
        for (index, token) in tokens.iter().enumerate() {
            if token.kind.is_trivia() {
                continue;
            }
            index_of[index] = toks.len() as u32;
            let span = lexed.span(index);
            toks.push(Tok {
                kind: token.kind,
                start: span.start,
                end: span.end,
                partner: NONE,
                line_break_before: token.line_break_before,
                soft_break: false,
            });
        }
        let mut position = 0;
        for (index, token) in tokens.iter().enumerate() {
            if token.kind.is_trivia() {
                continue;
            }
            if let Some(partner) = lexed.partner(index) {
                toks[position].partner = index_of[partner];
            }
            position += 1;
        }
        mark_soft_breaks(&mut toks);
        Parser {
            text,
            toks,
            pos: 0,
            split: 0,
            events: Vec::new(),
            fuel: Cell::new(FUEL),
            role,
            depth: 0,
            generic_memo: HashMap::new(),
            bracket: Bracket::Block,
            last_start: 0,
            last_end: 0,
            last_kind: EOF,
            last_error_at: NONE,
            speculating: 0,
            comma_forgiven: usize::MAX,
            expr_start: 0,
            unclosed: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Looking at tokens
    // ------------------------------------------------------------------

    /// The kind of the token `n` places ahead, as the grammar reads it: `&&` reads as `and`
    /// and so on ([`SyntaxKind::recovered`]), and a split `>>` reads as what is left of it.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let fuel = self.fuel.get();
        if fuel == 0 {
            // A bug made the parser stand still. Treat the rest as the end of the file, so the
            // parse ends; the tree builder still keeps every token.
            debug_assert!(false, "the parser is stuck at token {}", self.pos);
            return EOF;
        }
        self.fuel.set(fuel - 1);
        if n == 0 {
            return self.current_raw().recovered();
        }
        self.toks.get(self.pos + n).map_or(EOF, |tok| tok.kind.recovered())
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth(0) == kind
    }

    /// The current token's own kind (not recovered), taking a split `>>` into account.
    fn current_raw(&self) -> SyntaxKind {
        let Some(tok) = self.toks.get(self.pos) else { return EOF };
        if self.split == 0 {
            return tok.kind;
        }
        match (tok.kind, self.split) {
            (GT_GT, 1) => GT,
            (GT_EQ, 1) | (GT_GT_EQ, 2) => EQ,
            (GT_GT_EQ, 1) => GT_EQ,
            (kind, _) => kind,
        }
    }

    fn tok(&self, pos: usize) -> Tok {
        self.toks.get(pos).copied().unwrap_or_else(|| self.toks[self.toks.len() - 1])
    }

    /// The position of the current token in the parser's token list.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// The kind of the token at position `pos` (recovered), without using fuel.
    pub(crate) fn kind_at(&self, pos: usize) -> SyntaxKind {
        self.tok(pos).kind.recovered()
    }

    /// Where the token `n` places ahead is in the text. The current token's span starts after
    /// any part of it that was already used.
    pub(crate) fn nth_span(&self, n: usize) -> Span {
        let tok = self.tok(self.pos + n);
        let start = if n == 0 { tok.start + self.split } else { tok.start };
        Span::new(start.min(tok.end), tok.end)
    }

    pub(crate) fn span_at(&self, pos: usize) -> Span {
        if pos == self.pos {
            return self.nth_span(0);
        }
        let tok = self.tok(pos);
        Span::new(tok.start, tok.end)
    }

    /// The text of the token `n` places ahead.
    pub(crate) fn nth_text(&self, n: usize) -> &'t str {
        let span = self.nth_span(n);
        self.text.get(span.start as usize..span.end as usize).unwrap_or("")
    }

    pub(crate) fn text_of(&self, span: Span) -> &'t str {
        self.text.get(span.start as usize..span.end as usize).unwrap_or("")
    }

    pub(crate) fn text(&self) -> &'t str {
        self.text
    }

    /// Whether a line break (significant or not) comes before the token `n` places ahead.
    pub(crate) fn nth_line_break_before(&self, n: usize) -> bool {
        if n == 0 && self.split > 0 {
            return false;
        }
        self.tok(self.pos + n).line_break_before
    }

    /// Whether the token `n` places ahead directly follows the one before it, with nothing at
    /// all between them (for `::`, `++` and `--`).
    pub(crate) fn nth_is_joined(&self, n: usize) -> bool {
        n > 0 && self.tok(self.pos + n - 1).end == self.tok(self.pos + n).start
    }

    /// The position of the bracket that matches the token `n` places ahead, if it has one.
    pub(crate) fn nth_partner(&self, n: usize) -> Option<usize> {
        let partner = self.tok(self.pos + n).partner;
        (partner != NONE).then_some(partner as usize)
    }

    pub(crate) fn partner_of(&self, pos: usize) -> Option<usize> {
        let partner = self.tok(pos).partner;
        (partner != NONE).then_some(partner as usize)
    }

    /// Whether the current token comes after a line break inside a bracket that is never
    /// closed (see [`Tok::soft_break`]).
    pub(crate) fn at_soft_break(&self) -> bool {
        self.split == 0 && self.tok(self.pos).soft_break
    }

    /// The end of the last token added to the tree.
    pub(crate) fn last_end(&self) -> u32 {
        self.last_end
    }

    /// Where the last token added to the tree is.
    pub(crate) fn last_token_span(&self) -> Span {
        Span::new(self.last_start.min(self.last_end), self.last_end)
    }

    /// Whether no part of the current token was used yet (it is not what is left of a split
    /// `>>`).
    pub(crate) fn split_is_zero(&self) -> bool {
        self.split == 0
    }

    /// Whether a line break comes before the token at position `pos`.
    pub(crate) fn line_break_before_at(&self, pos: usize) -> bool {
        self.tok(pos).line_break_before
    }

    /// The whole span of the token at position `pos`, ignoring any split.
    pub(crate) fn span_at_raw(&self, pos: usize) -> Span {
        let tok = self.tok(pos);
        Span::new(tok.start, tok.end)
    }

    /// Whether the `{` that opens the body of a keyword construct is here (lexical.md, 12.6).
    /// It must be on the same line as what comes before it; if it is on the next line, that is
    /// reported and the `{` is still taken. `after` names what comes before, for the message.
    pub(crate) fn at_body_brace(&mut self, after: &str) -> bool {
        if self.at(L_BRACE) {
            if self.nth_line_break_before(0) {
                self.brace_on_next_line(after, 0);
            }
            return true;
        }
        if self.at(NEWLINE) && self.nth(1) == L_BRACE {
            self.brace_on_next_line(after, 1);
            self.bump();
            return true;
        }
        false
    }

    #[cold]
    fn brace_on_next_line(&mut self, after: &str, n: usize) {
        if self.speculating() {
            return;
        }
        let brace = self.nth_span(n);
        let between = Span::new(self.last_end, brace.start);
        let only_space = self.text_of(between).chars().all(char::is_whitespace);
        let diagnostic = Diagnostic::error(format!(
            "the `{{` that opens this block must be on the same line as {after}"
        ))
        .with_code(codes::BRACE_ON_NEXT_LINE)
        .primary(brace, "this `{` is on a new line")
        .note("a line break before `{` ends the line, so the block would stand on its own");
        let diagnostic = if only_space {
            diagnostic.suggest("move the `{` up", between, " ")
        } else {
            let rest = &self.text[brace.end as usize..];
            let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            diagnostic.suggest_edits(
                "move the `{` to the end of the line above",
                vec![
                    quell_diagnostics::Edit {
                        span: Span::at(self.last_end),
                        replacement: " {".into(),
                    },
                    quell_diagnostics::Edit {
                        span: Span::new(brace.start, brace.end + spaces as u32),
                        replacement: String::new(),
                    },
                ],
            )
        };
        self.push_error(diagnostic);
    }

    pub(crate) fn last_kind(&self) -> SyntaxKind {
        self.last_kind
    }

    // ------------------------------------------------------------------
    // Consuming tokens
    // ------------------------------------------------------------------

    /// Adds the current token to the tree and moves to the next one. Does nothing at the end
    /// of the file.
    pub(crate) fn bump(&mut self) {
        if self.pos + 1 >= self.toks.len() {
            return;
        }
        let kind = self.current_raw();
        let end = self.toks[self.pos].end;
        self.events.push(Event::Token { kind, end });
        self.last_start = self.toks[self.pos].start + self.split;
        self.last_end = end;
        self.last_kind = kind;
        self.pos += 1;
        self.split = 0;
        self.fuel.set(FUEL);
    }

    /// Adds the end-of-file token. Only the file rule does this, last.
    pub(crate) fn bump_eof(&mut self) {
        // Anything left (only after a bug made the parser stop early) goes into an error node,
        // so the tree still holds every token.
        if self.pos + 1 < self.toks.len() {
            let marker = self.start();
            while self.pos + 1 < self.toks.len() {
                self.bump();
            }
            self.complete(marker, ERROR);
        }
        let end = self.toks[self.toks.len() - 1].end;
        self.events.push(Event::Token { kind: EOF, end });
    }

    /// Adds the current token if it has this kind.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Whether the current token can close generic arguments: `>`, `>>`, `>=` or `>>=`.
    pub(crate) fn at_closing_angle(&self) -> bool {
        matches!(self.current(), GT | GT_GT | GT_EQ | GT_GT_EQ)
    }

    /// Uses one `>` of the current token to close generic arguments, splitting `>>`, `>=`
    /// and `>>=` (grammar.md, 7.3).
    pub(crate) fn bump_closing_angle(&mut self) {
        if self.current_raw() == GT {
            self.bump();
            return;
        }
        let tok = self.toks[self.pos];
        let end = tok.start + self.split + 1;
        self.events.push(Event::Token { kind: GT, end });
        self.last_start = end - 1;
        self.last_end = end;
        self.last_kind = GT;
        self.split += 1;
        self.fuel.set(FUEL);
    }

    // ------------------------------------------------------------------
    // Nodes
    // ------------------------------------------------------------------

    pub(crate) fn start(&mut self) -> Marker {
        let start = self.nth_span(0).start;
        let marker = Marker { event: self.events.len() as u32, start };
        self.events.push(Event::Start { kind: None, forward_parent: 0 });
        marker
    }

    pub(crate) fn complete(&mut self, marker: Marker, kind: SyntaxKind) -> Done {
        if let Some(Event::Start { kind: slot, .. }) = self.events.get_mut(marker.event as usize) {
            *slot = Some(kind);
        }
        self.events.push(Event::Finish);
        let end = self.last_end.max(marker.start);
        // A node that holds no token starts where it ends.
        let start = if self.last_end < marker.start { end } else { marker.start };
        Done { event: marker.event, kind, start, end }
    }

    pub(crate) fn abandon(&mut self, marker: Marker) {
        if marker.event as usize + 1 == self.events.len() {
            self.events.pop();
        }
    }

    /// Whether anything was added since `marker` started.
    pub(crate) fn consumed_since(&self, marker: &Marker) -> bool {
        self.events[marker.event as usize + 1..].iter().any(|e| matches!(e, Event::Token { .. }))
    }

    /// Wraps the current token in an error node.
    pub(crate) fn bump_error(&mut self) {
        let marker = self.start();
        self.bump();
        self.complete(marker, ERROR);
    }

    // ------------------------------------------------------------------
    // Nesting
    // ------------------------------------------------------------------

    /// Enters one more level of nesting. If that is too deep, reports it, skips the rest of
    /// the construct without recursing, and returns `false`; the caller must then return.
    pub(crate) fn enter(&mut self) -> bool {
        if self.depth >= NESTING_LIMIT {
            self.too_deep();
            return false;
        }
        self.depth += 1;
        true
    }

    /// Reports that the code here is nested too deeply and skips the rest of the construct,
    /// as [`Parser::enter`] does at the limit.
    pub(crate) fn enter_beyond_limit(&mut self) {
        self.too_deep();
    }

    pub(crate) fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub(crate) fn depth(&self) -> u32 {
        self.depth
    }

    pub(crate) fn set_depth(&mut self, depth: u32) {
        self.depth = depth;
    }

    #[cold]
    #[inline(never)]
    fn too_deep(&mut self) {
        let span = self.nth_span(0);
        self.push_error(
            Diagnostic::error("this code is nested too deeply")
                .with_code(codes::NESTED_TOO_DEEPLY)
                .primary(span, "the compiler stops reading this expression here")
                .note(format!(
                    "the compiler reads code nested up to {NESTING_LIMIT} levels deep, where each \
                     bracket, block and operator counts, and so does the expression inside it"
                ))
                .help("move part of this code into a separate variable or function"),
        );
        // Skip the rest of the construct, without recursion.
        let marker = self.start();
        loop {
            if self.at_safe_point() {
                break;
            }
            self.skip_one_balanced();
        }
        if self.consumed_since(&marker) {
            self.complete(marker, ERROR);
        } else {
            self.abandon(marker);
        }
    }

    // ------------------------------------------------------------------
    // Errors and recovery
    // ------------------------------------------------------------------

    pub(crate) fn push_error(&mut self, diagnostic: Diagnostic) {
        if self.speculating > 0 {
            return;
        }
        if let Some(label) = diagnostic.main_label() {
            self.last_error_at = label.span.start;
        }
        self.events.push(Event::Error(Box::new(diagnostic)));
    }

    /// Whether an error was already reported at the current token, so a plain "expected"
    /// error here would only repeat it.
    pub(crate) fn already_reported_here(&self) -> bool {
        self.last_error_at == self.nth_span(0).start || self.last_error_at == self.missing_at()
    }

    /// Where to point at something missing before the current token: right after the last
    /// token when the current one is on another line or ends the file, else the current token.
    pub(crate) fn missing_span(&self) -> Span {
        if matches!(self.current(), NEWLINE | EOF) || self.nth_line_break_before(0) {
            Span::at(self.last_end)
        } else {
            self.nth_span(0)
        }
    }

    fn missing_at(&self) -> u32 {
        self.missing_span().start
    }

    /// How the current token is named in "expected ..., found ..." messages.
    pub(crate) fn describe_current(&self) -> String {
        let text = self.nth_text(0);
        match self.current() {
            NEWLINE => "the end of the line".to_string(),
            EOF => "the end of the file".to_string(),
            UPPER_IDENT | LOWER_IDENT => format!("the name `{text}`"),
            INT | FLOAT => format!("the number `{text}`"),
            CHAR => format!("the character `{text}`"),
            STRING_START => "a string".to_string(),
            HOLE_END | FORMAT_SPEC => "the end of the `{ }` value".to_string(),
            STRING_END => "the end of the string".to_string(),
            kind if kind.is_keyword() => format!("the keyword `{text}`"),
            _ => format!("`{text}`"),
        }
    }

    /// An "expected X, found Y" error at the current token (or right after the last token,
    /// when the current one is on the next line), with a note when the line started with
    /// `.Name` (grammar.md, 11). Returns `None` when this place was already reported.
    pub(crate) fn expected(&mut self, code: &'static str, what: &str) -> Option<Diagnostic> {
        if self.already_reported_here() || self.speculating > 0 {
            return None;
        }
        let found = self.describe_current();
        let span = self.missing_span();
        let label = if span.is_empty() {
            format!("expected {what} here")
        } else {
            format!("expected {what}")
        };
        let diagnostic = Diagnostic::error(format!("expected {what}, found {found}"))
            .with_code(code)
            .primary(span, label);
        Some(self.with_dot_option_note(diagnostic))
    }

    /// Reports an "expected X" error with a plain help.
    pub(crate) fn error_expected(&mut self, code: &'static str, what: &str, help: &str) {
        if let Some(diagnostic) = self.expected(code, what) {
            self.push_error(diagnostic.help(help));
        }
    }

    /// Reports that a token is missing, with a fix that inserts it after the last token.
    pub(crate) fn error_missing_token(&mut self, token: &str, what: &str) {
        if let Some(diagnostic) = self.expected(codes::EXPECTED_TOKEN, what) {
            let at = Span::at(self.last_end);
            let separator =
                if matches!(token, ")" | "]" | "}" | "," | ":" | ">") { "" } else { " " };
            let diagnostic =
                diagnostic.suggest(format!("add `{token}`"), at, format!("{separator}{token}"));
            self.push_error(diagnostic);
        }
    }

    /// Adds the note of grammar.md, section 11 when the current line starts with `.Name`.
    fn with_dot_option_note(&self, diagnostic: Diagnostic) -> Diagnostic {
        // Find the first token of the current line (looking back a bounded distance).
        let mut first = self.pos.min(self.toks.len() - 1);
        let mut steps = 0;
        while first > 0 && !self.toks[first].line_break_before && steps < 1000 {
            first -= 1;
            steps += 1;
        }
        let dot = self.toks[first];
        let name = self.tok(first + 1);
        let starts_with_dot_option = self.toks[first].line_break_before
            && dot.kind == DOT
            && name.kind == UPPER_IDENT
            && dot.end == name.start;
        if !starts_with_dot_option {
            return diagnostic;
        }
        let name = self.text_of(Span::new(name.start, name.end));
        diagnostic
            .note(format!(
                "a line that starts with `.` continues the line above, so `.{name}` was read as a \
                 member of the value before it"
            ))
            .suggest(
                format!("enum options are written by name: write `{name}` (or `Type.{name}`)"),
                Span::new(dot.start, dot.end),
                "",
            )
    }

    /// Whether the current token is a place to stop skipping: a significant line break, the
    /// end of the file, a closing bracket that belongs to an enclosing construct, the end of a
    /// string hole, or a line break inside a bracket that is never closed.
    pub(crate) fn at_safe_point(&self) -> bool {
        let tok = self.tok(self.pos);
        match tok.kind {
            NEWLINE | EOF | HOLE_END | FORMAT_SPEC | STRING_END => true,
            R_PAREN | R_BRACK | R_BRACE => tok.partner != NONE,
            _ => self.at_soft_break(),
        }
    }

    /// Adds the current token, or the whole bracketed run that starts with it.
    pub(crate) fn skip_one_balanced(&mut self) {
        let tok = self.tok(self.pos);
        let opens = matches!(tok.kind, L_PAREN | L_BRACK | L_BRACE | STRING_START | HOLE_START);
        if opens && tok.partner != NONE && tok.partner as usize > self.pos {
            let partner = tok.partner as usize;
            while self.pos <= partner && self.pos + 1 < self.toks.len() {
                self.bump();
            }
        } else {
            self.bump();
        }
    }

    /// Wraps everything up to the next safe point in an error node. Consumes at least one
    /// token unless already at a safe point.
    pub(crate) fn skip_to_safe_point(&mut self) {
        if self.at_safe_point() {
            return;
        }
        let marker = self.start();
        while !self.at_safe_point() {
            self.skip_one_balanced();
        }
        self.complete(marker, ERROR);
    }

    /// Wraps everything up to (not including) position `end` in an error node.
    pub(crate) fn skip_until(&mut self, end: usize) {
        if self.pos >= end {
            return;
        }
        let marker = self.start();
        while self.pos < end && self.pos + 1 < self.toks.len() {
            self.bump();
        }
        self.complete(marker, ERROR);
    }

    /// Expects the bracket that closes the opener at position `open`. When something is left
    /// before it, that is reported and skipped. When the opener is never closed, the lexer has
    /// already reported it with a fix, and nothing more is said.
    pub(crate) fn close_bracket(&mut self, open: usize, what: &str) {
        let Some(partner) = self.partner_of(open) else {
            if self.speculating == 0 {
                self.unclosed.push((self.tok(open).start, self.last_end));
            }
            return;
        };
        if self.pos < partner {
            let closer = self.tok(partner).kind.fixed_text().unwrap_or(")");
            if let Some(diagnostic) = self.expected(codes::EXPECTED_TOKEN, &format!("`{closer}`")) {
                let span = Span::new(self.nth_span(0).start, self.tok(partner).start);
                let diagnostic = diagnostic.note(format!("this `{closer}` closes {what}")).suggest(
                    "remove the extra code",
                    span,
                    "",
                );
                self.push_error(diagnostic);
            }
            self.skip_until(partner);
        }
        if self.pos == partner {
            self.bump();
        }
    }

    // ------------------------------------------------------------------
    // Speculation
    // ------------------------------------------------------------------

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            pos: self.pos,
            split: self.split,
            events: self.events.len(),
            last_end: self.last_end,
            last_kind: self.last_kind,
            last_error_at: self.last_error_at,
            depth: self.depth,
        }
    }

    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.pos = checkpoint.pos;
        self.split = checkpoint.split;
        self.events.truncate(checkpoint.events);
        self.last_end = checkpoint.last_end;
        self.last_kind = checkpoint.last_kind;
        self.last_error_at = checkpoint.last_error_at;
        self.depth = checkpoint.depth;
        self.fuel.set(FUEL);
    }

    /// Runs `parse` from `from` without keeping anything, and returns where it ended. Used only
    /// to compute the extent of a fix. Speculation inside speculation is not done (`None`), so
    /// the cost stays linear in the nesting depth.
    pub(crate) fn measure(
        &mut self,
        from: Checkpoint,
        parse: impl FnOnce(&mut Parser<'t>),
    ) -> Option<u32> {
        if self.speculating > 0 {
            return None;
        }
        let saved = self.checkpoint();
        self.rollback(Checkpoint { events: self.events.len(), ..from });
        self.speculating += 1;
        parse(self);
        self.speculating -= 1;
        let end = self.last_end;
        self.rollback(saved);
        Some(end)
    }

    pub(crate) fn speculating(&self) -> bool {
        self.speculating > 0
    }
}

/// Marks the tokens that come after a line break inside a `(` or `[` that is never closed
/// (see [`Tok::soft_break`]). This follows the lexer's bracket stack (spec/lexical.md, 12.2).
fn mark_soft_breaks(toks: &mut [Tok]) {
    let mut stack: Vec<usize> = Vec::new();
    for index in 0..toks.len() {
        let tok = toks[index];
        if tok.line_break_before {
            if let Some(&open) = stack.last() {
                let open = toks[open];
                toks[index].soft_break =
                    matches!(open.kind, L_PAREN | L_BRACK) && open.partner == NONE;
            }
        }
        match tok.kind {
            L_PAREN | L_BRACK | L_BRACE | HOLE_START | STRING_START => stack.push(index),
            R_PAREN | R_BRACK | R_BRACE | HOLE_END | STRING_END if tok.partner != NONE => {
                let partner = tok.partner as usize;
                while let Some(open) = stack.pop() {
                    if open == partner {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}
