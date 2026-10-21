//! Step 1 of lexing: cutting the source text into raw tokens (spec/lexical.md, section 1).
//!
//! The scanner reads the text once, from start to end. Its only state besides the position is a
//! stack of modes: plain code, the inside of a string, and the inside of a hole in a string.
//! Strings and holes nest (a hole can hold a string that has holes), and the stack lives on the
//! heap, so deep nesting can't overflow the call stack.
//!
//! The one place where the scanner moves backwards is a hole that is never closed
//! (spec/lexical.md, section 9.3): it drops the tokens it read for the hole and reads the rest
//! of the line again as string text. A budget on the number of bytes read again on each line
//! keeps the total work linear even for input made to defeat this.

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::chars::{
    is_forbidden_control, is_ident_continue, is_ident_start, is_upper_start, look_alike,
    unusual_space, unusual_space_is_visible,
};
use super::codes;
use super::nfc;
use super::values::parse_format_spec;
use crate::SyntaxKind::{self, *};

/// A token before newline marking: its kind and the offset of its first byte. The token runs
/// up to the start of the next token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawToken {
    pub(crate) kind: SyntaxKind,
    pub(crate) start: u32,
}

/// Cuts `text` into raw tokens. The last token is always `EOF`.
pub(crate) fn scan(text: &str) -> (Vec<RawToken>, Vec<Diagnostic>) {
    Scanner::new(text).run()
}

fn span(start: usize, end: usize) -> Span {
    Span::new(start as u32, end as u32)
}

/// How a character is named in a message: in backticks, with its code point if it isn't plain
/// visible ASCII.
fn describe_char(c: char) -> String {
    match c {
        '`' => "the backtick character (U+0060)".to_string(),
        _ if c.is_ascii_graphic() => format!("the character `{c}`"),
        _ => format!("the character `{c}` (U+{:04X})", c as u32),
    }
}

enum Mode {
    Str(StrState),
    Hole(HoleState),
}

/// The state of a string literal being read.
struct StrState {
    /// The offset of the opening delimiter (its first `#`, or its `"`).
    open: usize,
    /// The offset just after the opening `"` or `"""`.
    open_end: usize,
    /// The number of `#` around a raw string; 0 for a normal string.
    hashes: usize,
    multi: bool,
    /// The index of the `STRING_START` token.
    start_token: usize,
    /// For a multi-line string, every content line seen so far.
    lines: Vec<LineRecord>,
    /// For a multi-line string with text right after the opening `"""` (an error), the offset
    /// where that text starts.
    opening_text: Option<usize>,
}

impl StrState {
    fn raw(&self) -> bool {
        self.hashes > 0
    }
}

/// The leading spaces and tabs of one content line of a multi-line string.
struct LineRecord {
    ws_start: usize,
    ws_end: usize,
    /// The line holds only spaces and tabs.
    blank: bool,
}

/// The state of a hole being read.
struct HoleState {
    /// The offset of the hole's `{`.
    brace: usize,
    /// The index of the `HOLE_START` token, and the number of diagnostics before it. If the
    /// hole is never closed, everything after these is dropped.
    start_token: usize,
    diag_len: usize,
    /// The brackets opened inside the hole and not yet closed.
    brackets: Vec<SyntaxKind>,
    /// How many of `brackets` are `(`, `[` and `{`.
    counts: [usize; 3],
    /// The number of tokens directly inside the hole (a nested string counts as one).
    items: u32,
    /// The last of those was a string literal.
    last_was_string: bool,
}

fn bracket_slot(kind: SyntaxKind) -> Option<usize> {
    match kind {
        L_PAREN | R_PAREN => Some(0),
        L_BRACK | R_BRACK => Some(1),
        L_BRACE | R_BRACE => Some(2),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EscapeContext {
    String,
    Char,
}

struct Scanner<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    tokens: Vec<RawToken>,
    diags: Vec<Diagnostic>,
    modes: Vec<Mode>,
    /// How many `Mode::Hole` entries `modes` holds.
    open_holes: usize,
    /// The line (start and end offset) that the rescan budget is for, the bytes on it read
    /// again because of holes that were never closed, and the most allowed.
    rescan_line: (usize, usize),
    rescanned: usize,
    rescan_limit: usize,
    /// A cache for `line_start`: (an offset, the start of its line).
    line_start_cache: (usize, usize),
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Scanner<'a> {
        Scanner {
            text,
            bytes: text.as_bytes(),
            pos: 0,
            tokens: Vec::with_capacity(text.len() / 3 + 2),
            diags: Vec::new(),
            modes: Vec::new(),
            open_holes: 0,
            rescan_line: (0, 0),
            rescanned: 0,
            rescan_limit: 0,
            line_start_cache: (0, 0),
        }
    }

    fn run(mut self) -> (Vec<RawToken>, Vec<Diagnostic>) {
        if self.text.starts_with('\u{FEFF}') {
            self.push(BYTE_ORDER_MARK, 0);
            self.pos = '\u{FEFF}'.len_utf8();
        }
        loop {
            match self.modes.last() {
                None => {
                    if self.pos >= self.bytes.len() {
                        break;
                    }
                    let start = self.pos;
                    if let Some(kind) = self.code_token() {
                        self.push(kind, start);
                    }
                }
                Some(Mode::Hole(_)) => self.hole_token(),
                Some(Mode::Str(_)) => self.string_piece(),
            }
        }
        self.push(EOF, self.bytes.len());
        (self.tokens, self.diags)
    }

    // ------------------------------------------------------------------
    // Small helpers
    // ------------------------------------------------------------------

    fn push(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(RawToken { kind, start: start as u32 });
    }

    /// Starts a `STRING_TEXT` token at `start`, unless the previous token is already string
    /// text that this text simply continues.
    fn push_text(&mut self, start: usize) {
        if self.tokens.last().is_none_or(|token| token.kind != STRING_TEXT) {
            self.push(STRING_TEXT, start);
        }
    }

    fn report(&mut self, diagnostic: Diagnostic) {
        self.diags.push(diagnostic);
    }

    fn is(&self, index: usize, byte: u8) -> bool {
        self.bytes.get(index) == Some(&byte)
    }

    fn starts_with_at(&self, index: usize, prefix: &str) -> bool {
        self.bytes.get(index..).is_some_and(|rest| rest.starts_with(prefix.as_bytes()))
    }

    fn char_at(&self, index: usize) -> Option<char> {
        self.text.get(index..)?.chars().next()
    }

    /// Whether a line break (LF or CR LF) starts at `index`.
    fn is_line_break_at(&self, index: usize) -> bool {
        match self.bytes.get(index) {
            Some(b'\n') => true,
            Some(b'\r') => self.is(index + 1, b'\n'),
            _ => false,
        }
    }

    fn line_break_len(&self, index: usize) -> usize {
        if self.is(index, b'\r') { 2 } else { 1 }
    }

    fn skip_spaces_from(&self, mut index: usize) -> usize {
        while matches!(self.bytes.get(index), Some(b' ' | b'\t')) {
            index += 1;
        }
        index
    }

    /// The offset where the line containing `offset` starts.
    fn line_start(&mut self, offset: usize) -> usize {
        let (cached_at, cached_start) = self.line_start_cache;
        let start = if offset >= cached_at {
            match self.bytes[cached_at..offset].iter().rposition(|&b| b == b'\n') {
                Some(index) => cached_at + index + 1,
                None => cached_start,
            }
        } else {
            self.bytes[..offset].iter().rposition(|&b| b == b'\n').map_or(0, |index| index + 1)
        };
        self.line_start_cache = (offset, start);
        start
    }

    /// The spaces and tabs at the start of the line containing `offset`.
    fn indentation_of_line(&mut self, offset: usize) -> &'a str {
        let start = self.line_start(offset);
        let end = self.skip_spaces_from(start).min(offset.max(start));
        &self.text[start..end]
    }

    /// `word`, with a space added on each side where it would otherwise touch the code next to
    /// the replaced text `start..end`.
    fn spaced_word(&self, start: usize, end: usize, word: &str) -> String {
        let before = start.checked_sub(1).and_then(|index| self.bytes.get(index));
        let after = self.bytes.get(end);
        let space_before = before.is_some_and(|b| !b.is_ascii_whitespace() && !b"([{".contains(b));
        let space_after = after.is_some_and(|b| !b.is_ascii_whitespace() && !b")]},".contains(b));
        let mut text = String::new();
        if space_before {
            text.push(' ');
        }
        text.push_str(word);
        if space_after {
            text.push(' ');
        }
        text
    }

    // ------------------------------------------------------------------
    // Code
    // ------------------------------------------------------------------

    /// Reads one token of code at `self.pos` (not at the end of the text). Returns the kind to
    /// push at the old position, or `None` if the token was pushed already (string openers).
    fn code_token(&mut self) -> Option<SyntaxKind> {
        let start = self.pos;
        let b = *self.bytes.get(start)?;
        let next = self.bytes.get(start + 1).copied();
        let kind = match b {
            b' ' | b'\t' => {
                self.pos = self.skip_spaces_from(start);
                WHITESPACE
            }
            b'\n' => {
                self.pos += 1;
                LINE_BREAK
            }
            b'\r' => {
                self.carriage_return();
                LINE_BREAK
            }
            b'/' => self.slash(),
            b'*' if next == Some(b'/') => {
                self.pos += 2;
                self.report(
                    Diagnostic::error("`*/` without a matching `/*`")
                        .with_code(codes::UNMATCHED_COMMENT_CLOSE)
                        .primary(span(start, start + 2), "no comment is open here")
                        .suggest("remove it", span(start, start + 2), ""),
                );
                ERROR
            }
            b'*' => self.with_eq(STAR_EQ, STAR),
            b'+' => self.with_eq(PLUS_EQ, PLUS),
            b'-' if next == Some(b'>') => self.fixed(2, THIN_ARROW),
            b'-' => self.with_eq(MINUS_EQ, MINUS),
            b'%' => self.with_eq(PERCENT_EQ, PERCENT),
            b'^' => self.with_eq(CARET_EQ, CARET),
            b'~' => self.fixed(1, TILDE),
            b'=' => match next {
                Some(b'=') => self.fixed(2, EQ_EQ),
                Some(b'>') => self.fixed(2, FAT_ARROW),
                _ => self.fixed(1, EQ),
            },
            b'!' if next == Some(b'=') => self.fixed(2, BANG_EQ),
            b'!' => {
                self.pos += 1;
                self.word_operator(start, "!", "not", codes::EXCLAMATION_MARK);
                BANG
            }
            b'<' if next == Some(b'<') => {
                if self.is(start + 2, b'=') {
                    self.fixed(3, LT_LT_EQ)
                } else {
                    self.fixed(2, LT_LT)
                }
            }
            b'<' => self.with_eq(LT_EQ, LT),
            b'>' if next == Some(b'>') => {
                if self.is(start + 2, b'=') {
                    self.fixed(3, GT_GT_EQ)
                } else {
                    self.fixed(2, GT_GT)
                }
            }
            b'>' => self.with_eq(GT_EQ, GT),
            b'&' if next == Some(b'&') => {
                self.pos += 2;
                self.word_operator(start, "&&", "and", codes::DOUBLE_AMPERSAND);
                AMP_AMP
            }
            b'&' => self.with_eq(AMP_EQ, AMP),
            b'|' if next == Some(b'|') => {
                self.pos += 2;
                self.word_operator(start, "||", "or", codes::DOUBLE_BAR);
                PIPE_PIPE
            }
            b'|' => self.with_eq(PIPE_EQ, PIPE),
            b'.' => self.dot(),
            b'?' => match next {
                Some(b'.') => self.fixed(2, QUESTION_DOT),
                Some(b'?') => self.fixed(2, QUESTION_QUESTION),
                _ => self.fixed(1, QUESTION),
            },
            b'@' => self.fixed(1, AT),
            b',' => self.fixed(1, COMMA),
            b':' => self.fixed(1, COLON),
            b'(' => self.fixed(1, L_PAREN),
            b')' => self.fixed(1, R_PAREN),
            b'[' => self.fixed(1, L_BRACK),
            b']' => self.fixed(1, R_BRACK),
            b'{' => self.fixed(1, L_BRACE),
            b'}' => self.fixed(1, R_BRACE),
            b';' => {
                self.pos += 1;
                self.semicolon(start);
                SEMICOLON
            }
            b'"' => {
                self.open_string(start, 0);
                return None;
            }
            b'#' => return self.hash(),
            b'\'' => self.char_literal(),
            b'0'..=b'9' => self.number(),
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => self.ident(),
            0 => {
                self.pos += 1;
                self.nul(start);
                ERROR
            }
            _ if is_forbidden_control(b) => {
                self.pos += 1;
                self.report(
                    Diagnostic::error(format!(
                        "this file contains an invisible control character (U+{b:04X})"
                    ))
                    .with_code(codes::CONTROL_CHARACTER)
                    .primary(span(start, start + 1), "this character")
                    .suggest("remove it", span(start, start + 1), ""),
                );
                ERROR
            }
            0x80.. => self.non_ascii(),
            _ => self.unknown_chars(),
        };
        Some(kind)
    }

    fn fixed(&mut self, len: usize, kind: SyntaxKind) -> SyntaxKind {
        self.pos += len;
        kind
    }

    /// An operator that has a compound assignment form, such as `+` and `+=`.
    fn with_eq(&mut self, with: SyntaxKind, without: SyntaxKind) -> SyntaxKind {
        if self.is(self.pos + 1, b'=') { self.fixed(2, with) } else { self.fixed(1, without) }
    }

    /// A carriage return in code. Alone, it is an error and counts as a line break.
    fn carriage_return(&mut self) {
        let start = self.pos;
        if self.is(start + 1, b'\n') {
            self.pos += 2;
            return;
        }
        self.pos += 1;
        self.lone_carriage_return(start, "use a normal line break", "\n");
    }

    /// Reports a carriage return at `at` that is not followed by a line feed.
    fn lone_carriage_return(&mut self, at: usize, fix: &str, replacement: &str) {
        self.report(
            Diagnostic::error("a carriage return must be followed by a line feed")
                .with_code(codes::LONE_CARRIAGE_RETURN)
                .primary(span(at, at + 1), "a carriage return (U+000D) on its own")
                .note("Quell files end lines with a line feed (LF) or CR LF")
                .suggest(fix, span(at, at + 1), replacement),
        );
    }

    /// Whether a byte order mark (U+FEFF, the bytes EF BB BF) starts at `index`.
    fn is_bom_at(&self, index: usize) -> bool {
        self.starts_with_at(index, "\u{FEFF}")
    }

    /// Reports a byte order mark at `at` that is not at the very start of the file.
    fn stray_byte_order_mark(&mut self, at: usize, in_string: bool) {
        let here = span(at, at + '\u{FEFF}'.len_utf8());
        let diagnostic =
            Diagnostic::error("a byte order mark can only come at the very start of a file")
                .with_code(codes::STRAY_BYTE_ORDER_MARK)
                .primary(here, "an invisible byte order mark (U+FEFF)");
        self.report(if in_string {
            diagnostic.suggest(
                "to put this character in a string, write `\\u{FEFF}`",
                here,
                "\\u{FEFF}",
            )
        } else {
            diagnostic.suggest("remove it", here, "")
        });
    }

    fn nul(&mut self, start: usize) {
        self.report(
            Diagnostic::error("this file contains a NUL character (U+0000)")
                .with_code(codes::NUL_CHARACTER)
                .primary(span(start, start + 1), "NUL character")
                .note("a NUL character is not allowed anywhere, not even in a comment or string")
                .suggest("remove it", span(start, start + 1), ""),
        );
    }

    /// `&&`, `||` and `!`: an error with the fix to write the Quell word.
    fn word_operator(&mut self, start: usize, written: &str, word: &str, code: &'static str) {
        let end = self.pos;
        let example = match word {
            "and" => "a and b",
            "or" => "a or b",
            _ => "not done",
        };
        let replacement = self.spaced_word(start, end, word);
        self.report(
            Diagnostic::error(format!("Quell uses the word `{word}` instead of `{written}`"))
                .with_code(code)
                .primary(span(start, end), format!("write `{word}` here"))
                .suggest(
                    format!("write `{word}`, as in `{example}`"),
                    span(start, end),
                    replacement,
                ),
        );
    }

    fn dot(&mut self) -> SyntaxKind {
        let start = self.pos;
        if !self.is(start + 1, b'.') {
            return self.fixed(1, DOT);
        }
        let (len, kind) = match self.bytes.get(start + 2) {
            Some(b'<') => return self.fixed(3, DOT_DOT_LT),
            Some(b'=') => return self.fixed(3, DOT_DOT_EQ),
            Some(b'.') => (3, DOT_DOT_DOT),
            _ => (2, DOT_DOT),
        };
        self.pos += len;
        let here = span(start, start + len);
        self.report(
            Diagnostic::error("a range must say whether it includes the end")
                .with_code(codes::BARE_RANGE)
                .primary(here, "write `..<` or `..=` here")
                .suggest(
                    "to stop before the end, write `..<` (`0..<10` stops before 10)",
                    here,
                    "..<",
                )
                .suggest("to include the end, write `..=` (`1..=10` includes 10)", here, "..="),
        );
        kind
    }

    fn semicolon(&mut self, start: usize) {
        let after = self.skip_spaces_from(start + 1);
        let at_line_end = after >= self.bytes.len()
            || matches!(self.bytes[after], b'\n' | b'\r')
            || self.starts_with_at(after, "//");
        let diagnostic = Diagnostic::error("Quell doesn't use semicolons")
            .with_code(codes::SEMICOLON)
            .primary(span(start, start + 1), "remove this `;`");
        let diagnostic = if at_line_end {
            let mut from = start;
            while from > 0 && matches!(self.bytes[from - 1], b' ' | b'\t') {
                from -= 1;
            }
            let from = if from > 0 && self.bytes[from - 1] != b'\n' { from } else { start };
            diagnostic.suggest(
                "a line break ends a statement, so remove the `;`",
                span(from, start + 1),
                "",
            )
        } else {
            let indent = self.indentation_of_line(start).to_string();
            let mut from = start;
            while from > 0 && matches!(self.bytes[from - 1], b' ' | b'\t') {
                from -= 1;
            }
            diagnostic.suggest(
                "put each statement on its own line",
                span(from, after),
                format!("\n{indent}"),
            )
        };
        self.report(diagnostic);
    }

    /// `#`: the start of a raw string, or an error.
    fn hash(&mut self) -> Option<SyntaxKind> {
        let start = self.pos;
        let mut after = start;
        while self.is(after, b'#') {
            after += 1;
        }
        if self.is(after, b'"') {
            self.open_string(start, after - start);
            return None;
        }
        let here = span(start, after);
        let diagnostic = Diagnostic::error("`#` is only used for raw strings")
            .with_code(codes::STRAY_HASH)
            .primary(here, "this `#` doesn't start a raw string");
        // `# text` or `#!` is a comment in Python and shell scripts. Outside a string, the rest
        // of the line is skipped with the `#`, so its words are not read as code.
        let comment = after - start == 1
            && self.open_holes == 0
            && matches!(self.bytes.get(after), None | Some(b' ' | b'\t' | b'!' | b'\n' | b'\r'));
        if comment {
            self.pos = self.line_comment_end(start);
            self.report(
                diagnostic
                    .note("comments in Quell start with `//`")
                    .suggest("to write a comment, use `//`", here, "//"),
            );
            return Some(ERROR);
        }
        self.pos = after;
        self.report(
            diagnostic
                .note("a raw string looks like `#\"{\"a\": 1}\"#`")
                .suggest("remove it", here, ""),
        );
        Some(ERROR)
    }

    fn non_ascii(&mut self) -> SyntaxKind {
        let start = self.pos;
        let Some(c) = self.char_at(start) else {
            // Not reachable for a `&str`, but never panic or loop.
            self.pos += 1;
            return ERROR;
        };
        if is_ident_start(c) {
            return self.ident();
        }
        let end = start + c.len_utf8();
        if c == '\u{FEFF}' {
            self.pos = end;
            self.stray_byte_order_mark(start, false);
            return ERROR;
        }
        if let Some(name) = unusual_space(c) {
            self.pos = end;
            let here = span(start, end);
            let diagnostic = Diagnostic::error(format!(
                "this is {name} (U+{:04X}), not a normal space",
                c as u32
            ))
            .with_code(codes::UNUSUAL_SPACE)
            .primary(here, "this character");
            let diagnostic = match c {
                '\u{85}' | '\u{2028}' | '\u{2029}' => {
                    diagnostic.suggest("use a normal line break", here, "\n")
                }
                _ if unusual_space_is_visible(c) => {
                    diagnostic.suggest("use a normal space", here, " ")
                }
                _ => diagnostic.suggest("remove it", here, ""),
            };
            self.report(diagnostic);
            return WHITESPACE;
        }
        if let Some((name, ascii)) = look_alike(c) {
            self.pos = end;
            let here = span(start, end);
            self.report(
                Diagnostic::error(format!(
                    "this is {name} (U+{:04X}), which Quell doesn't use",
                    c as u32
                ))
                .with_code(codes::LOOK_ALIKE_CHARACTER)
                .primary(here, format!("this looks like `{ascii}` but is a different character"))
                .suggest(format!("write `{ascii}`"), here, ascii),
            );
            return ERROR;
        }
        self.unknown_chars()
    }

    /// A run of characters that can't start any token.
    fn unknown_chars(&mut self) -> SyntaxKind {
        let start = self.pos;
        let Some(first) = self.char_at(start) else {
            self.pos += 1;
            return ERROR;
        };
        let unknown = |c: char| {
            matches!(c, '$' | '`' | '\\')
                || (!c.is_ascii()
                    && !is_ident_start(c)
                    && c != '\u{FEFF}'
                    && unusual_space(c).is_none()
                    && look_alike(c).is_none())
        };
        self.pos += first.len_utf8();
        while let Some(c) = self.char_at(self.pos) {
            if c != first || !unknown(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        let here = span(start, self.pos);
        let mut diagnostic =
            Diagnostic::error(format!("Quell doesn't use {}", describe_char(first)))
                .with_code(codes::UNKNOWN_CHARACTER)
                .primary(here, "this character can't start any part of Quell code");
        diagnostic =
            match first {
                '\\' => diagnostic
                    .note("a backslash is only used for escapes inside strings, like `\\n`"),
                '`' => diagnostic.note("strings use double quotes: `\"text\"`"),
                '$' => diagnostic
                    .note("to put a value into a string, write `{name}` inside the string"),
                _ => diagnostic,
            };
        self.report(diagnostic.suggest("remove it", here, ""));
        ERROR
    }

    // ------------------------------------------------------------------
    // Comments
    // ------------------------------------------------------------------

    fn slash(&mut self) -> SyntaxKind {
        let start = self.pos;
        match self.bytes.get(start + 1) {
            Some(b'/') => {
                let doc = self.is(start + 2, b'/') && !self.is(start + 3, b'/');
                let end = self.line_comment_end(start);
                self.pos = end;
                if doc { DOC_COMMENT } else { LINE_COMMENT }
            }
            Some(b'*') => self.block_comment(),
            Some(b'=') => self.fixed(2, SLASH_EQ),
            _ => self.fixed(1, SLASH),
        }
    }

    /// The end of a line comment that starts at `start`: the line break, which is not part of
    /// the comment. A carriage return on its own also ends it (`code_token` reports it and reads
    /// it as a line break). Reports NUL characters and byte order marks inside the comment.
    fn line_comment_end(&mut self, start: usize) -> usize {
        let mut index = start;
        while let Some(&b) = self.bytes.get(index) {
            match b {
                b'\n' | b'\r' => break,
                0 => self.nul(index),
                0xEF if self.is_bom_at(index) => self.stray_byte_order_mark(index, false),
                _ => {}
            }
            index += 1;
        }
        index
    }

    fn block_comment(&mut self) -> SyntaxKind {
        let start = self.pos;
        let mut open = vec![start];
        let mut index = start + 2;
        while let Some(&b) = self.bytes.get(index) {
            if b == b'*' && self.is(index + 1, b'/') {
                open.pop();
                index += 2;
                if open.is_empty() {
                    self.pos = index;
                    return BLOCK_COMMENT;
                }
            } else if b == b'/' && self.is(index + 1, b'*') {
                open.push(index);
                index += 2;
            } else {
                match b {
                    0 => self.nul(index),
                    b'\r' if !self.is(index + 1, b'\n') => {
                        self.lone_carriage_return(index, "use a normal line break", "\n");
                    }
                    0xEF if self.is_bom_at(index) => self.stray_byte_order_mark(index, false),
                    _ => {}
                }
                index += 1;
            }
        }
        self.pos = index;
        let mut diagnostic = Diagnostic::error("this block comment is never closed")
            .with_code(codes::UNCLOSED_BLOCK_COMMENT)
            .primary(span(start, start + 2), "this `/*` has no matching `*/`");
        // Show a few of the nested comments that are still open, not thousands.
        for &nested in open[1..].iter().rev().take(3).rev() {
            diagnostic = diagnostic
                .secondary(span(nested, nested + 2), "this nested `/*` also needs its own `*/`");
        }
        if open.len() > 1 {
            diagnostic = diagnostic.note("block comments nest: every `/*` needs its own `*/`");
        }
        self.report(diagnostic.suggest(
            "close the comment with `*/`",
            span(index, index),
            "*/".repeat(open.len()),
        ));
        BLOCK_COMMENT
    }

    // ------------------------------------------------------------------
    // Names
    // ------------------------------------------------------------------

    fn ident(&mut self) -> SyntaxKind {
        let start = self.pos;
        let Some(first) = self.char_at(start) else {
            self.pos += 1;
            return ERROR;
        };
        self.pos += first.len_utf8();
        let mut ascii = first.is_ascii();
        while let Some(&b) = self.bytes.get(self.pos) {
            if b < 0x80 {
                if b.is_ascii_alphanumeric() || b == b'_' {
                    self.pos += 1;
                    continue;
                }
                break;
            }
            match self.char_at(self.pos) {
                Some(c) if is_ident_continue(c) => {
                    ascii = false;
                    self.pos += c.len_utf8();
                }
                _ => break,
            }
        }
        let text = &self.text[start..self.pos];
        if text == "_" {
            return UNDERSCORE;
        }
        if ascii {
            if let Some(keyword) = SyntaxKind::from_keyword(text) {
                return keyword;
            }
        } else if let Some(normalized) = nfc::nfc_if_different(text) {
            let here = span(start, self.pos);
            self.report(
                Diagnostic::error(format!("the name `{text}` is not in the standard Unicode form"))
                    .with_code(codes::NOT_NFC)
                    .primary(here, "this name mixes separate accents with letters")
                    .note("names must be in Unicode Normalization Form C (NFC), so that two names that look the same are the same")
                    .suggest(format!("write it as `{normalized}`"), here, normalized),
            );
        }
        if is_upper_start(first) { UPPER_IDENT } else { LOWER_IDENT }
    }

    // ------------------------------------------------------------------
    // Numbers
    // ------------------------------------------------------------------

    fn number(&mut self) -> SyntaxKind {
        let start = self.pos;
        if self.bytes[start] == b'0' {
            if let Some(&prefix @ (b'x' | b'b' | b'o' | b'X' | b'B' | b'O')) =
                self.bytes.get(start + 1)
            {
                return self.prefixed_number(start, prefix);
            }
        }
        self.decimal_number(start)
    }

    /// Advances over digits and underscores; with `hex`, also over hex letters.
    fn skip_digits(&mut self, hex: bool) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_digit() || b == b'_' || (hex && b.is_ascii_hexdigit()) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn prefixed_number(&mut self, start: usize, prefix: u8) -> SyntaxKind {
        let lower = prefix.to_ascii_lowercase();
        let (base, name, digit_names) = match lower {
            b'x' => (16, "hex", "the digits 0 to 9 and the letters a to f"),
            b'b' => (2, "binary", "only the digits 0 and 1"),
            _ => (8, "octal", "only the digits 0 to 7"),
        };
        let article = if base == 8 { "an" } else { "a" };
        if prefix != lower {
            let here = span(start + 1, start + 2);
            let fixed = (lower as char).to_string();
            self.report(
                Diagnostic::error(format!("number prefixes are lowercase: write `0{fixed}`"))
                    .with_code(codes::UPPERCASE_PREFIX)
                    .primary(here, "uppercase prefix")
                    .suggest(format!("write `0{fixed}`"), here, fixed),
            );
        }
        self.pos = start + 2;
        let digits_start = self.pos;
        self.skip_digits(base == 16);
        let digits_end = self.pos;
        let digits = &self.text[digits_start..digits_end];
        let has_suffix = self.char_at(self.pos).is_some_and(is_ident_continue);

        if !digits.bytes().any(|b| b != b'_') {
            self.pos = self.skip_suffix(self.pos);
            let prefix_text = &self.text[start..start + 2];
            self.report(
                Diagnostic::error(format!("`{prefix_text}` must be followed by {name} digits"))
                    .with_code(codes::PREFIX_WITHOUT_DIGITS)
                    .primary(span(start, self.pos), format!("no {name} digits here"))
                    .help(format!(
                        "{article} {name} number uses {digit_names}, as in `0{}{}`",
                        lower as char,
                        match base {
                            16 => "ff",
                            2 => "1010",
                            _ => "17",
                        }
                    )),
            );
            return INT;
        }

        if let Some(bad) = digits.bytes().position(|b| b.is_ascii_digit() && b - b'0' >= base) {
            let at = digits_start + bad;
            let digit = self.bytes[at] as char;
            let mut diagnostic =
                Diagnostic::error(format!("`{digit}` is not {article} {name} digit"))
                    .with_code(codes::DIGIT_OUTSIDE_BASE)
                    .primary(
                        span(at, at + 1),
                        format!("{article} {name} number uses {digit_names}"),
                    );
            let decimal = !digits.starts_with(['0', '_'])
                && digits.bytes().all(|b| b.is_ascii_digit() || b == b'_')
                && !has_suffix;
            if decimal {
                diagnostic = diagnostic.suggest(
                    format!("for the decimal number {digits}, remove the `0{}`", lower as char),
                    span(start, start + 2),
                    "",
                );
            } else {
                diagnostic =
                    diagnostic.help("change the digit, or write the number in another base");
            }
            self.report(diagnostic);
        }
        self.check_underscores(digits_start, digits_end);
        self.number_suffix();
        INT
    }

    fn decimal_number(&mut self, start: usize) -> SyntaxKind {
        let mut kind = INT;
        self.skip_digits(false);
        let int_end = self.pos;
        // The dot rule (spec/lexical.md, section 7.3).
        let mut fraction = None;
        if self.is(int_end, b'.') && self.bytes.get(int_end + 1).is_some_and(u8::is_ascii_digit) {
            self.pos = int_end + 1;
            self.skip_digits(false);
            fraction = Some((int_end + 1, self.pos));
            kind = FLOAT;
        }
        // An exponent needs a digit after `e`, or after `e+` or `e-`.
        let mut exponent = None;
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            let sign = matches!(self.bytes.get(self.pos + 1), Some(b'+' | b'-'));
            let digit_at = self.pos + 1 + usize::from(sign);
            if self.bytes.get(digit_at).is_some_and(u8::is_ascii_digit) {
                self.pos = digit_at;
                self.skip_digits(false);
                exponent = Some((digit_at, self.pos));
                kind = FLOAT;
            }
        }

        let int_part = &self.text[start..int_end];
        if int_part.starts_with('0') && int_part[1..].bytes().any(|b| b.is_ascii_digit()) {
            let kept = int_part.trim_start_matches(['0', '_']);
            let kept = if kept.is_empty() { "0" } else { kept };
            let here = span(start, int_end);
            let mut diagnostic = Diagnostic::error("numbers can't start with `0`")
                .with_code(codes::LEADING_ZERO)
                .primary(here, "leading zero")
                .note("a leading zero would mean an octal number in some languages, so Quell doesn't allow it")
                .suggest(format!("for the number {kept}, remove the leading zeros"), here, kept);
            if kind == INT && kept.bytes().all(|b| (b'0'..=b'7').contains(&b) || b == b'_') {
                diagnostic = diagnostic.suggest(
                    format!("for an octal number, write `0o{kept}`"),
                    here,
                    format!("0o{kept}"),
                );
            }
            self.report(diagnostic);
        }
        self.check_underscores(start, int_end);
        if let Some((from, to)) = fraction {
            self.check_underscores(from, to);
        }
        if let Some((from, to)) = exponent {
            self.check_underscores(from, to);
        }
        self.number_suffix();
        kind
    }

    /// Each underscore in the digits `from..to` must sit between two digits.
    fn check_underscores(&mut self, from: usize, to: usize) {
        let mut edits = Vec::new();
        let mut index = from;
        while index < to {
            if self.bytes[index] != b'_' {
                index += 1;
                continue;
            }
            let run_start = index;
            while index < to && self.bytes[index] == b'_' {
                index += 1;
            }
            let at_edge = run_start == from || index == to;
            let remove_from = if at_edge { run_start } else { run_start + 1 };
            if remove_from < index {
                edits.push(Edit { span: span(remove_from, index), replacement: String::new() });
            }
        }
        let Some(first) = edits.first() else { return };
        let here = first.span;
        self.report(
            Diagnostic::error("an underscore in a number must sit between two digits")
                .with_code(codes::MISPLACED_UNDERSCORE)
                .primary(here, "misplaced underscore")
                .note("underscores make long numbers readable, as in `1_000_000`")
                .suggest_edits("remove it", edits),
        );
    }

    fn skip_suffix(&self, mut index: usize) -> usize {
        while let Some(c) = self.char_at(index) {
            if !is_ident_continue(c) {
                break;
            }
            index += c.len_utf8();
        }
        index
    }

    /// Letters right after a number (spec/lexical.md, section 7.5). They become part of the
    /// number token, so the parser sees one number.
    fn number_suffix(&mut self) {
        let start = self.pos;
        let end = self.skip_suffix(start);
        if end == start {
            return;
        }
        self.pos = end;
        let suffix = &self.text[start..end];
        let here = span(start, end);
        self.report(
            Diagnostic::error("a number can't be directly followed by a letter")
                .with_code(codes::NUMBER_SUFFIX)
                .primary(here, format!("`{suffix}` is not part of the number"))
                .note("Quell has no number suffixes; the type comes from the context or a conversion such as `UInt8(255)`")
                .suggest(format!("remove `{suffix}`"), here, ""),
        );
    }

    // ------------------------------------------------------------------
    // Character literals and escapes
    // ------------------------------------------------------------------

    fn char_literal(&mut self) -> SyntaxKind {
        let start = self.pos;
        self.pos += 1;
        match self.bytes.get(self.pos) {
            None | Some(b'\n' | b'\r') => {
                self.report(
                    Diagnostic::error("this character literal is never closed")
                        .with_code(codes::UNCLOSED_CHAR)
                        .primary(span(start, start + 1), "the line ends before the closing `'`")
                        .help("a character literal is one character between single quotes, such as `'a'`"),
                );
                return CHAR;
            }
            Some(b'\'') => {
                self.pos += 1;
                self.report(
                    Diagnostic::error("a character literal needs exactly one character")
                        .with_code(codes::EMPTY_CHAR)
                        .primary(span(start, self.pos), "nothing between the quotes")
                        .help("put one character between the quotes, such as `'a'` or `' '`")
                        .suggest("for empty text, use a string", span(start, self.pos), "\"\""),
                );
                return CHAR;
            }
            _ => {}
        }
        if self.is(self.pos, b'\\') {
            self.escape(EscapeContext::Char);
        } else if let Some(c) = self.char_at(self.pos) {
            self.check_control_in_literal(self.pos, false);
            self.pos += c.len_utf8();
        }
        if self.is(self.pos, b'\'') {
            self.pos += 1;
            return CHAR;
        }

        let mut close = self.pos;
        while let Some(&b) = self.bytes.get(close) {
            if matches!(b, b'\'' | b'\n' | b'\r') {
                break;
            }
            close += 1;
        }
        if !self.is(close, b'\'') {
            let at = self.pos;
            self.report(
                Diagnostic::error("this character literal is never closed")
                    .with_code(codes::UNCLOSED_CHAR)
                    .primary(span(start, at), "this character has no closing `'`")
                    .suggest("add the closing `'`", span(at, at), "'"),
            );
            return CHAR;
        }

        self.pos = close + 1;
        let content = &self.text[start + 1..close];
        let here = span(start, self.pos);
        let diagnostic = Diagnostic::error("single quotes hold exactly one character")
            .with_code(codes::MULTI_CHAR)
            .primary(here, "this has more than one character");
        let composed = (!content.contains('\\')).then(|| nfc::nfc(content));
        let diagnostic = match composed {
            Some(composed) if composed.chars().count() == 1 => diagnostic
                .note("this is a letter followed by separate accents, which together look like one character")
                .suggest(format!("write it as the single character `{composed}`"), span(start + 1, close), composed),
            _ => {
                // Keep escapes as they are; double the braces that a string would read as a hole.
                let mut text = String::with_capacity(content.len() + 2);
                text.push('"');
                let mut chars = content.chars().peekable();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            text.push(c);
                            let Some(escaped) = chars.next() else { break };
                            text.push(escaped);
                            if escaped == 'u' && chars.peek() == Some(&'{') {
                                // `\u{...}` runs to its `}`.
                                for d in chars.by_ref() {
                                    text.push(d);
                                    if d == '}' {
                                        break;
                                    }
                                }
                            }
                        }
                        '"' => text.push_str("\\\""),
                        '{' => text.push_str("{{"),
                        '}' => text.push_str("}}"),
                        _ => text.push(c),
                    }
                }
                text.push('"');
                diagnostic.suggest("for text, use double quotes", here, text)
            }
        };
        self.report(diagnostic);
        CHAR
    }

    /// Reports a NUL, a control character, a carriage return on its own or a byte order mark
    /// at `at` inside a string or character literal.
    fn check_control_in_literal(&mut self, at: usize, raw: bool) {
        let Some(&b) = self.bytes.get(at) else { return };
        if b == 0 {
            self.nul(at);
            return;
        }
        if b == 0xEF && self.is_bom_at(at) {
            self.stray_byte_order_mark(at, !raw);
            return;
        }
        if b == b'\r' {
            if raw {
                self.lone_carriage_return(at, "remove it", "");
            } else {
                self.lone_carriage_return(at, "write it as the escape `\\r`", "\\r");
            }
            return;
        }
        if !is_forbidden_control(b) {
            return;
        }
        let here = span(at, at + 1);
        let escape = format!("\\u{{{b:X}}}");
        let diagnostic = Diagnostic::error(format!(
            "an invisible control character (U+{b:04X}) can't be written directly in a literal"
        ))
        .with_code(codes::CONTROL_IN_STRING)
        .primary(here, "this character");
        self.report(if raw {
            diagnostic
                .help(format!("raw strings have no escapes; use a normal string with `{escape}`"))
                .suggest("or remove it", here, "")
        } else {
            diagnostic.suggest(format!("write it as the escape `{escape}`"), here, escape)
        });
    }

    /// Reads one escape starting at the `\` at `self.pos` and reports it if it is invalid.
    fn escape(&mut self, context: EscapeContext) {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.char_at(self.pos) else {
            self.report(
                Diagnostic::error("this `\\` starts an escape, but the file ends here")
                    .with_code(codes::UNKNOWN_ESCAPE)
                    .primary(span(start, start + 1), "incomplete escape")
                    .suggest("for a backslash, write `\\\\`", span(start, start + 1), "\\\\"),
            );
            return;
        };
        if c == '\n' || c == '\r' {
            self.report(
                Diagnostic::error("this `\\` starts an escape, but the line ends here")
                    .with_code(codes::UNKNOWN_ESCAPE)
                    .primary(span(start, start + 1), "incomplete escape")
                    .suggest("for a backslash, write `\\\\`", span(start, start + 1), "\\\\"),
            );
            return;
        }
        self.pos += c.len_utf8();
        let here = span(start, self.pos);
        match c {
            'n' | 'r' | 't' | '0' | '\\' | '"' | '\'' => {}
            'u' => self.unicode_escape(start),
            '{' | '}' => {
                let doubled = if c == '{' { "{{" } else { "}}" };
                let diagnostic = Diagnostic::error(format!("there is no `\\{c}` escape"))
                    .with_code(codes::BRACE_ESCAPE)
                    .primary(here, "not an escape");
                self.report(match context {
                    EscapeContext::String => diagnostic.suggest(
                        format!("in a string, a literal brace is written twice: `{doubled}`"),
                        here,
                        doubled,
                    ),
                    EscapeContext::Char => diagnostic.suggest(
                        format!("a brace needs no escape in a character literal: `'{c}'`"),
                        here,
                        c.to_string(),
                    ),
                });
            }
            _ => {
                self.report(
                    Diagnostic::error(format!("`\\{c}` is not an escape"))
                        .with_code(codes::UNKNOWN_ESCAPE)
                        .primary(here, "unknown escape")
                        .note("the escapes are `\\n`, `\\r`, `\\t`, `\\0`, `\\\\`, `\\\"`, `\\'` and `\\u{...}`")
                        .suggest(
                            "for a backslash, write it twice",
                            span(start, start + 1),
                            "\\\\",
                        ),
                );
            }
        }
    }

    /// `\u{...}`. `start` is the offset of the `\`; `self.pos` is just after the `u`.
    fn unicode_escape(&mut self, start: usize) {
        if !self.is(self.pos, b'{') {
            self.report(
                Diagnostic::error("`\\u` must be followed by a hex code in braces")
                    .with_code(codes::MALFORMED_UNICODE_ESCAPE)
                    .primary(span(start, self.pos), "incomplete escape")
                    .help("write the code of the character in hex, such as `\\u{E9}` for `\u{E9}`"),
            );
            return;
        }
        self.pos += 1;
        let digits_start = self.pos;
        while self.bytes.get(self.pos).is_some_and(u8::is_ascii_alphanumeric) {
            self.pos += 1;
        }
        let digits_end = self.pos;
        let closed = self.is(self.pos, b'}');
        if closed {
            self.pos += 1;
        }
        let digits = &self.text[digits_start..digits_end];
        let here = span(start, self.pos);
        let all_hex = digits.bytes().all(|b| b.is_ascii_hexdigit());
        let malformed = |message: &str| {
            Diagnostic::error(message.to_string())
                .with_code(codes::MALFORMED_UNICODE_ESCAPE)
                .primary(here, "invalid escape")
        };
        if !closed {
            let diagnostic = malformed("this `\\u{` escape is missing its closing `}`");
            self.report(if all_hex && (1..=6).contains(&digits.len()) {
                diagnostic.suggest("add the `}`", span(digits_end, digits_end), "}")
            } else {
                diagnostic.help("write the code in hex between braces, such as `\\u{E9}`")
            });
            return;
        }
        if digits.is_empty() || !all_hex {
            self.report(
                malformed("`\\u{...}` needs a hex number of 1 to 6 digits").help(
                    "write the code of the character in hex, such as `\\u{E9}` or `\\u{1F600}`",
                ),
            );
            return;
        }
        if digits.len() > 6 {
            let trimmed = digits.trim_start_matches('0');
            let diagnostic = malformed("`\\u{...}` takes at most 6 hex digits");
            self.report(if !trimmed.is_empty() && trimmed.len() <= 6 {
                diagnostic.suggest(
                    "remove the leading zeros",
                    span(digits_start, digits_end),
                    trimmed.to_string(),
                )
            } else {
                diagnostic.note("the largest character code is `10FFFF`").help(
                    "write the code of the character in hex, such as `\\u{E9}` or `\\u{1F600}`",
                )
            });
            return;
        }
        let value = u32::from_str_radix(digits, 16).unwrap_or(u32::MAX);
        if char::from_u32(value).is_none() {
            let reason = if (0xD800..=0xDFFF).contains(&value) {
                "the codes from `D800` to `DFFF` are surrogates, which are not characters"
            } else {
                "the largest character code is `10FFFF`"
            };
            self.report(
                Diagnostic::error(format!("`\\u{{{digits}}}` is not a Unicode character"))
                    .with_code(codes::INVALID_UNICODE_SCALAR)
                    .primary(here, "no character has this code")
                    .note(reason)
                    .help("check the code of the character you want"),
            );
        }
    }

    // ------------------------------------------------------------------
    // Strings
    // ------------------------------------------------------------------

    /// Opens a string literal whose delimiter starts at `start` with `hashes` `#` signs.
    /// Pushes `STRING_START` (and, for a multi-line string, the first pieces after it).
    fn open_string(&mut self, start: usize, hashes: usize) {
        let quote = start + hashes;
        let triple = self.starts_with_at(quote, "\"\"\"");
        let after_triple = quote + 3;
        let ws_end = self.skip_spaces_from(after_triple);
        let rest_empty = triple && (ws_end >= self.bytes.len() || self.is_line_break_at(ws_end));
        // A raw string is multi-line only if nothing follows `#"""` (spec 9.6); a normal string
        // with `"""` is always multi-line, and text after it is an error (spec 9.5, rule 1).
        let multi = if hashes > 0 { rest_empty } else { triple };
        let open_end = if multi { after_triple } else { quote + 1 };
        self.push(STRING_START, start);
        let mut state = StrState {
            open: start,
            open_end,
            hashes,
            multi,
            start_token: self.tokens.len() - 1,
            lines: Vec::new(),
            opening_text: None,
        };
        if !multi {
            self.pos = quote + 1;
            self.modes.push(Mode::Str(state));
        } else if rest_empty && ws_end < self.bytes.len() {
            self.pos = ws_end + self.line_break_len(ws_end);
            self.begin_line(state, self.pos);
        } else if rest_empty {
            self.pos = ws_end;
            self.modes.push(Mode::Str(state));
        } else {
            state.opening_text = Some(after_triple);
            self.pos = after_triple;
            self.modes.push(Mode::Str(state));
        }
    }

    /// If the closing delimiter of `s` starts at `index`, the offset just after it.
    fn closer_at(&self, s: &StrState, index: usize) -> Option<usize> {
        let quotes = if s.multi { 3 } else { 1 };
        let end = index + quotes + s.hashes;
        let bytes = self.bytes.get(index..end)?;
        let ok = bytes[..quotes].iter().all(|&b| b == b'"')
            && bytes[quotes..].iter().all(|&b| b == b'#');
        ok.then_some(end)
    }

    /// The text of a closing delimiter, for fixes.
    fn closer_text(s: &StrState) -> String {
        let quotes = if s.multi { "\"\"\"" } else { "\"" };
        format!("{quotes}{}", "#".repeat(s.hashes))
    }

    /// Reads one piece of the string on top of the mode stack.
    fn string_piece(&mut self) {
        let Some(Mode::Str(s)) = self.modes.pop() else { return };
        let pos = self.pos;
        let len = self.bytes.len();

        if pos >= len || self.is_line_break_at(pos) {
            if self.open_holes > 0 {
                // A hole can't contain a line break, so the innermost hole is never closed.
                self.fail_innermost_hole();
                return;
            }
            if pos < len && s.multi {
                self.pos = pos + self.line_break_len(pos);
                self.begin_line(s, pos);
                return;
            }
            self.unclosed_string(s);
            return;
        }

        if let Some(end) = self.closer_at(&s, pos) {
            if s.multi {
                let prefix = s.lines.last().map_or((pos, pos), |line| (line.ws_start, line.ws_end));
                self.finish_multiline(s, pos, end, prefix, true);
            } else {
                self.push(STRING_END, pos);
                self.pos = end;
            }
            return;
        }

        if !s.raw() {
            match self.bytes[pos] {
                b'\\' => {
                    self.escape(EscapeContext::String);
                    self.push(STRING_ESCAPE, pos);
                    self.modes.push(Mode::Str(s));
                    return;
                }
                b'{' if self.is(pos + 1, b'{') => {
                    self.push(STRING_ESCAPE, pos);
                    self.pos += 2;
                    self.modes.push(Mode::Str(s));
                    return;
                }
                b'{' => {
                    self.open_hole(s);
                    return;
                }
                b'}' if self.is(pos + 1, b'}') => {
                    self.push(STRING_ESCAPE, pos);
                    self.pos += 2;
                    self.modes.push(Mode::Str(s));
                    return;
                }
                b'}' => {
                    let here = span(pos, pos + 1);
                    self.report(
                        Diagnostic::error("a single `}` in a string doesn't close anything")
                            .with_code(codes::LONE_CLOSING_BRACE)
                            .primary(here, "no `{` is open here")
                            .suggest("for a literal brace, write `}}`", here, "}}"),
                    );
                    self.push_text(pos);
                    self.pos += 1;
                    self.modes.push(Mode::Str(s));
                    return;
                }
                _ => {}
            }
        }
        self.string_text(&s);
        self.modes.push(Mode::Str(s));
    }

    /// Plain text inside a string, up to the next piece that needs attention.
    fn string_text(&mut self, s: &StrState) {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b'\n' => break,
                b'\r' if self.is(self.pos + 1, b'\n') => break,
                b'"' if self.closer_at(s, self.pos).is_some() => break,
                b'\\' | b'{' | b'}' if !s.raw() => break,
                0 | b'\r' => {
                    self.check_control_in_literal(self.pos, s.raw());
                    self.pos += 1;
                }
                _ if is_forbidden_control(b) => {
                    self.check_control_in_literal(self.pos, s.raw());
                    self.pos += 1;
                }
                0xEF if self.is_bom_at(self.pos) => {
                    self.check_control_in_literal(self.pos, s.raw());
                    self.pos += '\u{FEFF}'.len_utf8();
                }
                // Every other byte that needs attention is ASCII, and the bytes of a multi-byte
                // character never are, so stepping byte by byte never splits a character.
                _ => self.pos += 1,
            }
        }
        if self.pos > start {
            self.push_text(start);
        }
    }

    /// The start of a content line of a multi-line string (or its closing line). `self.pos` is
    /// at the first byte of the line; `token_start` is where the line break before it starts.
    fn begin_line(&mut self, mut s: StrState, token_start: usize) {
        let ws_start = self.pos;
        let ws_end = self.skip_spaces_from(ws_start);
        if let Some(end) = self.closer_at(&s, ws_end) {
            self.finish_multiline(s, token_start, end, (ws_start, ws_end), false);
            return;
        }
        if token_start < ws_start {
            self.push(STRING_LINE_BREAK, token_start);
        }
        if ws_end > ws_start {
            self.push(STRING_INDENT, ws_start);
        }
        let blank = ws_end >= self.bytes.len() || self.is_line_break_at(ws_end);
        s.lines.push(LineRecord { ws_start, ws_end, blank });
        self.pos = ws_end;
        self.modes.push(Mode::Str(s));
    }

    /// Ends a multi-line string. The closing delimiter (with the line break and indentation
    /// before it, if it is on its own line) is `end_start..end_end`; `prefix` is the
    /// indentation that every content line loses (spec/lexical.md, section 9.5).
    fn finish_multiline(
        &mut self,
        s: StrState,
        end_start: usize,
        end_end: usize,
        prefix: (usize, usize),
        mid_line: bool,
    ) {
        let prefix_text = &self.text[prefix.0..prefix.1];
        let plen = prefix_text.len();

        // Now that the indentation is known, turn each line's leading spaces into the part
        // that is removed (STRING_INDENT) and the part that is kept (STRING_TEXT).
        let tail = self.tokens.split_off(s.start_token + 1);
        let mut out = Vec::with_capacity(tail.len() + s.lines.len());
        let mut lines = s.lines.iter().peekable();
        let mut index = 0;
        while let Some(&token) = tail.get(index) {
            index += 1;
            if token.kind != STRING_INDENT {
                out.push(token);
                continue;
            }
            while lines.peek().is_some_and(|line| (line.ws_start as u32) < token.start) {
                lines.next();
            }
            let Some(line) = lines.peek().filter(|line| line.ws_start as u32 == token.start) else {
                out.push(token);
                continue;
            };
            let ws = &self.text[line.ws_start..line.ws_end];
            if !ws.starts_with(prefix_text) {
                out.push(token);
                continue;
            }
            let kept_from = line.ws_start + plen;
            if plen > 0 {
                out.push(token);
            }
            if kept_from < line.ws_end {
                out.push(RawToken { kind: STRING_TEXT, start: kept_from as u32 });
                if tail.get(index).is_some_and(|next| next.kind == STRING_TEXT) {
                    index += 1;
                }
            }
        }
        self.tokens.extend(out);
        self.push(STRING_END, end_start);
        self.pos = end_end;

        let closer = Self::closer_text(&s);
        let closer_span = span(end_end - closer.len(), end_end);

        if let Some(text_start) = s.opening_text {
            if mid_line && s.lines.is_empty() {
                // `"""text"""` on one line.
                let content = &self.text[text_start..end_end - closer.len()];
                let mut diagnostic = Diagnostic::error(
                    "a string that starts with `\"\"\"` must start its text on the next line",
                )
                .with_code(codes::TEXT_AFTER_MULTILINE_OPENING)
                .primary(span(s.open, s.open_end), "text follows on the same line");
                if !content.contains('"') {
                    diagnostic = diagnostic.suggest_edits(
                        "for text on one line, use a normal string",
                        vec![
                            Edit { span: span(s.open, s.open_end), replacement: "\"".into() },
                            Edit { span: closer_span, replacement: "\"".into() },
                        ],
                    );
                } else {
                    diagnostic = diagnostic
                        .help("for text on one line, use a normal string such as `\"text\"`");
                }
                self.report(diagnostic);
                return;
            }
            self.report(
                Diagnostic::error(
                    "a string that starts with `\"\"\"` must start its text on the next line",
                )
                .with_code(codes::TEXT_AFTER_MULTILINE_OPENING)
                .primary(span(s.open, s.open_end), "text follows on the same line")
                .suggest(
                    "move the text to the next line",
                    span(text_start, text_start),
                    format!("\n{prefix_text}"),
                ),
            );
        }
        if mid_line {
            self.report(
                Diagnostic::error(format!("the closing `{closer}` must be on its own line"))
                    .with_code(codes::MULTILINE_CLOSING_NOT_ALONE)
                    .primary(closer_span, "this closes the string")
                    .note("the indentation before the closing delimiter is removed from every line")
                    .suggest(
                        "put it on a new line",
                        span(end_start, end_start),
                        format!("\n{prefix_text}"),
                    ),
            );
        }
        for line in &s.lines {
            let ws = &self.text[line.ws_start..line.ws_end];
            if line.blank || ws.starts_with(prefix_text) {
                continue;
            }
            let here = if ws.is_empty() {
                let width = self.char_at(line.ws_start).map_or(0, char::len_utf8);
                span(line.ws_start, line.ws_start + width)
            } else {
                span(line.ws_start, line.ws_end)
            };
            self.report(
                Diagnostic::error(format!(
                    "this line is less indented than the closing `{closer}`"
                ))
                .with_code(codes::LESS_INDENTED_LINE)
                .primary(here, "not indented enough")
                .secondary(closer_span, "this sets the indentation that every line loses")
                .suggest(
                    "indent the line at least as far as the closing delimiter",
                    span(line.ws_start, line.ws_end),
                    prefix_text.to_string(),
                ),
            );
        }
    }

    /// The end of the line or file comes before the string's closing delimiter.
    fn unclosed_string(&mut self, s: StrState) {
        let at = self.pos;
        let closer = Self::closer_text(&s);
        let opening = span(s.open, s.open_end);
        if s.multi {
            self.finish_multiline(s, at, at, (at, at), false);
            self.report(
                Diagnostic::error("this multi-line string is never closed")
                    .with_code(codes::UNCLOSED_STRING)
                    .primary(opening, "the file ends before the closing delimiter")
                    .suggest(
                        format!("end the string with `{closer}` on a line of its own"),
                        span(at, at),
                        format!("\n{closer}"),
                    ),
            );
            return;
        }
        if let Some(closer) = self.swallowed_closer(&s, at) {
            // Read the string as ending before the `)`, so that bracket is closed as intended.
            self.push(STRING_END, closer);
            self.pos = closer;
            let bracket = if self.bytes[closer] == b')' { ")" } else { "]" };
            let quote = Self::closer_text(&s);
            self.report(
                Diagnostic::error("this string is never closed")
                    .with_code(codes::UNCLOSED_STRING)
                    .primary(opening, format!("the line ends before the closing `{quote}`"))
                    .secondary(
                        span(closer, closer + 1),
                        format!("this `{bracket}` is most likely meant to close the bracket before the string"),
                    )
                    .suggest(format!("add `{quote}` before the `{bracket}`"), span(closer, closer), quote),
            );
            return;
        }
        self.push(STRING_END, at);
        let note = if s.raw() {
            "a raw string on one line can't hold a line break; for several lines, start it with `#\"\"\"` on a line of its own"
        } else {
            "a string can't hold a line break; write `\\n` for one, or use a multi-line string that starts with `\"\"\"`"
        };
        let ends = if at >= self.bytes.len() { "the file ends" } else { "the line ends" };
        // A `\\` right before the end would turn an added `"` into the escape `\\"`.
        let dangling_backslash = !s.raw()
            && self.tokens.len() >= 2
            && self.tokens[self.tokens.len() - 2].kind == STRING_ESCAPE
            && self.tokens[self.tokens.len() - 2].start as usize + 1 == at;
        let diagnostic = Diagnostic::error("this string is never closed")
            .with_code(codes::UNCLOSED_STRING)
            .primary(opening, format!("{ends} before the closing `{closer}`"))
            .note(note);
        self.report(if dangling_backslash {
            diagnostic.suggest(
                format!("write the `\\` twice and add `{closer}` where the text ends"),
                span(at, at),
                format!("\\{closer}"),
            )
        } else {
            diagnostic.suggest(format!("add `{closer}` where the text ends"), span(at, at), closer)
        });
    }

    /// For a one-line string that is never closed, as in `print("hello)`: the `)` or `]` in its
    /// text that most likely closes a bracket opened before the string on the same line. It
    /// must be followed only by closers and spaces up to the end of the line, and the string
    /// must end with plain text that holds more than that closer.
    fn swallowed_closer(&mut self, s: &StrState, at: usize) -> Option<usize> {
        let last = *self.tokens.last()?;
        if last.kind != STRING_TEXT {
            return None;
        }
        let text_start = last.start as usize;
        let tail = self.bytes[text_start..at]
            .iter()
            .rev()
            .take_while(|&&b| matches!(b, b')' | b']' | b' ' | b'\t'))
            .count();
        let closer = (at - tail..at).find(|&index| matches!(self.bytes[index], b')' | b']'))?;
        if closer == text_start {
            return None;
        }
        // The innermost bracket opened before the string on its line and not closed there.
        let line_start = self.line_start(s.open);
        let mut depth = 0usize;
        for token in self.tokens[..s.start_token].iter().rev() {
            if (token.start as usize) < line_start {
                break;
            }
            match token.kind {
                R_PAREN | R_BRACK => depth += 1,
                L_PAREN | L_BRACK if depth > 0 => depth -= 1,
                L_PAREN => return (self.bytes[closer] == b')').then_some(closer),
                L_BRACK => return (self.bytes[closer] == b']').then_some(closer),
                _ => {}
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Holes
    // ------------------------------------------------------------------

    fn open_hole(&mut self, s: StrState) {
        let brace = self.pos;
        self.modes.push(Mode::Str(s));
        let hole = HoleState {
            brace,
            start_token: self.tokens.len(),
            diag_len: self.diags.len(),
            brackets: Vec::new(),
            counts: [0; 3],
            items: 0,
            last_was_string: false,
        };
        self.push(HOLE_START, brace);
        self.pos += 1;
        self.modes.push(Mode::Hole(hole));
        self.open_holes += 1;
    }

    /// Reads one token inside the hole on top of the mode stack.
    fn hole_token(&mut self) {
        let Some(Mode::Hole(hole)) = self.modes.pop() else { return };
        let pos = self.pos;
        let Some(&b) = self.bytes.get(pos) else {
            self.fail_hole(hole, 0);
            return;
        };
        match b {
            b'\n' | b'\r' => {
                self.fail_hole(hole, 0);
                return;
            }
            b'}' if hole.counts[2] == 0 => {
                self.close_hole(hole);
                return;
            }
            b':' if hole.brackets.is_empty() => {
                self.format_spec(hole);
                return;
            }
            b'/' if matches!(self.bytes.get(pos + 1), Some(b'/' | b'*')) => {
                self.comment_in_hole();
                self.modes.push(Mode::Hole(hole));
                return;
            }
            _ => {}
        }

        let hole_index = self.modes.len();
        self.modes.push(Mode::Hole(hole));
        let kind = match self.code_token() {
            Some(kind) => {
                self.push(kind, pos);
                kind
            }
            None => STRING_START,
        };
        let Some(Mode::Hole(hole)) = self.modes.get_mut(hole_index) else { return };
        if let Some(slot) = bracket_slot(kind) {
            if kind.is_opening_bracket() {
                hole.brackets.push(kind);
                hole.counts[slot] += 1;
            } else if hole.counts[slot] > 0 {
                // Pop back to the matching opener; the newline pass reports any brackets
                // in between that are never closed.
                while let Some(open) = hole.brackets.pop() {
                    if let Some(open_slot) = bracket_slot(open) {
                        hole.counts[open_slot] -= 1;
                    }
                    if bracket_slot(open) == Some(slot) {
                        break;
                    }
                }
            }
        }
        if !kind.is_trivia() {
            hole.items += 1;
            hole.last_was_string = kind == STRING_START;
        }
    }

    fn close_hole(&mut self, hole: HoleState) {
        let end = self.pos;
        self.push(HOLE_END, end);
        self.pos += 1;
        self.open_holes -= 1;
        if hole.items == 0 {
            let inside = &self.text[hole.brace + 1..end];
            self.report(
                Diagnostic::error("this `{}` in a string is empty")
                    .with_code(codes::EMPTY_HOLE)
                    .primary(span(hole.brace, end + 1), "a value must go between `{` and `}`")
                    .suggest_edits(
                        format!("for literal braces, write `{{{{{inside}}}}}`"),
                        vec![
                            Edit {
                                span: span(hole.brace, hole.brace + 1),
                                replacement: "{{".into(),
                            },
                            Edit { span: span(end, end + 1), replacement: "}}".into() },
                        ],
                    ),
            );
        }
    }

    fn format_spec(&mut self, hole: HoleState) {
        let colon = self.pos;
        let mut end = colon + 1;
        while let Some(&b) = self.bytes.get(end) {
            if matches!(b, b'}' | b'\n' | b'\r') {
                break;
            }
            end += 1;
        }
        if !self.is(end, b'}') {
            self.pos = end;
            self.fail_hole(hole, 0);
            return;
        }
        self.push(FORMAT_SPEC, colon);
        let spec = &self.text[colon + 1..end];
        if parse_format_spec(spec).is_none() {
            let here = span(colon, end);
            let message = if spec.is_empty() {
                "this format spec is empty".to_string()
            } else {
                format!("`{spec}` is not a valid format spec")
            };
            let mut diagnostic = Diagnostic::error(message)
                .with_code(codes::INVALID_FORMAT_SPEC)
                .primary(here, "invalid format spec")
                .note("a format spec is written `[width][.precision][kind]`, as in `.2` (two decimals), `5` (at least 5 characters) or `02x` (hex, padded with zeros to 2 characters)");
            let without_spaces: String = spec.chars().filter(|c| !c.is_whitespace()).collect();
            if hole.items == 1 && hole.last_was_string {
                // Most likely JSON written in a normal string, as in `"{"a": 1}"`.
                diagnostic = diagnostic.help(
                    "to write JSON or other text with braces, use a raw string, such as `#\"{\"a\": 1}\"#`",
                );
            } else if spec.is_empty() {
                diagnostic = diagnostic.suggest("remove the `:`", span(colon, colon + 1), "");
            } else if without_spaces != spec && parse_format_spec(&without_spaces).is_some() {
                diagnostic =
                    diagnostic.suggest("remove the spaces", span(colon + 1, end), without_spaces);
            } else {
                diagnostic = diagnostic.suggest("remove the format spec", here, "");
            }
            self.report(diagnostic);
        }
        if hole.items == 0 {
            self.report(
                Diagnostic::error("this `{` has a format spec but no value")
                    .with_code(codes::HOLE_WITHOUT_VALUE)
                    .primary(span(hole.brace, end + 1), "no value before the `:`")
                    .help("put the value before the `:`, as in `{price:.2}`")
                    .suggest_edits(
                        "for literal braces, double them",
                        vec![
                            Edit {
                                span: span(hole.brace, hole.brace + 1),
                                replacement: "{{".into(),
                            },
                            Edit { span: span(end, end + 1), replacement: "}}".into() },
                        ],
                    ),
            );
        }
        self.push(HOLE_END, end);
        self.pos = end + 1;
        self.open_holes -= 1;
    }

    /// `//` or `/*` inside a hole. The comment is reported and kept as trivia. It runs up to
    /// the next `}` (which then closes the hole) or the end of the line, and a `/*` comment ends
    /// earlier at its `*/`. Everything scanned is consumed, so this stays linear.
    fn comment_in_hole(&mut self) {
        let start = self.pos;
        let block = self.is(start + 1, b'*');
        let mut end = start + 2;
        while let Some(&b) = self.bytes.get(end) {
            if matches!(b, b'\n' | b'\r' | b'}') {
                break;
            }
            if block && b == b'*' && self.is(end + 1, b'/') {
                end += 2;
                break;
            }
            end += 1;
        }
        self.push(if block { BLOCK_COMMENT } else { LINE_COMMENT }, start);
        self.pos = end;
        let here = span(start, end);
        self.report(
            Diagnostic::error("comments can't go inside `{ }` in a string")
                .with_code(codes::COMMENT_IN_HOLE)
                .primary(here, "comment inside a string's `{ }`")
                .suggest("remove the comment, or move it after the string", here, ""),
        );
    }

    /// The innermost hole is never closed: drop everything read since its `{` and read the
    /// rest of the line again as string text (spec/lexical.md, section 9.3). `s`, the string
    /// that was being read inside the hole, has already been taken off the mode stack.
    fn fail_innermost_hole(&mut self) {
        let Some(index) = self.modes.iter().rposition(|mode| matches!(mode, Mode::Hole(_))) else {
            return;
        };
        let mut removed = self.modes.drain(index..);
        let Some(Mode::Hole(hole)) = removed.next() else { return };
        // Only strings can be above the innermost hole.
        let strings_inside = 1 + removed.count();
        self.fail_hole(hole, strings_inside);
    }

    /// See `fail_innermost_hole`. `hole` has already been taken off the mode stack, together
    /// with every mode above it: `strings_inside` strings that were opened inside it and are
    /// still open.
    ///
    /// Reading again is limited to a few times the length of the line. Past that limit, the
    /// hole is instead closed where the line ends (with the same error), so that a line made
    /// of thousands of unclosed holes is still read in linear time.
    fn fail_hole(&mut self, hole: HoleState, strings_inside: usize) {
        self.open_holes -= 1;
        let (line_start, line_end) = self.rescan_line;
        if !(line_start..line_end).contains(&hole.brace) {
            let start =
                self.bytes[..hole.brace].iter().rposition(|&b| b == b'\n').map_or(0, |at| at + 1);
            let end = self.bytes[hole.brace..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(self.bytes.len(), |len| hole.brace + len);
            self.rescan_line = (start, end + 1);
            self.rescanned = 0;
            self.rescan_limit = (end - start).saturating_mul(4).saturating_add(4096);
        }
        let here = span(hole.brace, hole.brace + 1);
        let diagnostic =
            Diagnostic::error("this `{` starts a value inside the string, but it is never closed")
                .with_code(codes::UNCLOSED_HOLE)
                .primary(here, "the line ends before the matching `}`")
                .suggest("for a literal brace, write `{{`", here, "{{")
                .help("for JSON, use a raw string such as `#\"{\"a\": 1}\"#`");
        let cost = self.pos.saturating_sub(hole.brace);
        if self.rescanned.saturating_add(cost) > self.rescan_limit {
            for _ in 0..strings_inside {
                self.push(STRING_END, self.pos);
            }
            self.push(HOLE_END, self.pos);
            self.report(diagnostic);
            return;
        }
        self.rescanned += cost;
        self.tokens.truncate(hole.start_token);
        self.diags.truncate(hole.diag_len);
        self.pos = hole.brace + 1;
        self.push_text(hole.brace);
        self.report(diagnostic);
    }
}
