//! Reading indentation to find where a block ends.
//!
//! Indentation has no meaning in Quell (spec/lexical.md, 3), but people indent their code, and
//! the indentation shows what they meant where the braces don't. The parser looks at it only
//! after an error has been found, so it never changes how valid code parses:
//!
//! - **A `{` that is never closed** (spec/lexical.md, 12.2). The lexer matches each `}` with
//!   the nearest open `{`, so when one `}` is missing it reports the outermost `{`, which is
//!   often far from the mistake, and everything after the mistake seems to be inside the wrong
//!   block. [`repair_braces`] matches the braces again and closes a block at the first line that
//!   is not indented more than the line with its `{`. When that explains the code with no more
//!   errors than the lexer's matching, the parser uses it: the error points at the `{` whose
//!   block really ends early, the fix adds the `}` on a new line where the block ends, and the
//!   code after it is read where it belongs.
//! - **A block written the Python way**, with `:` and indented lines. [`indented_block_end`]
//!   finds where it ends, so the parser can read it as a block and report it once.

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::fixes::indentation_at;
use super::{NONE, Tok};
use crate::SyntaxKind::{self, *};

/// A `{` that the indentation shows is never closed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct UnclosedBrace {
    /// The position of the `{` in the parser's token list.
    pub(crate) open: usize,
    /// The position of the first token after the block: the first token of the line that is
    /// not indented more than the line with the `{`, or `EOF`.
    pub(crate) end: usize,
    /// The `}` the lexer matched this `{` with, if any.
    pub(crate) lexer_partner: Option<usize>,
    /// The bracket around the `{` is a `(` or `[`, where line breaks are not significant.
    in_parens: bool,
}

/// The number of spaces and tabs at the start of the line that holds `offset`.
pub(crate) fn indent_width(text: &str, offset: u32) -> usize {
    indentation_at(text, offset).len()
}

/// Which slot of the bracket counts a bracket kind uses: `(`, `[` or `{`.
fn slot(kind: SyntaxKind) -> Option<usize> {
    match kind {
        L_PAREN | R_PAREN => Some(0),
        L_BRACK | R_BRACK => Some(1),
        L_BRACE | R_BRACE => Some(2),
        _ => None,
    }
}

/// The result of matching the brackets again with the indentation rule.
struct Matching {
    partners: Vec<u32>,
    unclosed: Vec<UnclosedBrace>,
    /// Brackets never closed (by a closer or by the indentation) and closers of nothing.
    errors: usize,
}

/// When the lexer found a `{` that is never closed, matches the braces again using the
/// indentation (see the module docs). If the new matching has no more errors than the lexer's
/// and only changes which `{` goes with which `}`, it replaces the partners in `toks`, gives
/// each block that the indentation closes a virtual end, and returns those blocks.
pub(crate) fn repair_braces(toks: &mut [Tok], text: &str) -> Vec<UnclosedBrace> {
    if !toks.iter().any(|tok| tok.kind == L_BRACE && tok.partner == NONE) {
        return Vec::new();
    }
    let matching = match_by_indentation(toks, text);
    let lexer_errors =
        toks.iter().filter(|tok| slot(tok.kind).is_some() && tok.partner == NONE).count();
    if matching.errors > lexer_errors {
        return Vec::new();
    }
    let same_elsewhere = toks
        .iter()
        .zip(&matching.partners)
        .all(|(tok, &partner)| matches!(tok.kind, L_BRACE | R_BRACE) || tok.partner == partner);
    if !same_elsewhere {
        return Vec::new();
    }
    let mut unclosed = matching.unclosed;
    for brace in &mut unclosed {
        let partner = toks[brace.open].partner;
        brace.lexer_partner = (partner != NONE).then_some(partner as usize);
    }
    for (tok, &partner) in toks.iter_mut().zip(&matching.partners) {
        if matches!(tok.kind, L_BRACE | R_BRACE) {
            tok.partner = partner;
        }
    }
    for brace in &unclosed {
        // Inside `( )` the closer that ends the block has no `NEWLINE` before it once the `}`
        // is there, so the list stops right at it.
        let end = if brace.in_parens { brace.end } else { virtual_end(toks, brace.end) };
        toks[brace.open].vend = end as u32;
    }
    unclosed
}

/// Where a list that ends before the token at `end` stops: before the `NEWLINE` that comes
/// right before that token, if there is one, so the enclosing list sees the line break.
fn virtual_end(toks: &[Tok], end: usize) -> usize {
    if end > 0 && toks[end - 1].kind == NEWLINE { end - 1 } else { end }
}

/// Matches the brackets the way the lexer does (spec/lexical.md, 12.2), with one more rule: a
/// `{` is closed before the first line, inside it, that is not indented more than the line
/// with the `{` (unless that line starts with the `}` that closes it).
fn match_by_indentation(toks: &[Tok], text: &str) -> Matching {
    let mut partners = vec![NONE; toks.len()];
    // Open brackets, with the indentation of the line of a `{`.
    let mut stack: Vec<(usize, Option<usize>)> = Vec::new();
    // For each string or hole (and the file), how many `(`, `[` and `{` are open in it.
    let mut segments: Vec<[usize; 3]> = vec![[0; 3]];
    let mut unclosed = Vec::new();
    let mut errors = 0;
    for (pos, tok) in toks.iter().enumerate() {
        if tok.line_break_before || tok.kind == EOF {
            let width = if tok.kind == EOF { 0 } else { indent_width(text, tok.start) };
            while let Some(&(open, Some(indent))) = stack.last() {
                if width > indent || (width == indent && tok.kind == R_BRACE) {
                    break;
                }
                stack.pop();
                if let Some(counts) = segments.last_mut() {
                    counts[2] = counts[2].saturating_sub(1);
                }
                let in_parens =
                    stack.last().is_some_and(|&(outer, _)| matches!(toks[outer].kind, L_PAREN | L_BRACK));
                unclosed.push(UnclosedBrace { open, end: pos, lexer_partner: None, in_parens });
                errors += 1;
            }
        }
        match tok.kind {
            L_PAREN | L_BRACK | L_BRACE => {
                if let (Some(slot), Some(counts)) = (slot(tok.kind), segments.last_mut()) {
                    counts[slot] += 1;
                }
                let indent = (tok.kind == L_BRACE).then(|| indent_width(text, tok.start));
                stack.push((pos, indent));
            }
            HOLE_START | STRING_START => {
                segments.push([0; 3]);
                stack.push((pos, None));
            }
            R_PAREN | R_BRACK | R_BRACE => {
                let Some(wanted) = slot(tok.kind) else { continue };
                if segments.last().is_none_or(|counts| counts[wanted] == 0) {
                    errors += 1;
                    continue;
                }
                while let Some((open, _)) = stack.pop() {
                    let open_slot = slot(toks[open].kind);
                    if let (Some(open_slot), Some(counts)) = (open_slot, segments.last_mut()) {
                        counts[open_slot] = counts[open_slot].saturating_sub(1);
                    }
                    if open_slot == Some(wanted) {
                        partners[open] = pos as u32;
                        partners[pos] = open as u32;
                        break;
                    }
                    errors += 1;
                }
            }
            HOLE_END | STRING_END => {
                while let Some((open, _)) = stack.pop() {
                    match toks[open].kind {
                        HOLE_START | STRING_START => {
                            segments.pop();
                            if toks[open].kind.closing_partner() == Some(tok.kind) {
                                partners[open] = pos as u32;
                                partners[pos] = open as u32;
                                break;
                            }
                        }
                        _ => errors += 1,
                    }
                }
            }
            _ => {}
        }
    }
    errors += stack.iter().filter(|(open, _)| slot(toks[*open].kind).is_some()).count();
    Matching { partners, unclosed, errors }
}

/// The error for a `{` that [`repair_braces`] found is never closed, with a fix that adds the
/// `}` on a new line after the block's last line.
pub(crate) fn unclosed_brace_error(text: &str, toks: &[Tok], brace: UnclosedBrace) -> Diagnostic {
    let open = toks[brace.open];
    let open_span = Span::new(open.start, open.end);
    let end = toks[brace.end.min(toks.len() - 1)];
    let mut diagnostic = Diagnostic::error("this `{` is never closed")
        .with_code(crate::lexer::codes::UNCLOSED_BRACKET)
        .primary(open_span, "this needs a matching `}`");
    let outer = "judging by its indentation, this `}` closes an outer block";
    if end.kind != EOF {
        let end_span = Span::new(end.start, end.end);
        if brace.lexer_partner == Some(brace.end) {
            diagnostic = diagnostic.secondary(end_span, outer);
        } else {
            diagnostic = diagnostic.secondary(
                end_span,
                "the block ends before this line, which is not indented more than the `{` line",
            );
            if let Some(partner) = brace.lexer_partner {
                let partner = toks[partner];
                diagnostic = diagnostic.secondary(Span::new(partner.start, partner.end), outer);
            }
        }
    }
    // The last token of the block (the `{` itself if the block is empty).
    let last = (brace.open..brace.end).rev().find(|&pos| toks[pos].kind != NEWLINE);
    let last_end = last.map_or(open.end, |pos| toks[pos].end);
    let indent = indentation_at(text, open.start);
    let one_line = !text[open.start as usize..last_end as usize].contains('\n');
    let edit = if one_line && last.is_some_and(|pos| pos != brace.open) {
        // `{ code` with nothing after it on later lines: close it on the same line.
        Edit { span: Span::at(last_end), replacement: " }".to_string() }
    } else {
        closing_line(text, last_end, indent)
    };
    diagnostic.suggest_edits("add the missing `}` where the block ends", vec![edit])
}

/// The edit that adds a line holding `indent` and `}` after the line on which the code ending
/// at `after` stands: at the start of the line break after it, past any comment on that line.
pub(crate) fn closing_line(text: &str, after: u32, indent: &str) -> Edit {
    let bytes = text.as_bytes();
    let end = bytes.len();
    let mut at = after as usize;
    while at < end {
        match bytes[at] {
            // Spaces, and `;`, which the lexer reported and reads as a line break.
            b' ' | b'\t' | b';' => at += 1,
            b'\n' => return line_edit(at, "\n", indent),
            b'\r' if bytes.get(at + 1) == Some(&b'\n') => return line_edit(at, "\r\n", indent),
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                while at < end && !matches!(bytes[at], b'\n' | b'\r') {
                    at += 1;
                }
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                // A block comment (they nest). If it spans lines, the line ends after it.
                let start = at;
                let mut depth = 0usize;
                while at < end {
                    if bytes[at] == b'/' && bytes.get(at + 1) == Some(&b'*') {
                        depth += 1;
                        at += 2;
                    } else if bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/') {
                        depth -= 1;
                        at += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        at += 1;
                    }
                }
                if bytes[start..at].contains(&b'\n') {
                    return Edit {
                        span: Span::at(at as u32),
                        replacement: format!("\n{indent}}}\n"),
                    };
                }
            }
            _ => break,
        }
    }
    if at >= bytes.len() {
        // The file ends on the block's last line.
        return Edit { span: Span::at(bytes.len() as u32), replacement: format!("\n{indent}}}") };
    }
    // No line break before the code that follows: put the `}` on a line of its own.
    Edit { span: Span::at(at as u32), replacement: format!("\n{indent}}}\n") }
}

fn line_edit(at: usize, line_break: &str, indent: &str) -> Edit {
    Edit { span: Span::at(at as u32), replacement: format!("{line_break}{indent}}}") }
}

/// Where a block written with `:` ends (spec: Quell has no such blocks; this is only for the
/// error message and for reading the code after it). `colon` is the position of the `:`.
///
/// - If code follows the `:` on the same line, the block is the rest of that line.
/// - If the `:` ends its line and the next line is indented more than the line with the `:`,
///   the block is every line up to the first one that is not indented more.
/// - Otherwise there is no block (`None`).
///
/// The block also ends at a closing bracket that belongs to an enclosing construct. The result
/// is the position where the block's list stops (see [`virtual_end`]) and the position of its
/// last token.
pub(crate) fn indented_block_end(toks: &[Tok], text: &str, colon: usize) -> Option<(usize, usize)> {
    let header_indent = indent_width(text, toks[colon].start);
    let first = colon + 1;
    let first_tok = toks.get(first)?;
    let one_line = !first_tok.line_break_before && first_tok.kind != NEWLINE;
    if !one_line {
        let body = if first_tok.kind == NEWLINE { first + 1 } else { first };
        let body_tok = toks.get(body)?;
        if body_tok.kind == EOF || indent_width(text, body_tok.start) <= header_indent {
            return None;
        }
    }
    let mut pos = first;
    let mut last = colon;
    loop {
        let tok = toks[pos];
        let ends = match tok.kind {
            EOF => true,
            NEWLINE if one_line => true,
            R_PAREN | R_BRACK | R_BRACE | HOLE_END | FORMAT_SPEC | STRING_END => {
                tok.partner == NONE || (tok.partner as usize) < colon
            }
            _ => {
                !one_line
                    && tok.line_break_before
                    && tok.kind != NEWLINE
                    && indent_width(text, tok.start) <= header_indent
            }
        };
        if ends {
            let stop = if one_line { pos } else { virtual_end(toks, pos) };
            return Some((stop.max(colon + 1), last));
        }
        if tok.kind != NEWLINE {
            last = pos;
        }
        let opens = matches!(tok.kind, L_PAREN | L_BRACK | L_BRACE | STRING_START | HOLE_START);
        if opens && tok.partner != NONE && tok.partner as usize > pos {
            last = tok.partner as usize;
            pos = tok.partner as usize + 1;
        } else {
            pos += 1;
        }
        if pos >= toks.len() {
            return Some((toks.len() - 1, last));
        }
    }
}

/// The fix for a block written with `:`: `{` instead of the `:`, and `}` after the block, on
/// a line of its own when the block spans lines.
pub(crate) fn colon_block_fix(text: &str, toks: &[Tok], colon: usize, last: usize) -> Vec<Edit> {
    let colon_tok = toks[colon];
    let last_tok = toks[last];
    // Remove spaces before the `:` too, as in `def f() :`.
    let before = text[..colon_tok.start as usize].trim_end_matches([' ', '\t']).len() as u32;
    let mut edits =
        vec![Edit { span: Span::new(before, colon_tok.end), replacement: " {".to_string() }];
    let same_line = !text[colon_tok.end as usize..last_tok.end as usize].contains('\n');
    if same_line && last != colon {
        edits.push(Edit { span: Span::at(last_tok.end), replacement: " }".to_string() });
    } else {
        let indent = indentation_at(text, colon_tok.start);
        edits.push(closing_line(text, last_tok.end, indent));
    }
    edits
}
