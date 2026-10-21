//! The Quell lexer: source text to tokens (spec/lexical.md).
//!
//! Call [`lex`] with the text of one file. It returns a [`Lexed`]: every token of the file with
//! its [`SyntaxKind`] and start offset, the matching partner of every bracket, and the
//! [`Diagnostic`]s for every mistake it found. Lexing never fails and never panics: after a
//! mistake the lexer reports it, produces the most likely intended token, and carries on.
//!
//! # Lossless
//!
//! Every character of the input belongs to exactly one token, trivia included. A token runs
//! from its start to the start of the next token, so concatenating the text of all tokens gives
//! back the input byte for byte, whatever the input. The last token is always an empty
//! [`EOF`](SyntaxKind::EOF).
//!
//! # Trivia and the tokens the parser reads
//!
//! Whitespace, comments, doc comments, removed line breaks, a byte order mark, `;` and skipped
//! error text are trivia ([`SyntaxKind::is_trivia`]). The grammar never sees them; the parser
//! skips them but keeps them in the tree. Everything else is a token the grammar reads.
//!
//! # Significant line breaks
//!
//! The newline rules (spec/lexical.md, section 12) are already applied. Every line break in
//! code is one of two tokens:
//!
//! - [`NEWLINE`](SyntaxKind::NEWLINE): a significant line break, which the grammar reads.
//! - [`LINE_BREAK`](SyntaxKind::LINE_BREAK): a line break the rules removed (trivia).
//!
//! When several line breaks come between two tokens, at most the first one becomes `NEWLINE`.
//! A block comment that spans lines, and a `;` (always an error), also count as a line break;
//! when one of them is the only break, an empty `NEWLINE` token follows it.
//!
//! Lines ending in `>` or `>>` keep their `NEWLINE`, because the lexer can't tell a generic `>`
//! from a comparison (spec 12.5). When the parser uses such a token as a binary operator, it
//! skips the `NEWLINE` right after it ([`SyntaxKind::newline_left_to_parser`]).
//!
//! Every token also records whether a line break (of any kind) came before it
//! ([`Token::line_break_before`]). The parser uses this for the rule that the `{` of a keyword
//! construct must be on the same line, which applies inside `( )` too (spec 12.6).
//!
//! # Brackets
//!
//! [`Lexed::partner`] gives the matching closer of every `(`, `[` and `{` and the opener of
//! every closer (or `None` if there is none; the lexer has reported that). It does the same for
//! [`STRING_START`](SyntaxKind::STRING_START) and [`STRING_END`](SyntaxKind::STRING_END), and
//! for [`HOLE_START`](SyntaxKind::HOLE_START) and [`HOLE_END`](SyntaxKind::HOLE_END). The parser
//! uses this to look past a pair of parentheses in one step (grammar.md, 6.1).
//!
//! # Strings
//!
//! A string literal (the grammar's `STRING`) is a run of tokens from `STRING_START` to its
//! partner `STRING_END`. Between them come the pieces of the string, in order:
//!
//! | Kind | Text | Value |
//! |---|---|---|
//! | `STRING_START` | `"`, `#"`, `##"`, ...; for a multi-line string `"""` or `#"""` with the spaces and the line break after it | none |
//! | `STRING_TEXT` | literal text | its text |
//! | `STRING_ESCAPE` | `\n`, `\u{E9}`, ..., or a doubled brace `{{` or `}}` | [`escape_value`] |
//! | `STRING_INDENT` | indentation that a multi-line string removes | empty |
//! | `STRING_LINE_BREAK` | a line break inside a multi-line string | `\n` |
//! | `HOLE_START` | the `{` of a hole | see below |
//! | `FORMAT_SPEC` | the `:` of a hole and the spec after it, such as `:.2` | [`parse_format_spec`] of the text after `:` |
//! | `HOLE_END` | the `}` of a hole | |
//! | `STRING_END` | `"`, `"#`, ...; for a multi-line string the last line break, the closing line's indentation and `"""`; empty if the string is never closed | none |
//!
//! A hole is `HOLE_START`, then the ordinary tokens of its expression (with whitespace trivia
//! between them, and nested strings as token runs of their own), then an optional
//! `FORMAT_SPEC`, then `HOLE_END`. The parser parses the tokens between `HOLE_START` and
//! `FORMAT_SPEC` or `HOLE_END` as exactly one expression (grammar.md, 6.13). A hole never holds
//! a line break. A `PLAIN_STRING` is a string with no `HOLE_START` before its `STRING_END`;
//! [`Lexed::plain_string_value`] gives its value.
//!
//! For example, `"Total: {sum:.2}!"` is `STRING_START` `STRING_TEXT("Total: ")` `HOLE_START`
//! `LOWER_IDENT("sum")` `FORMAT_SPEC(":.2")` `HOLE_END` `STRING_TEXT("!")` `STRING_END`.
//! A multi-line string
//!
//! ```text
//! let poem = """
//!     Roses are red,
//!       Quell is fast.
//!     """
//! ```
//!
//! is `STRING_START("\"\"\"\n")` `STRING_INDENT("    ")` `STRING_TEXT("Roses are red,")`
//! `STRING_LINE_BREAK` `STRING_INDENT("    ")` `STRING_TEXT("  Quell is fast.")`
//! `STRING_END("\n    \"\"\"")`, whose value is `"Roses are red,\n  Quell is fast."`.
//!
//! # Errors
//!
//! Every lexer error has a stable code from `Q0001` to `Q0099`, listed in docs/errors.md. The
//! tokens that exist only to give a helpful error (spec 11) are kept with kinds of their own
//! (`&&` is [`AMP_AMP`](SyntaxKind::AMP_AMP), and so on). The lexer has already reported them;
//! the parser reads each one as [`SyntaxKind::recovered`] says (`&&` as `and`, and so on).
//!
//! # Example
//!
//! ```
//! use quell_syntax::{SyntaxKind, lex};
//!
//! let lexed = lex("let total = price +\n    tax\nprint(total)\n");
//! let kinds: Vec<SyntaxKind> = lexed.non_trivia().map(|index| lexed.kind(index)).collect();
//! use SyntaxKind::*;
//! assert_eq!(
//!     kinds,
//!     [LET_KW, LOWER_IDENT, EQ, LOWER_IDENT, PLUS, LOWER_IDENT, NEWLINE,
//!      LOWER_IDENT, L_PAREN, LOWER_IDENT, R_PAREN, EOF],
//! );
//! assert!(lexed.diagnostics().is_empty());
//! ```

mod chars;
pub(crate) mod codes;
mod doc;
mod newlines;
mod nfc;
mod nfc_tables;
mod scanner;
mod values;

#[cfg(test)]
mod tests;

use quell_diagnostics::{Diagnostic, Span};

use crate::SyntaxKind;

pub use chars::is_upper_name;
pub use doc::{DocExample, doc_examples};
pub use values::{
    FormatKind, FormatSpec, IntValue, char_value, escape_value, float_value, int_value,
    parse_format_spec,
};

/// One token: its kind, where it starts, and whether a line break came before it.
///
/// A token's text runs from `start` to the start of the next token (see [`Lexed::text_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// What kind of token this is.
    pub kind: SyntaxKind,
    /// The byte offset of its first character.
    pub start: u32,
    /// Whether at least one line break (a line break, a block comment that spans lines, or a
    /// `;`) came between the previous non-trivia token and this one. Always `false` for trivia.
    pub line_break_before: bool,
}

/// The result of lexing one file. See the module docs.
#[derive(Debug, Clone)]
pub struct Lexed<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    /// For each token, the index of its partner, or `NO_PARTNER`.
    partners: Vec<u32>,
    diagnostics: Vec<Diagnostic>,
}

/// The largest text the lexer reads: offsets are `u32`, as in `rowan` and `quell_diagnostics`.
const MAX_LEN: usize = u32::MAX as usize - 1;

/// Turns the text of one Quell file into tokens. See the module docs.
///
/// This is one pass over the text plus one pass over the tokens, in linear time. It never
/// panics. A text of 4 GiB or more is not lexed: the result is one `ERROR` token and an error.
pub fn lex(text: &str) -> Lexed<'_> {
    if text.len() > MAX_LEN {
        let diagnostic = Diagnostic::error("this file is too large to compile")
            .with_code(codes::FILE_TOO_LARGE)
            .primary(Span::at(0), "the file starts here")
            .note("a Quell source file must be smaller than 4 GiB")
            .help("split the code into several files");
        let tokens = vec![
            Token { kind: SyntaxKind::ERROR, start: 0, line_break_before: false },
            Token { kind: SyntaxKind::EOF, start: MAX_LEN as u32, line_break_before: false },
        ];
        return Lexed {
            text,
            tokens,
            partners: vec![newlines::NO_PARTNER; 2],
            diagnostics: vec![diagnostic],
        };
    }
    let (raw, mut diagnostics) = scanner::scan(text);
    doc::check_doc_fences(text, &raw, &mut diagnostics);
    let (tokens, partners) = newlines::mark(text, &raw, &mut diagnostics);
    // Show the mistakes in the order the reader meets them.
    diagnostics
        .sort_by_key(|diagnostic| diagnostic.main_label().map_or(0, |label| label.span.start));
    Lexed { text, tokens, partners, diagnostics }
}

/// Checks that the bytes of a source file are UTF-8 (spec/lexical.md, section 2) and returns
/// them as text.
///
/// The error points at the first bad byte with an empty span. To show it, render it with a
/// `SourceFile` made from the valid part before that byte, `&bytes[..span.start]`.
pub fn decode_source(bytes: &[u8]) -> Result<&str, Diagnostic> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(error) => {
            let at = error.valid_up_to().min(MAX_LEN) as u32;
            let mut diagnostic = Diagnostic::error("this file is not valid UTF-8 text")
                .with_code(codes::INVALID_UTF8)
                .primary(Span::at(at), format!("the first bad byte is at byte offset {at}"));
            if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
                diagnostic = diagnostic.note("this file looks like UTF-16 text");
            }
            Err(diagnostic.help("save the file as UTF-8; most editors offer this when saving"))
        }
    }
}

impl<'a> Lexed<'a> {
    /// The text that was lexed.
    pub fn text(&self) -> &'a str {
        self.text
    }

    /// Every token, trivia included, in order. The last one is `EOF`.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// The number of tokens, `EOF` included (so never 0).
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Always `false`: there is at least the `EOF` token.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The errors found while lexing, in the order of their place in the text.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any of the diagnostics is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// Takes the diagnostics out, for a caller that collects them from several stages.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    /// The kind of token `index`. Past the end, `EOF`.
    pub fn kind(&self, index: usize) -> SyntaxKind {
        self.tokens.get(index).map_or(SyntaxKind::EOF, |token| token.kind)
    }

    /// Whether a line break came before token `index` (see [`Token::line_break_before`]).
    pub fn line_break_before(&self, index: usize) -> bool {
        self.tokens.get(index).is_some_and(|token| token.line_break_before)
    }

    /// Where token `index` starts and ends.
    pub fn span(&self, index: usize) -> Span {
        let end_of_text = self.text.len().min(MAX_LEN) as u32;
        let Some(token) = self.tokens.get(index) else { return Span::at(end_of_text) };
        let end = self.tokens.get(index + 1).map_or(token.start, |next| next.start);
        Span::new(token.start, end.max(token.start))
    }

    /// The text of token `index`.
    pub fn text_of(&self, index: usize) -> &'a str {
        let span = self.span(index);
        self.text.get(span.start as usize..span.end as usize).unwrap_or("")
    }

    /// The index of the token that matches token `index`: the closer of an opening bracket,
    /// the opener of a closing bracket, and likewise for `HOLE_START` and `HOLE_END` and for
    /// `STRING_START` and `STRING_END`. `None` for other tokens and for brackets that are never
    /// matched (the lexer has reported those).
    pub fn partner(&self, index: usize) -> Option<usize> {
        match self.partners.get(index) {
            Some(&partner) if partner != newlines::NO_PARTNER => Some(partner as usize),
            _ => None,
        }
    }

    /// The indexes of the tokens the grammar reads (everything but trivia), `EOF` included.
    pub fn non_trivia(&self) -> impl Iterator<Item = usize> + '_ {
        self.tokens.iter().enumerate().filter(|(_, t)| !t.kind.is_trivia()).map(|(index, _)| index)
    }

    /// The first non-trivia token at `index` or after it. Past the end, the index of `EOF`.
    pub fn next_non_trivia(&self, index: usize) -> usize {
        let last = self.tokens.len() - 1;
        (index..last).find(|&i| !self.tokens[i].kind.is_trivia()).unwrap_or(last)
    }

    /// The last non-trivia token before `index`, if there is one.
    pub fn prev_non_trivia(&self, index: usize) -> Option<usize> {
        (0..index.min(self.tokens.len())).rev().find(|&i| !self.tokens[i].kind.is_trivia())
    }

    /// The spaces and tabs at the start of the line that holds `offset`. The parser uses this
    /// for the warning about a line that starts with `-` or `~` (spec/lexical.md, 12.6), the
    /// one place outside multi-line strings where indentation matters.
    pub fn indentation_of_line(&self, offset: u32) -> &'a str {
        let offset = (offset as usize).min(self.text.len());
        let bytes = self.text.as_bytes();
        let start = bytes[..offset].iter().rposition(|&b| b == b'\n').map_or(0, |at| at + 1);
        let end = bytes[start..].iter().position(|&b| b != b' ' && b != b'\t');
        let end = end.map_or(bytes.len(), |len| start + len);
        &self.text[start..end]
    }

    /// The value of the string literal whose `STRING_START` is token `start`, if it is a plain
    /// string (no holes) and all its escapes are valid. The grammar requires a plain string after
    /// `test` and `import c` and in patterns.
    pub fn plain_string_value(&self, start: usize) -> Option<String> {
        if self.kind(start) != SyntaxKind::STRING_START {
            return None;
        }
        let end = self.partner(start)?;
        let mut value = String::new();
        for index in start + 1..end {
            let text = self.text_of(index);
            match self.kind(index) {
                SyntaxKind::STRING_TEXT => value.push_str(text),
                SyntaxKind::STRING_ESCAPE => value.push(escape_value(text)?),
                SyntaxKind::STRING_LINE_BREAK => value.push('\n'),
                SyntaxKind::STRING_INDENT => {}
                _ => return None,
            }
        }
        Some(value)
    }

    /// Whether the string literal whose `STRING_START` is token `start` has no holes.
    pub fn is_plain_string(&self, start: usize) -> bool {
        self.kind(start) == SyntaxKind::STRING_START
            && self.partner(start).is_some_and(|end| {
                (start + 1..end).all(|index| self.kind(index) != SyntaxKind::HOLE_START)
            })
    }
}
