//! Turns a diagnostic into the text shown in the terminal:
//!
//! ```text
//! error[Q0207]: `score` can't be changed
//!   --> game.ql:12:5
//!    |
//!  3 | let score = 0
//!    |     ----- declared with `let`, so it can't change
//! ...
//! 12 |     score += 10
//!    |     ^^^^^ trying to change it here
//!    |
//! help: if `score` should change, declare it with `var`:
//!    |
//!  3 | var score = 0
//!    |
//! ```

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::diagnostic::{Diagnostic, Edit, Help, Label, Severity};
use crate::source::{SourceFile, Span};

/// Tabs are shown as this many spaces so underlines line up.
const TAB_WIDTH: usize = 4;

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderOptions {
    /// Use ANSI colors.
    pub color: bool,
}

pub fn render(diagnostic: &Diagnostic, file: &SourceFile, options: RenderOptions) -> String {
    Renderer { diagnostic, file, style: Style { color: options.color } }.render()
}

/// Renders several diagnostics separated by blank lines.
pub fn render_all(diagnostics: &[Diagnostic], file: &SourceFile, options: RenderOptions) -> String {
    let mut out = String::new();
    for (i, diagnostic) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&render(diagnostic, file, options));
    }
    out
}

struct Renderer<'a> {
    diagnostic: &'a Diagnostic,
    file: &'a SourceFile,
    style: Style,
}

impl Renderer<'_> {
    fn render(&self) -> String {
        let mut out = String::new();
        let width = self.gutter_width();
        let gutter = " ".repeat(width);

        self.header(&mut out);

        if let Some(main) = self.diagnostic.main_label() {
            let at = self.file.line_col(main.span.start);
            let arrow = self.style.paint(Paint::Gutter, "-->");
            writeln!(out, "{gutter}{arrow} {}:{}:{}", self.file.name(), at.line, at.col).unwrap();
            self.empty_gutter(&mut out, width);
            self.snippet(&mut out, width);
        }

        let has_footer = !self.diagnostic.notes.is_empty() || !self.diagnostic.helps.is_empty();
        if has_footer && !self.diagnostic.labels.is_empty() {
            self.empty_gutter(&mut out, width);
        }
        for note in &self.diagnostic.notes {
            let eq = self.style.paint(Paint::Gutter, "=");
            let label = self.style.paint(Paint::Bold, "note");
            writeln!(out, "{gutter} {eq} {label}: {note}").unwrap();
        }
        for help in &self.diagnostic.helps {
            self.help(&mut out, help, width);
        }
        out
    }

    fn header(&self, out: &mut String) {
        let severity = self.diagnostic.severity;
        let paint = match severity {
            Severity::Error => Paint::Error,
            Severity::Warning => Paint::Warning,
            Severity::Note => Paint::Note,
        };
        let title = match self.diagnostic.code {
            Some(code) => format!("{}[{code}]", severity.as_str()),
            None => severity.as_str().to_string(),
        };
        let message = self.style.paint(Paint::Bold, &self.diagnostic.message);
        writeln!(
            out,
            "{}{} {message}",
            self.style.paint(paint, &title),
            self.style.paint(Paint::Bold, ":")
        )
        .unwrap();
    }

    /// Every labelled line, in order, with underlines. Gaps of more than one line become `...`.
    fn snippet(&self, out: &mut String, width: usize) {
        let mut by_line: BTreeMap<u32, Vec<&Label>> = BTreeMap::new();
        for label in &self.diagnostic.labels {
            by_line.entry(self.file.line_of(label.span.start)).or_default().push(label);
        }

        let mut previous: Option<u32> = None;
        for (&line, labels) in &mut by_line {
            if let Some(previous) = previous {
                if line == previous + 2 {
                    self.source_line(out, width, previous + 1);
                } else if line > previous + 2 {
                    writeln!(out, "{}", self.style.paint(Paint::Gutter, "...")).unwrap();
                }
            }
            self.source_line(out, width, line);
            labels.sort_by_key(|label| (label.span.start, !label.primary));
            for label in labels.iter() {
                self.underline(out, width, line, label);
            }
            previous = Some(line);
        }
    }

    fn source_line(&self, out: &mut String, width: usize, line: u32) {
        let number = self.style.paint(Paint::Gutter, &format!("{line:>width$} |"));
        let text = expand_tabs(self.file.line_text(line));
        if text.is_empty() {
            writeln!(out, "{number}").unwrap();
        } else {
            writeln!(out, "{number} {text}").unwrap();
        }
    }

    fn underline(&self, out: &mut String, width: usize, line: u32, label: &Label) {
        let line_range = self.file.line_range(line);
        let line_text = self.file.line_text(line);
        // Spans that run past the end of the line are underlined to the end of the line.
        let start = (label.span.start - line_range.start) as usize;
        let end =
            (label.span.end.min(line_range.end).max(label.span.start) - line_range.start) as usize;
        let pad = display_width(&line_text[..start]);
        let length = display_width(&line_text[start..end]).max(1);

        let (marker, paint) =
            if label.primary { ('^', self.primary_paint()) } else { ('-', Paint::Secondary) };
        let markers = marker.to_string().repeat(length);
        let gutter = self.style.paint(Paint::Gutter, &format!("{} |", " ".repeat(width)));
        let mut text = format!("{}{markers}", " ".repeat(pad));
        if !label.message.is_empty() {
            text.push(' ');
            text.push_str(&label.message);
        }
        writeln!(out, "{gutter} {}", self.style.paint(paint, &text)).unwrap();
    }

    fn help(&self, out: &mut String, help: &Help, width: usize) {
        let gutter = " ".repeat(width);
        let label = self.style.paint(Paint::Help, "help");
        if help.edits.is_empty() {
            let eq = self.style.paint(Paint::Gutter, "=");
            writeln!(out, "{gutter} {eq} {label}: {}", help.message).unwrap();
            return;
        }
        writeln!(out, "{label}: {}", help.message).unwrap();
        self.empty_gutter(out, width);
        for (number, text) in fixed_lines(self.file, &help.edits) {
            let gutter = self.style.paint(Paint::Gutter, &format!("{number:>width$} |"));
            let text = expand_tabs(&text);
            if text.is_empty() {
                writeln!(out, "{gutter}").unwrap();
            } else {
                writeln!(out, "{gutter} {}", self.style.paint(Paint::Help, &text)).unwrap();
            }
        }
        self.empty_gutter(out, width);
    }

    fn empty_gutter(&self, out: &mut String, width: usize) {
        writeln!(out, "{}", self.style.paint(Paint::Gutter, &format!("{} |", " ".repeat(width))))
            .unwrap();
    }

    fn primary_paint(&self) -> Paint {
        match self.diagnostic.severity {
            Severity::Error => Paint::Error,
            Severity::Warning => Paint::Warning,
            Severity::Note => Paint::Note,
        }
    }

    /// Wide enough for the largest line number shown anywhere in the diagnostic.
    fn gutter_width(&self) -> usize {
        let mut largest = 1;
        for label in &self.diagnostic.labels {
            largest = largest.max(self.file.line_of(label.span.end));
        }
        for help in &self.diagnostic.helps {
            for (number, _) in fixed_lines(self.file, &help.edits) {
                largest = largest.max(number);
            }
        }
        largest.to_string().len()
    }
}

/// Applies `edits` and returns the changed lines with their line numbers.
/// Lines are numbered from the first original line each group of edits touches.
fn fixed_lines(file: &SourceFile, edits: &[Edit]) -> Vec<(u32, String)> {
    let mut edits: Vec<&Edit> = edits.iter().collect();
    edits.sort_by_key(|edit| edit.span.start);

    // Group edits whose lines touch or overlap, so each group is shown once.
    let mut groups: Vec<(u32, u32, Vec<&Edit>)> = Vec::new();
    for edit in edits {
        let first = file.line_of(edit.span.start);
        let last = file.line_of(edit.span.end);
        match groups.last_mut() {
            Some((_, group_last, group)) if first <= *group_last => {
                *group_last = (*group_last).max(last);
                group.push(edit);
            }
            _ => groups.push((first, last, vec![edit])),
        }
    }

    let mut lines = Vec::new();
    for (first, last, group) in groups {
        let range = file.line_range(first).to(file.line_range(last));
        let mut text = String::new();
        let mut cursor = range.start;
        for edit in group {
            text.push_str(file.slice(Span::new(cursor, edit.span.start)));
            text.push_str(&edit.replacement);
            cursor = edit.span.end;
        }
        text.push_str(file.slice(Span::new(cursor, range.end)));
        for (i, line) in text.split('\n').enumerate() {
            lines.push((first + i as u32, line.trim_end_matches('\r').to_string()));
        }
    }
    lines
}

fn expand_tabs(text: &str) -> String {
    text.replace('\t', &" ".repeat(TAB_WIDTH))
}

fn display_width(text: &str) -> usize {
    text.chars().map(|c| if c == '\t' { TAB_WIDTH } else { 1 }).sum()
}

#[derive(Clone, Copy)]
enum Paint {
    Error,
    Warning,
    Note,
    Help,
    Secondary,
    Gutter,
    Bold,
}

#[derive(Clone, Copy)]
struct Style {
    color: bool,
}

impl Style {
    fn paint(self, paint: Paint, text: &str) -> String {
        if !self.color || text.is_empty() {
            return text.to_string();
        }
        let code = match paint {
            Paint::Error => "1;31",
            Paint::Warning => "1;33",
            Paint::Note => "1;36",
            Paint::Help => "1;32",
            Paint::Secondary => "1;34",
            Paint::Gutter => "1;34",
            Paint::Bold => "1",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(diagnostic: &Diagnostic, file: &SourceFile) -> String {
        render(diagnostic, file, RenderOptions::default())
    }

    fn span_of(file: &SourceFile, needle: &str, nth: usize) -> Span {
        let start = file.text().match_indices(needle).nth(nth).unwrap().0 as u32;
        Span::new(start, start + needle.len() as u32)
    }

    #[test]
    fn matches_the_plan_example() {
        let mut text = String::from("let score = 0\n");
        for _ in 0..8 {
            text.push_str("print(score)\n");
        }
        text.push_str("for i in 0..<3 {\n    print(i)\n    score += 10\n}\n");
        let file = SourceFile::new("game.ql", text);
        let declared = span_of(&file, "score", 0);
        let changed = span_of(&file, "score", 9);
        let keyword = span_of(&file, "let", 0);

        let diagnostic = Diagnostic::error("`score` can't be changed")
            .with_code("Q0207")
            .secondary(declared, "declared with `let`, so it can't change")
            .primary(changed, "trying to change it here")
            .suggest("if `score` should change, declare it with `var`:", keyword, "var");

        let expected = "\
error[Q0207]: `score` can't be changed
  --> game.ql:12:5
   |
 1 | let score = 0
   |     ----- declared with `let`, so it can't change
...
12 |     score += 10
   |     ^^^^^ trying to change it here
   |
help: if `score` should change, declare it with `var`:
   |
 1 | var score = 0
   |
";
        assert_eq!(plain(&diagnostic, &file), expected);
    }

    #[test]
    fn a_single_line_gap_shows_the_line_instead_of_dots() {
        let file = SourceFile::new("a.ql", "let a = 1\nlet b = 2\nprint(a)\n");
        let diagnostic = Diagnostic::warning("unused value")
            .secondary(span_of(&file, "a", 0), "first")
            .primary(span_of(&file, "a", 1), "second");
        let expected = "\
warning: unused value
 --> a.ql:3:7
  |
1 | let a = 1
  |     - first
2 | let b = 2
3 | print(a)
  |       ^ second
";
        assert_eq!(plain(&diagnostic, &file), expected);
    }

    #[test]
    fn notes_plain_help_and_missing_text() {
        let file = SourceFile::new("b.ql", "if x > 5 && y {\n}\n");
        let operator = span_of(&file, "&&", 0);
        let diagnostic = Diagnostic::error("Quell uses `and`, not `&&`")
            .with_code("Q0001")
            .primary(operator, "")
            .note("words are easier to read than symbols")
            .suggest("write `and`:", operator, "and");
        let expected = "\
error[Q0001]: Quell uses `and`, not `&&`
 --> b.ql:1:10
  |
1 | if x > 5 && y {
  |          ^^
  |
  = note: words are easier to read than symbols
help: write `and`:
  |
1 | if x > 5 and y {
  |
";
        assert_eq!(plain(&diagnostic, &file), expected);
    }

    #[test]
    fn empty_spans_and_tabs() {
        let file = SourceFile::new("c.ql", "\tprint(\"hi\"\n");
        let end = file.line_range(1).end;
        let diagnostic = Diagnostic::error("this call is missing its closing `)`")
            .primary(Span::at(end), "add `)` here")
            .help("every `(` needs a matching `)`");
        let expected = "\
error: this call is missing its closing `)`
 --> c.ql:1:12
  |
1 |     print(\"hi\"
  |               ^ add `)` here
  |
  = help: every `(` needs a matching `)`
";
        assert_eq!(plain(&diagnostic, &file), expected);
    }

    #[test]
    fn multi_line_spans_underline_to_the_end_of_the_first_line() {
        let file = SourceFile::new("d.ql", "let x = [\n    1,\n]\n");
        let start = file.text().find('[').unwrap() as u32;
        let end = file.text().find(']').unwrap() as u32 + 1;
        let diagnostic = Diagnostic::error("example").primary(Span::new(start, end), "this list");
        let expected = "\
error: example
 --> d.ql:1:9
  |
1 | let x = [
  |         ^ this list
";
        assert_eq!(plain(&diagnostic, &file), expected);
    }

    #[test]
    fn edits_that_add_lines() {
        let file = SourceFile::new("e.ql", "fn f() {\n    1\n");
        let end = file.text().len() as u32;
        let diagnostic = Diagnostic::error("this block is never closed")
            .primary(span_of(&file, "{", 0), "opened here")
            .suggest("close it:", Span::at(end), "}\n");
        let rendered = plain(&diagnostic, &file);
        assert!(rendered.contains("3 | }\n"), "{rendered}");
    }

    #[test]
    fn color_output_wraps_codes() {
        let file = SourceFile::new("f.ql", "x\n");
        let diagnostic = Diagnostic::error("boom").primary(Span::new(0, 1), "here");
        let rendered = render(&diagnostic, &file, RenderOptions { color: true });
        assert!(rendered.starts_with("\x1b[1;31merror\x1b[0m"));
        assert!(rendered.contains("\x1b[1;31m^ here\x1b[0m"));
    }
}
