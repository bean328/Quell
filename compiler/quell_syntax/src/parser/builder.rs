//! The tree builder: turns the parser's events into a rowan green tree.
//!
//! The parser's events hold only the tokens the grammar reads. The builder puts the trivia
//! (whitespace, removed line breaks, comments, skipped text) back between them, so the tree's
//! text is exactly the input. Trivia goes into the parent of the next node, before it, with
//! one exception: the doc comments right before an item, field, method, enum option or trait
//! member go inside that node, before its attributes (spec/grammar.md, 3.1). Doc comments that
//! end up anywhere else document nothing, and get a warning (spec/lexical.md, 4.3).

use quell_diagnostics::{Diagnostic, Edit, Span};
use rowan::{GreenNode, GreenNodeBuilder};

use super::Event;
use super::codes;
use crate::Lexed;
use crate::SyntaxKind::{self, *};

/// Builds the tree from `events` and returns it with the diagnostics found in the events and
/// by the builder.
pub(crate) fn build_tree(
    lexed: &Lexed<'_>,
    mut events: Vec<Event>,
) -> (GreenNode, Vec<Diagnostic>) {
    let mut builder = Builder {
        lexed,
        next: 0,
        offset: 0,
        green: GreenNodeBuilder::new(),
        open: 0,
        loose_docs: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let mut parents: Vec<SyntaxKind> = Vec::new();
    for index in 0..events.len() {
        match std::mem::replace(&mut events[index], Event::Finish) {
            Event::Start { kind, forward_parent } => {
                // Collect the chain of nodes that must wrap this one (made by `precede`) and
                // start them from the outermost in.
                parents.clear();
                parents.extend(kind);
                let mut at = index;
                let mut forward = forward_parent;
                while forward != 0 {
                    at += forward as usize;
                    let event = std::mem::replace(
                        &mut events[at],
                        Event::Start { kind: None, forward_parent: 0 },
                    );
                    match event {
                        Event::Start { kind, forward_parent } => {
                            parents.extend(kind);
                            forward = forward_parent;
                        }
                        other => {
                            // Not possible: `precede` always points at a `Start`.
                            events[at] = other;
                            break;
                        }
                    }
                }
                for &kind in parents.iter().rev() {
                    builder.start_node(kind);
                }
            }
            Event::Token { kind, end } => builder.token(kind, end),
            Event::Finish => builder.finish_node(),
            Event::Error(diagnostic) => diagnostics.push(*diagnostic),
        }
    }
    builder.finish(&mut diagnostics)
}

struct Builder<'a, 'l> {
    lexed: &'a Lexed<'l>,
    /// The index of the next lexer token not yet fully in the tree.
    next: usize,
    /// Where the text not yet in the tree starts (inside token `next` when a `>>` was split).
    offset: u32,
    green: GreenNodeBuilder<'static>,
    /// How many nodes are open.
    open: usize,
    /// The lexer indexes of doc comments that ended up outside a documented element.
    loose_docs: Vec<usize>,
}

/// The nodes that take the doc comments right before them.
fn is_documented(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        FN_DECL
            | STRUCT_DECL
            | ENUM_DECL
            | TRAIT_DECL
            | EXTEND_DECL
            | IMPORT_DECL
            | TEST_DECL
            | COMPTIME_BLOCK
            | GLOBAL_LET
            | FIELD
            | ENUM_OPTION
    )
}

impl Builder<'_, '_> {
    /// Adds the trivia before the next grammar token, up to (not including) lexer token `until`.
    fn trivia(&mut self, until: usize) {
        while self.next < until && self.lexed.kind(self.next).is_trivia() {
            let kind = self.lexed.kind(self.next);
            if kind == DOC_COMMENT {
                self.loose_docs.push(self.next);
            }
            let text = self.lexed.text_of(self.next);
            self.green.token(kind.into(), text);
            self.next += 1;
            self.offset = self.lexed.span(self.next).start;
        }
    }

    /// The index of the next lexer token that is not trivia.
    fn next_grammar_token(&self) -> usize {
        let mut index = self.next;
        while index + 1 < self.lexed.len() && self.lexed.kind(index).is_trivia() {
            index += 1;
        }
        index
    }

    fn start_node(&mut self, kind: SyntaxKind) {
        if self.open > 0 {
            let until = if is_documented(kind) {
                // Leave the doc comments (and what follows them) for the node itself.
                let end = self.next_grammar_token();
                (self.next..end).find(|&i| self.lexed.kind(i) == DOC_COMMENT).unwrap_or(end)
            } else {
                self.next_grammar_token()
            };
            self.trivia(until);
        }
        self.green.start_node(kind.into());
        self.open += 1;
        if is_documented(kind) {
            // The doc comments now go inside this node.
            let end = self.next_grammar_token();
            let before = self.loose_docs.len();
            self.trivia(end);
            self.loose_docs.truncate(before);
        }
    }

    fn token(&mut self, kind: SyntaxKind, end: u32) {
        let index = self.next_grammar_token();
        self.trivia(index);
        let token_end = self.lexed.span(self.next).end.max(self.offset);
        let end = end.max(self.offset).min(token_end);
        let text = self.lexed.text().get(self.offset as usize..end as usize).unwrap_or("");
        self.green.token(kind.into(), text);
        self.offset = end;
        if end >= token_end {
            self.next += 1;
            self.offset = self.lexed.span(self.next).start;
        }
    }

    fn finish_node(&mut self) {
        if self.open > 1 {
            self.green.finish_node();
            self.open -= 1;
        }
    }

    fn finish(mut self, diagnostics: &mut Vec<Diagnostic>) -> (GreenNode, Vec<Diagnostic>) {
        if self.open == 0 {
            self.green.start_node(FILE.into());
            self.open = 1;
        }
        // Everything left (normally nothing) goes into the root, so the tree keeps every byte.
        while self.next < self.lexed.len() {
            let kind = self.lexed.kind(self.next);
            if kind.is_trivia() {
                self.trivia(self.next + 1);
                continue;
            }
            let end = self.lexed.span(self.next).end;
            self.token(kind, end);
        }
        while self.open > 1 {
            self.green.finish_node();
            self.open -= 1;
        }
        self.green.finish_node();
        let green = self.green.finish();
        diagnostics.extend(loose_doc_warnings(self.lexed, &self.loose_docs));
        (green, std::mem::take(diagnostics))
    }
}

/// One warning for each run of doc comment lines that documents nothing.
fn loose_doc_warnings(lexed: &Lexed<'_>, loose: &[usize]) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for &index in loose {
        if run.last().is_some_and(|&previous| !same_run(lexed, previous, index)) {
            warnings.push(loose_doc_warning(lexed, &run));
            run.clear();
        }
        run.push(index);
    }
    if !run.is_empty() {
        warnings.push(loose_doc_warning(lexed, &run));
    }
    warnings
}

/// Whether two doc comments are lines of one run: only spaces and exactly one line break
/// between them.
fn same_run(lexed: &Lexed<'_>, first: usize, second: usize) -> bool {
    let mut breaks = 0;
    for index in first + 1..second {
        match lexed.kind(index) {
            WHITESPACE => {}
            LINE_BREAK | NEWLINE => breaks += 1,
            _ => return false,
        }
    }
    breaks == 1
}

/// The warning for one run of doc comments (the lexer indexes of its lines).
fn loose_doc_warning(lexed: &Lexed<'_>, run: &[usize]) -> Diagnostic {
    let first = lexed.span(run[0]);
    let lines = if run.len() == 1 { "this doc comment" } else { "these doc comments" };
    // Each `///` becomes `//`: the text stays, as a plain comment.
    let edits = run
        .iter()
        .map(|&index| {
            let start = lexed.span(index).start;
            Edit { span: Span::new(start, start + 3), replacement: "//".to_string() }
        })
        .collect();
    Diagnostic::warning("this doc comment doesn't document anything")
        .with_code(codes::DOC_COMMENT_DOCUMENTS_NOTHING)
        .primary(first, "`///` documents the item, field, method or option right after it")
        .note(
            "doc comments go right before a function, a type, a field, an enum option or a \
             method; inside a function, use `//` for a plain comment",
        )
        .suggest_edits(
            format!("to keep the text as a plain comment, change {lines} to `//`"),
            edits,
        )
}
