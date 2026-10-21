//! Helpers that write fixes: the naming rule (spec/lexical.md, 5.3), indentation, and the
//! naming errors themselves.

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::Parser;
use super::codes;
use crate::{SyntaxKind, is_upper_name};

/// `name` changed to start with a capital letter, in the PascalCase style: `player_info`
/// becomes `PlayerInfo`. `None` if that is not possible (a script without capital letters) or
/// would give a reserved word.
pub(crate) fn to_upper_name(name: &str) -> Option<String> {
    let mut out = String::new();
    for part in name.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    (is_upper_name(&out) && SyntaxKind::from_keyword(&out).is_none()).then_some(out)
}

/// `name` changed to start with a lowercase letter, in the snake_case style: `TakeDamage`
/// becomes `take_damage` and `HTTPServer` becomes `http_server`. `None` if that would give a
/// reserved word.
pub(crate) fn to_lower_name(name: &str) -> Option<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && index > 0 {
            let previous = chars[index - 1];
            let next_is_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            let starts_word = previous.is_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_uppercase() && next_is_lower);
            if starts_word && !out.ends_with('_') {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    (!out.is_empty() && !is_upper_name(&out) && SyntaxKind::from_keyword(&out).is_none())
        .then_some(out)
}

/// A field name made from a type, for the fix that names an enum option's field:
/// `Float` gives `float`, `AgeError` gives `age_error`, `List<Int>` gives `list`.
pub(crate) fn field_name_for_type(type_text: &str) -> String {
    let base: String =
        type_text.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
    to_lower_name(&base).unwrap_or_else(|| "value".to_string())
}

/// The spaces and tabs at the start of the line that holds `offset`.
pub(crate) fn indentation_at(text: &str, offset: u32) -> &str {
    let offset = (offset as usize).min(text.len());
    let start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let rest = &text[start..];
    let len = rest.find(|c: char| c != ' ' && c != '\t').unwrap_or(rest.len());
    &rest[..len]
}

/// The edits that put `(` before `start` and `)` at `end`.
pub(crate) fn wrap_in_parens(start: u32, end: u32) -> Vec<Edit> {
    vec![
        Edit { span: Span::at(start), replacement: "(".to_string() },
        Edit { span: Span::at(end), replacement: ")".to_string() },
    ]
}

/// What a declared name must look like at a declaration site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Case {
    Upper,
    Lower,
}

impl Parser<'_> {
    /// Checks the first letter of the name declared at `span` (spec/lexical.md, 5.3). `what`
    /// names the kind of declaration in the plural, as in "struct names".
    pub(crate) fn check_name(&mut self, span: Span, case: Case, what: &str) {
        let name = self.text_of(span);
        if name.is_empty() || self.speculating() {
            return;
        }
        let upper = is_upper_name(name);
        let diagnostic = match case {
            Case::Upper if !upper => match to_upper_name(name) {
                Some(fixed) => Diagnostic::error(format!("{what} start with a capital letter"))
                    .with_code(codes::NAME_NEEDS_UPPERCASE)
                    .primary(span, "this name starts with a lowercase letter")
                    .suggest(format!("write `{fixed}`"), span, fixed),
                None if starts_with_caseless_letter(name) => {
                    Diagnostic::error(format!("{what} must start with an uppercase letter"))
                        .with_code(codes::NAME_WITHOUT_CASE)
                        .primary(span, "this name starts with a letter that has no uppercase form")
                        .note(
                            "Quell tells types and enum options from other names by their first \
                             letter, so they need a letter such as A to Z at the start",
                        )
                        .help(
                            "rename it so that it starts with an uppercase letter; there is no \
                             automatic fix, because the script has no capital letters",
                        )
                }
                None => Diagnostic::error(format!("{what} start with a capital letter"))
                    .with_code(codes::NAME_NEEDS_UPPERCASE)
                    .primary(span, "this name starts with a lowercase letter")
                    .help("rename it so that it starts with a capital letter"),
            },
            Case::Lower if upper => match to_lower_name(name) {
                Some(fixed) => Diagnostic::error(format!("{what} start with a lowercase letter"))
                    .with_code(codes::NAME_NEEDS_LOWERCASE)
                    .primary(span, "this name starts with a capital letter")
                    .note("only types, enum options and generic parameters start with a capital")
                    .suggest(format!("write `{fixed}`"), span, fixed),
                None => Diagnostic::error(format!("{what} start with a lowercase letter"))
                    .with_code(codes::NAME_NEEDS_LOWERCASE)
                    .primary(span, "this name starts with a capital letter")
                    .help("rename it so that it starts with a lowercase letter"),
            },
            _ => return,
        };
        self.push_error(diagnostic);
    }
}

/// Whether the name starts with a letter that has no uppercase form (Chinese, Japanese, ...).
fn starts_with_caseless_letter(name: &str) -> bool {
    name.trim_start_matches('_')
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() && !c.is_lowercase() && !c.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_conversions() {
        assert_eq!(to_upper_name("player").as_deref(), Some("Player"));
        assert_eq!(to_upper_name("player_info").as_deref(), Some("PlayerInfo"));
        assert_eq!(to_upper_name("_x").as_deref(), Some("X"));
        assert_eq!(to_upper_name("\u{5929}\u{6C14}"), None);
        assert_eq!(to_lower_name("TakeDamage").as_deref(), Some("take_damage"));
        assert_eq!(to_lower_name("HTTPServer").as_deref(), Some("http_server"));
        assert_eq!(to_lower_name("Item").as_deref(), Some("item"));
        assert_eq!(to_lower_name("X").as_deref(), Some("x"));
        assert_eq!(to_lower_name("Point2D").as_deref(), Some("point2_d"));
        assert_eq!(to_lower_name("Fn"), None, "`fn` is a keyword");
        assert_eq!(field_name_for_type("Float"), "float");
        assert_eq!(field_name_for_type("AgeError"), "age_error");
        assert_eq!(field_name_for_type("List<Int>"), "list");
        assert_eq!(field_name_for_type("(Int, Int)"), "value");
        assert_eq!(indentation_at("a\n    b", 7), "    ");
        assert_eq!(indentation_at("\tx", 1), "\t");
        assert!(starts_with_caseless_letter("\u{5929}\u{6C14}"));
        assert!(!starts_with_caseless_letter("weather"));
    }
}
