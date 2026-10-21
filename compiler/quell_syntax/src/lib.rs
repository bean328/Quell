//! The syntax of Quell: the lexer, the parser and the lossless syntax tree
//! (docs/decisions/0005-front-end-architecture.md).
//!
//! # Parsing
//!
//! [`parse`] reads the text of one file and returns a [`Parse`]: a lossless rowan syntax tree
//! (its text is exactly the input, whatever the input) and every error and warning found by the
//! lexer and the parser, in source order. The parser implements spec/grammar.md, including the
//! resolutions of section 10, every error of section 9.1 and 11 with a fix, and every warning
//! of section 9.2. It never stops at the first error and never panics.
//!
//! ```
//! use quell_syntax::{FileRole, SyntaxKind, parse};
//!
//! let parse = parse("let total = price * 3\nprint(total)\n", FileRole::Entry);
//! assert!(parse.diagnostics().is_empty());
//! assert_eq!(parse.syntax().to_string(), "let total = price * 3\nprint(total)\n");
//! assert_eq!(parse.syntax().kind(), SyntaxKind::FILE);
//! // The typed view: a top-level `let` is an item, `print(total)` a statement.
//! assert_eq!(parse.tree().items().count(), 1);
//! assert_eq!(parse.tree().statements().count(), 1);
//! ```
//!
//! The file's role decides whether top-level statements are allowed (grammar.md, 2.2): an
//! [`FileRole::Entry`] file may hold statements, a [`FileRole::Library`] file only items.
//!
//! [`ast`] has small typed wrappers over the tree's nodes. [`parse_doc_examples`] finds the
//! code examples in doc comments and parses each one as an entry file (lexical.md, 4.3).
//!
//! # The lexer
//!
//! [`lex`] turns the text of one file into a [`Lexed`]: the tokens (each a [`Token`] with a
//! [`SyntaxKind`], a start offset and a line-break flag), the partner of every bracket, and the
//! error messages. It implements spec/lexical.md exactly, including the newline rules: a line
//! break is either a significant [`NEWLINE`](SyntaxKind::NEWLINE) token or a
//! [`LINE_BREAK`](SyntaxKind::LINE_BREAK) that is trivia. The [`lexer`] module docs describe the
//! token stream in detail, including how strings and their holes are represented.
//!
//! ```
//! use quell_syntax::{SyntaxKind, lex};
//!
//! let lexed = lex("print(\"Hi {name}!\")\n");
//! assert!(lexed.diagnostics().is_empty());
//! // Concatenating the tokens gives back the text.
//! let text: String = (0..lexed.len()).map(|index| lexed.text_of(index)).collect();
//! assert_eq!(text, lexed.text());
//! // The `(` knows its `)`.
//! assert_eq!(lexed.kind(lexed.partner(1).unwrap()), SyntaxKind::R_PAREN);
//! ```
//!
//! Other helpers for the stages after the lexer:
//!
//! - [`decode_source`] checks that the bytes of a file are UTF-8.
//! - [`escape_value`], [`parse_format_spec`], [`int_value`], [`float_value`] and [`char_value`]
//!   give the values of literal tokens.
//! - [`doc_examples`] extracts the code examples from doc comments.
//! - [`is_upper_name`] and [`is_contextual_keyword`] answer the questions the grammar asks
//!   about names (spec/lexical.md, sections 5.2 and 6.2).

pub mod ast;
pub mod lexer;
mod parser;
mod syntax_kind;

use std::fmt::Write;

use quell_diagnostics::{Diagnostic, Edit};
use rowan::{GreenNode, NodeOrToken, WalkEvent};

pub use lexer::{
    DocExample, FormatKind, FormatSpec, IntValue, Lexed, Token, char_value, decode_source,
    doc_examples, escape_value, float_value, int_value, is_upper_name, lex, parse_format_spec,
};
pub use parser::NESTING_LIMIT;
pub use syntax_kind::{
    CONTEXTUAL_KEYWORDS, QuellLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    is_contextual_keyword,
};

/// The role of a file (spec/grammar.md, 2.2). Every file has exactly one, given by the build
/// driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileRole {
    /// The file named on the command line, the main file of a program package, or a code
    /// example in a doc comment: it may hold items and statements in any order.
    Entry,
    /// Every other file: it may hold only items. Top-level statements are reported.
    Library,
}

/// The result of parsing one file: the syntax tree and the diagnostics.
#[derive(Clone, Debug)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

/// Parses the text of one file. Never fails and never panics: mistakes are reported in
/// [`Parse::diagnostics`], and the tree still holds every byte of the text.
pub fn parse(text: &str, role: FileRole) -> Parse {
    let lexed = lex(text);
    parse_lexed(&lexed, role)
}

/// Parses a file that is already lexed. The lexer's diagnostics are included in the result.
pub fn parse_lexed(lexed: &Lexed<'_>, role: FileRole) -> Parse {
    let (green, mut diagnostics) = parser::parse_lexed(lexed, role);
    // Show the mistakes in the order the reader meets them (the sort is stable).
    diagnostics
        .sort_by_key(|diagnostic| diagnostic.main_label().map_or(0, |label| label.span.start));
    Parse { green, diagnostics }
}

impl Parse {
    /// The green tree (immutable and cheap to clone).
    pub fn green(&self) -> &GreenNode {
        &self.green
    }

    /// The root of the syntax tree, a node of kind [`SyntaxKind::FILE`].
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// The typed view of the root.
    pub fn tree(&self) -> ast::SourceFile {
        ast::SourceFile { syntax: self.syntax() }
    }

    /// Every error and warning of the lexer and the parser, in source order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// The tree as text, one node or token per line, for snapshot tests:
    ///
    /// ```text
    /// FILE@0..9
    ///   EXPR_STMT@0..8
    ///     CALL_EXPR@0..8
    ///       NAME_EXPR@0..5
    ///         LOWER_IDENT@0..5 "print"
    /// ...
    /// ```
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        let mut depth = 0usize;
        for event in self.syntax().preorder_with_tokens() {
            match event {
                WalkEvent::Enter(element) => {
                    for _ in 0..depth {
                        out.push_str("  ");
                    }
                    let range = element.text_range();
                    let (start, end) = (u32::from(range.start()), u32::from(range.end()));
                    match element {
                        NodeOrToken::Node(node) => {
                            let _ = writeln!(out, "{:?}@{start}..{end}", node.kind());
                            depth += 1;
                        }
                        NodeOrToken::Token(token) => {
                            let _ = writeln!(
                                out,
                                "{:?}@{start}..{end} {:?}",
                                token.kind(),
                                token.text()
                            );
                        }
                    }
                }
                WalkEvent::Leave(NodeOrToken::Node(_)) => depth = depth.saturating_sub(1),
                WalkEvent::Leave(NodeOrToken::Token(_)) => {}
            }
        }
        out
    }
}

/// A code example from a doc comment, parsed as an entry file.
#[derive(Clone, Debug)]
pub struct DocExampleParse {
    /// The example: its text and where it is in the file.
    pub example: DocExample,
    /// The example parsed as an entry file (lexical.md, 4.3). Its spans are offsets in
    /// `example.text`; [`DocExampleParse::diagnostics_in_file`] maps them to the file.
    pub parse: Parse,
}

impl DocExampleParse {
    /// The example's diagnostics, with every label and fix moved to its place in the file
    /// whose text is `file_text`. Text that a fix inserts on a new line gets the doc comment
    /// prefix (`///`) of the line it goes in.
    pub fn diagnostics_in_file(&self, file_text: &str) -> Vec<Diagnostic> {
        self.parse.diagnostics().iter().map(|diagnostic| self.map(diagnostic, file_text)).collect()
    }

    fn map(&self, diagnostic: &Diagnostic, file_text: &str) -> Diagnostic {
        let mut mapped = diagnostic.clone();
        for label in &mut mapped.labels {
            label.span = self.example.file_span(label.span);
        }
        for help in &mut mapped.helps {
            for edit in &mut help.edits {
                let span = self.example.file_span(edit.span);
                let prefix = doc_line_prefix(file_text, span.start);
                *edit = Edit {
                    span,
                    replacement: edit.replacement.replace('\n', &format!("\n{prefix}")),
                };
            }
        }
        mapped
    }
}

/// The start of the doc comment line that holds `offset`: its indentation, `///` and one space.
fn doc_line_prefix(text: &str, offset: u32) -> String {
    let offset = (offset as usize).min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let line = &text[line_start..];
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    format!("{}/// ", &line[..indent_len])
}

/// The code examples in the doc comments of a file (lexical.md, 4.3), each parsed as an entry
/// file. `quell test` runs them.
pub fn parse_doc_examples(text: &str) -> Vec<DocExampleParse> {
    let lexed = lex(text);
    doc_examples(&lexed)
        .into_iter()
        .map(|example| {
            let parse = parse(&example.text, FileRole::Entry);
            DocExampleParse { example, parse }
        })
        .collect()
}
