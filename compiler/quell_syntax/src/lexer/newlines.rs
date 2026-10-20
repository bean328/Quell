//! Step 2 of lexing: newline marking and bracket matching (spec/lexical.md, section 12).
//!
//! This pass walks the raw tokens once. It keeps a stack of open brackets, records each
//! bracket's partner, and decides for every line break whether it is a significant `NEWLINE`.
//! It follows the pseudocode of spec/lexical.md, section 12.2:
//!
//! ```text
//! for each token T in order:
//!     if T.has_line_break_before
//!        and P exists and T is not EOF
//!        and innermost_open_bracket() not in { "(", "[" }
//!        and not (P.kind in CONTINUE_AFTER and not is_member_name(P))
//!        and T.kind not in CONTINUE_BEFORE:
//!         emit NEWLINE
//!     emit T
//!     update the bracket stack with T
//!     P = T
//! ```
//!
//! A hole in a string counts as `(` (spec 12.2), and so does a string itself (the only line
//! breaks inside a string are inside its own tokens, which this pass never looks at).
//!
//! When a line break is significant, the first `LINE_BREAK` token since `P` becomes `NEWLINE`.
//! If the only line break is a block comment that spans lines or a `;`, a `NEWLINE` token with
//! no text is inserted right after it.

use quell_diagnostics::{Diagnostic, Span};

use super::Token;
use super::codes;
use super::scanner::RawToken;
use crate::SyntaxKind::{self, *};

/// Marks significant line breaks and matches brackets. Returns the final tokens and, for each
/// token, the index of its partner (`u32::MAX` for none).
pub(crate) fn mark(
    text: &str,
    raw: &[RawToken],
    diags: &mut Vec<Diagnostic>,
) -> (Vec<Token>, Vec<u32>) {
    let mut pass = Pass {
        text,
        out: Vec::with_capacity(raw.len() + 8),
        partners: Vec::with_capacity(raw.len() + 8),
        stack: Vec::new(),
        segments: vec![[0; 3]],
        diags,
    };
    let mut prev: Option<usize> = None;
    let mut prev_prev: Option<usize> = None;
    let mut first_line_break: Option<usize> = None;
    let mut first_other_break: Option<usize> = None;

    for (index, token) in raw.iter().enumerate() {
        let end = raw.get(index + 1).map_or(token.start, |next| next.start);
        if token.kind.is_trivia() {
            let at = pass.push(token.kind, token.start, false);
            match token.kind {
                LINE_BREAK => {
                    first_line_break.get_or_insert(at);
                }
                SEMICOLON => {
                    first_other_break.get_or_insert(at);
                }
                BLOCK_COMMENT if text[token.start as usize..end as usize].contains('\n') => {
                    first_other_break.get_or_insert(at);
                }
                _ => {}
            }
            continue;
        }

        let line_break_before = first_line_break.is_some() || first_other_break.is_some();
        if line_break_before && pass.is_significant(prev, prev_prev, token.kind) {
            if let Some(at) = first_line_break {
                pass.out[at].kind = NEWLINE;
            } else if let Some(at) = first_other_break {
                let start = pass.out.get(at + 1).map_or(token.start, |next| next.start);
                pass.out.insert(at + 1, Token { kind: NEWLINE, start, line_break_before: false });
                pass.partners.insert(at + 1, NO_PARTNER);
            }
        }
        first_line_break = None;
        first_other_break = None;

        let at = pass.push(token.kind, token.start, line_break_before);
        pass.brackets(at, token.kind, Span::new(token.start, end));
        prev_prev = prev;
        prev = Some(at);
    }
    pass.unclosed_at_end();
    (pass.out, pass.partners)
}

pub(crate) const NO_PARTNER: u32 = u32::MAX;

/// An open bracket on the stack.
struct Open {
    kind: SyntaxKind,
    index: usize,
    span: Span,
}

struct Pass<'a, 'd> {
    text: &'a str,
    out: Vec<Token>,
    partners: Vec<u32>,
    stack: Vec<Open>,
    /// For each string or hole on the stack (and the file itself), how many `(`, `[` and `{`
    /// are open inside it. A closer only matches an opener in the same segment, and the counts
    /// make that check O(1).
    segments: Vec<[usize; 3]>,
    diags: &'d mut Vec<Diagnostic>,
}

fn slot(kind: SyntaxKind) -> Option<usize> {
    match kind {
        L_PAREN | R_PAREN => Some(0),
        L_BRACK | R_BRACK => Some(1),
        L_BRACE | R_BRACE => Some(2),
        _ => None,
    }
}

fn closer_text(opener: SyntaxKind) -> &'static str {
    opener.closing_partner().and_then(SyntaxKind::fixed_text).unwrap_or("")
}

impl Pass<'_, '_> {
    fn push(&mut self, kind: SyntaxKind, start: u32, line_break_before: bool) -> usize {
        self.out.push(Token { kind, start, line_break_before });
        self.partners.push(NO_PARTNER);
        self.out.len() - 1
    }

    /// The newline rules of spec/lexical.md, section 12.2, for a token of kind `kind` that has
    /// a line break before it.
    fn is_significant(&self, prev: Option<usize>, prev_prev: Option<usize>, kind: SyntaxKind) -> bool {
        // Rule 1: start or end of file.
        let Some(prev) = prev else { return false };
        if kind == EOF {
            return false;
        }
        // Rule 2: inside parentheses or brackets (a hole or a string counts as parentheses).
        if let Some(open) = self.stack.last() {
            if matches!(open.kind, L_PAREN | L_BRACK | HOLE_START | STRING_START) {
                return false;
            }
        }
        // Rule 3: the previous token says "more to come", unless it is a keyword used as a
        // member name after `.` or `?.`.
        let prev_kind = self.out[prev].kind;
        let member_name = prev_kind.is_keyword()
            && prev_prev.is_some_and(|index| matches!(self.out[index].kind, DOT | QUESTION_DOT));
        if prev_kind.recovered().continues_after() && !member_name {
            return false;
        }
        // Rule 4: this token says "I continue the line above".
        !kind.recovered().continues_before()
    }

    /// Updates the bracket stack with the token at `index`.
    fn brackets(&mut self, index: usize, kind: SyntaxKind, span: Span) {
        match kind {
            L_PAREN | L_BRACK | L_BRACE => {
                if let (Some(slot), Some(counts)) = (slot(kind), self.segments.last_mut()) {
                    counts[slot] += 1;
                }
                self.stack.push(Open { kind, index, span });
            }
            HOLE_START | STRING_START => {
                self.segments.push([0; 3]);
                self.stack.push(Open { kind, index, span });
            }
            R_PAREN | R_BRACK | R_BRACE => self.close_bracket(index, kind, span),
            HOLE_END | STRING_END => self.close_segment(index, kind, span),
            _ => {}
        }
    }

    fn set_partners(&mut self, a: usize, b: usize) {
        self.partners[a] = b as u32;
        self.partners[b] = a as u32;
    }

    fn close_bracket(&mut self, index: usize, kind: SyntaxKind, span: Span) {
        let Some(slot) = slot(kind) else { return };
        let open_in_segment = self.segments.last().map_or(0, |counts| counts[slot]);
        if open_in_segment == 0 {
            let opener = match kind {
                R_PAREN => "(",
                R_BRACK => "[",
                _ => "{",
            };
            let closer = closer_text(match kind {
                R_PAREN => L_PAREN,
                R_BRACK => L_BRACK,
                _ => L_BRACE,
            });
            self.diags.push(
                Diagnostic::error(format!("this `{closer}` doesn't close anything"))
                    .with_code(codes::UNMATCHED_CLOSING_BRACKET)
                    .primary(span, format!("no `{opener}` is open here"))
                    .suggest("remove it", span, ""),
            );
            return;
        }
        while let Some(open) = self.stack.pop() {
            if let (Some(open_slot), Some(counts)) = (slot_of(open.kind), self.segments.last_mut())
            {
                counts[open_slot] -= 1;
            }
            if slot_of(open.kind) == Some(slot) {
                self.set_partners(open.index, index);
                return;
            }
            self.never_closed(&open, Some((span, kind)));
        }
    }

    /// The end of a hole or a string: closes everything opened inside it.
    fn close_segment(&mut self, index: usize, kind: SyntaxKind, span: Span) {
        while let Some(open) = self.stack.pop() {
            match open.kind {
                HOLE_START | STRING_START => {
                    self.segments.pop();
                    if open.kind.closing_partner() == Some(kind) {
                        self.set_partners(open.index, index);
                        return;
                    }
                }
                _ => self.never_closed(&open, Some((span, kind))),
            }
        }
    }

    /// Reports a bracket that is never closed. `before` is the token that ends the place where
    /// the closer should be, or `None` for the end of the file.
    fn never_closed(&mut self, open: &Open, before: Option<(Span, SyntaxKind)>) {
        let closer = closer_text(open.kind);
        let opener = open.kind.fixed_text().unwrap_or("");
        let diagnostic = Diagnostic::error(format!("this `{opener}` is never closed"))
            .with_code(codes::UNCLOSED_BRACKET)
            .primary(open.span, format!("this needs a matching `{closer}`"));
        let diagnostic = match before {
            Some((span, kind)) => {
                let found = kind.fixed_text().unwrap_or("this");
                let found = if kind == STRING_END { "the end of the string" } else { found };
                diagnostic
                    .secondary(span, format!("`{found}` comes before the `{closer}`"))
                    .suggest(format!("add the missing `{closer}`"), Span::at(span.start), closer)
            }
            None => {
                let end = self.text.len() as u32;
                let prefix = if self.text.ends_with('\n') || self.text.is_empty() { "" } else { "\n" };
                diagnostic.suggest(
                    format!("add the missing `{closer}`"),
                    Span::at(end),
                    format!("{prefix}{closer}"),
                )
            }
        };
        self.diags.push(diagnostic);
    }

    fn unclosed_at_end(&mut self) {
        // Report the outermost bracket first, as the reader meets them.
        let open: Vec<Open> = std::mem::take(&mut self.stack);
        for open in &open {
            if slot_of(open.kind).is_some() {
                self.never_closed(open, None);
            }
        }
    }
}

fn slot_of(kind: SyntaxKind) -> Option<usize> {
    slot(kind)
}
