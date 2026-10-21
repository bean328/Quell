//! Code examples in doc comments (spec/lexical.md, section 4.3).
//!
//! The lexer checks that every fenced block in a run of doc comments is closed. Later stages
//! use [`doc_examples`] to get the code examples out, lex and parse each one as a separate
//! entry file, and report its errors at their place in the original file.

use quell_diagnostics::{Diagnostic, Span};

use super::Lexed;
use super::codes;
use super::scanner::RawToken;
use crate::SyntaxKind::{self, *};

/// One code example from a doc comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocExample {
    /// The source text of the example: each line of its body without `///` and at most one
    /// space after it, joined with line breaks.
    pub text: String,
    /// Where the opening fence (the doc comment line with ```` ``` ````) is in the file.
    pub fence: Span,
    /// For each line of `text`: its offset in `text` and the offset in the file it came from.
    lines: Vec<(u32, u32)>,
}

impl DocExample {
    /// The file offset of the character at `offset` in [`text`](Self::text). The line breaks
    /// that join the lines map to the end of their doc comment line.
    pub fn file_offset(&self, offset: u32) -> u32 {
        let line = self.lines.partition_point(|&(start, _)| start <= offset).saturating_sub(1);
        match self.lines.get(line) {
            Some(&(start, file_start)) => file_start + (offset - start.min(offset)),
            None => self.fence.end,
        }
    }

    /// The place in the file of `span`, a span in [`text`](Self::text).
    pub fn file_span(&self, span: Span) -> Span {
        let start = self.file_offset(span.start);
        let end = self.file_offset(span.end).max(start);
        Span::new(start, end)
    }
}

/// The code examples in the doc comments of a lexed file, in order. A fenced block is a code
/// example if its opening fence has no info string or the info string `quell`. Blocks that are
/// never closed are left out (the lexer has reported them).
pub fn doc_examples(lexed: &Lexed<'_>) -> Vec<DocExample> {
    let text = lexed.text();
    let tokens = lexed.tokens();
    let runs = doc_runs(tokens.len(), |index| tokens[index].kind, |index| lexed.span(index));
    let mut examples = Vec::new();
    for run in runs {
        for block in fenced_blocks(text, &run) {
            let Some(close) = block.close else { continue };
            if !matches!(block.info, "" | "quell") {
                continue;
            }
            let mut example = DocExample {
                text: String::new(),
                fence: run[block.open],
                lines: Vec::with_capacity(close - block.open),
            };
            for (number, line) in run[block.open + 1..close].iter().enumerate() {
                if number > 0 {
                    example.text.push('\n');
                }
                let body_start = line.start as usize + 3;
                let body = &text[body_start..line.end as usize];
                let (skip, body) = match body.strip_prefix(' ') {
                    Some(rest) => (1, rest),
                    None => (0, body),
                };
                example.lines.push((example.text.len() as u32, (body_start + skip) as u32));
                example.text.push_str(body);
            }
            examples.push(example);
        }
    }
    examples
}

/// Reports a fenced block in a run of doc comments that is opened and never closed.
pub(crate) fn check_doc_fences(text: &str, raw: &[RawToken], diags: &mut Vec<Diagnostic>) {
    let span = |index: usize| {
        let end = raw.get(index + 1).map_or(text.len() as u32, |next| next.start);
        Span::new(raw[index].start, end)
    };
    for run in doc_runs(raw.len(), |index| raw[index].kind, span) {
        for block in fenced_blocks(text, &run) {
            if block.close.is_some() {
                continue;
            }
            let fence = run[block.open];
            let last = run[run.len() - 1];
            let line_start = text[..fence.start as usize].rfind('\n').map_or(0, |at| at + 1);
            let indent: String = text[line_start..fence.start as usize]
                .chars()
                .take_while(|&c| c == ' ' || c == '\t')
                .collect();
            diags.push(
                Diagnostic::error("this code block in a doc comment is never closed")
                    .with_code(codes::UNCLOSED_DOC_FENCE)
                    .primary(fence, "this ``` opens a code block")
                    .note("every ``` that opens a code block needs a ``` line that closes it")
                    .suggest(
                        "close it with a doc comment line that holds only ```",
                        Span::at(last.end),
                        format!("\n{indent}/// ```"),
                    ),
            );
        }
    }
}

/// The runs of consecutive doc comment lines, each as the spans of its doc comments. Two doc
/// comments are in the same run when only spaces, tabs and exactly one line break separate them.
fn doc_runs(
    len: usize,
    kind: impl Fn(usize) -> SyntaxKind,
    span: impl Fn(usize) -> Span,
) -> Vec<Vec<Span>> {
    let mut runs = Vec::new();
    let mut run: Vec<Span> = Vec::new();
    let mut breaks = 0;
    for index in 0..len {
        match kind(index) {
            DOC_COMMENT => {
                if breaks != 1 && !run.is_empty() {
                    runs.push(std::mem::take(&mut run));
                }
                run.push(span(index));
                breaks = 0;
            }
            WHITESPACE => {}
            LINE_BREAK | NEWLINE => breaks += 1,
            _ => {
                if !run.is_empty() {
                    runs.push(std::mem::take(&mut run));
                }
                breaks = 0;
            }
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

/// A fenced block in a run: the index of its opening fence line, of its closing fence line (if
/// there is one), and its info string.
struct Block<'t> {
    open: usize,
    close: Option<usize>,
    info: &'t str,
}

/// The fenced blocks of one run. Fences pair up in order: the first opens a block, the next one
/// closes it, and so on.
fn fenced_blocks<'t>(text: &'t str, run: &[Span]) -> Vec<Block<'t>> {
    let mut blocks: Vec<Block<'t>> = Vec::new();
    for (line, span) in run.iter().enumerate() {
        let body = &text[span.start as usize + 3..span.end as usize];
        let Some(after) = body.trim_start_matches(' ').strip_prefix("```") else { continue };
        match blocks.last_mut() {
            Some(block) if block.close.is_none() => block.close = Some(line),
            _ => blocks.push(Block { open: line, close: None, info: after.trim_matches(' ') }),
        }
    }
    blocks
}
