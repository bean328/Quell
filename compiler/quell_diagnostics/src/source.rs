//! Source files and positions inside them.

use std::fmt;

/// A byte range inside one source file. `start` is inclusive, `end` is exclusive.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Span {
        debug_assert!(start <= end, "span start {start} is after its end {end}");
        Span { start, end }
    }

    /// An empty span at `offset`, used for "something is missing here" errors.
    pub fn at(offset: u32) -> Span {
        Span { start: offset, end: offset }
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The smallest span that covers both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span { start: self.start.min(other.start), end: self.end.max(other.end) }
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A line and column, both counted from 1. Columns count characters, not bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
}

/// The text of one file, with an index of where each line starts.
#[derive(Debug, Clone)]
pub struct SourceFile {
    name: String,
    text: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let text = text.into();
        assert!(u32::try_from(text.len()).is_ok(), "source files must be smaller than 4 GiB");
        let mut line_starts = vec![0];
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        SourceFile { name: name.into(), text, line_starts }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn slice(&self, span: Span) -> &str {
        &self.text[span.start as usize..span.end as usize]
    }

    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }

    /// The line (counted from 1) that contains byte `offset`.
    pub fn line_of(&self, offset: u32) -> u32 {
        match self.line_starts.binary_search(&offset) {
            Ok(index) => index as u32 + 1,
            Err(index) => index as u32,
        }
    }

    pub fn line_col(&self, offset: u32) -> LineCol {
        let line = self.line_of(offset);
        let line_start = self.line_starts[line as usize - 1] as usize;
        let col = self.text[line_start..offset as usize].chars().count() as u32 + 1;
        LineCol { line, col }
    }

    /// The text of line `line` (counted from 1), without its line ending.
    pub fn line_text(&self, line: u32) -> &str {
        let range = self.line_range(line);
        &self.text[range.start as usize..range.end as usize]
    }

    /// The byte range of line `line`, without its line ending.
    pub fn line_range(&self, line: u32) -> Span {
        let index = line as usize - 1;
        let start = self.line_starts[index];
        let mut end =
            self.line_starts.get(index + 1).map_or(self.text.len() as u32, |next| next - 1);
        if end > start && self.text.as_bytes()[end as usize - 1] == b'\r' {
            end -= 1;
        }
        Span::new(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_columns() {
        let file = SourceFile::new("test.ql", "let a = 1\nlet bé = 2\r\n\nprint(a)");
        assert_eq!(file.line_count(), 4);
        assert_eq!(file.line_text(1), "let a = 1");
        assert_eq!(file.line_text(2), "let bé = 2");
        assert_eq!(file.line_text(3), "");
        assert_eq!(file.line_text(4), "print(a)");
        assert_eq!(file.line_col(0), LineCol { line: 1, col: 1 });
        // "=" on line 2 comes after a two-byte character, so its byte offset and column differ.
        let eq = file.text().find("= 2").unwrap() as u32;
        assert_eq!(file.line_col(eq), LineCol { line: 2, col: 8 });
        assert_eq!(file.line_of(file.text().len() as u32), 4);
    }

    #[test]
    fn span_union() {
        assert_eq!(Span::new(4, 6).to(Span::new(1, 2)), Span::new(1, 6));
        assert!(Span::at(3).is_empty());
    }
}
