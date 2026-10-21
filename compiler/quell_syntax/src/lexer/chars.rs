//! Character classes used by the lexer.

/// Whether `c` can start an identifier: `_` or a character with the property XID_Start.
pub(crate) fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || (!c.is_ascii() && unicode_ident::is_xid_start(c))
}

/// Whether `c` can continue an identifier (the property XID_Continue).
pub(crate) fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric() || (!c.is_ascii() && unicode_ident::is_xid_continue(c))
}

/// Whether an identifier starting with `c` is an `UPPER_IDENT`: `c` is an uppercase (Lu) or
/// titlecase (Lt) letter (spec/lexical.md, section 5.2).
pub(crate) fn is_upper_start(c: char) -> bool {
    // `char::is_uppercase` is Lu plus Other_Uppercase. Of the Other_Uppercase characters, only
    // the Roman numerals (category Nl) can start an identifier, so they are left out here.
    let roman_numeral = ('\u{2160}'..='\u{216F}').contains(&c);
    (c.is_uppercase() && !roman_numeral) || is_titlecase(c)
}

/// The 31 characters of general category Lt.
fn is_titlecase(c: char) -> bool {
    matches!(
        c,
        '\u{1C5}'
            | '\u{1C8}'
            | '\u{1CB}'
            | '\u{1F2}'
            | '\u{1F88}'..='\u{1F8F}'
            | '\u{1F98}'..='\u{1F9F}'
            | '\u{1FA8}'..='\u{1FAF}'
            | '\u{1FBC}'
            | '\u{1FCC}'
            | '\u{1FFC}'
    )
}

/// Whether `name` is an `UPPER_IDENT`, decided only by its first character
/// (spec/lexical.md, section 5.2). The parser uses this for its declaration checks.
pub fn is_upper_name(name: &str) -> bool {
    name.chars().next().is_some_and(is_upper_start)
}

/// A Unicode space or invisible character that is not a space or a tab
/// (spec/lexical.md, section 3). Returns a short description for the error message.
pub(crate) fn unusual_space(c: char) -> Option<&'static str> {
    let name = match c {
        '\u{A0}' => "a non-breaking space",
        '\u{85}' => "a next-line character",
        '\u{1680}' => "an Ogham space mark",
        '\u{2000}'..='\u{200A}' => "a typographic space",
        '\u{2028}' => "a line separator",
        '\u{2029}' => "a paragraph separator",
        '\u{202F}' => "a narrow non-breaking space",
        '\u{205F}' => "a mathematical space",
        '\u{3000}' => "an ideographic space",
        '\u{200B}' => "a zero-width space",
        '\u{200C}' => "a zero-width non-joiner",
        '\u{200D}' => "a zero-width joiner",
        '\u{2060}' => "a word joiner",
        _ => return None,
    };
    Some(name)
}

/// Whether an unusual space should be replaced by a normal space (visible spaces) or simply
/// removed (zero-width characters).
pub(crate) fn unusual_space_is_visible(c: char) -> bool {
    !matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}')
}

/// A character that is easy to type by accident instead of an ASCII one, for example by a
/// word processor. Returns a description and the ASCII text the user most likely meant.
pub(crate) fn look_alike(c: char) -> Option<(&'static str, &'static str)> {
    let found = match c {
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => ("a curly double quote", "\""),
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => ("a curly single quote", "'"),
        '\u{2212}' => ("a minus sign", "-"),
        '\u{2010}'..='\u{2015}' => ("a dash", "-"),
        '\u{D7}' => ("a multiplication sign", "*"),
        '\u{F7}' => ("a division sign", "/"),
        '\u{2264}' => ("a less-than-or-equal sign", "<="),
        '\u{2265}' => ("a greater-than-or-equal sign", ">="),
        '\u{2260}' => ("a not-equal sign", "!="),
        '\u{2192}' => ("an arrow", "->"),
        '\u{21D2}' => ("a double arrow", "=>"),
        '\u{FF08}' => ("a full-width parenthesis", "("),
        '\u{FF09}' => ("a full-width parenthesis", ")"),
        '\u{FF0C}' => ("a full-width comma", ","),
        '\u{FF1A}' => ("a full-width colon", ":"),
        _ => return None,
    };
    Some(found)
}

/// Whether `c` is a control character that is an error outside comments
/// (spec/lexical.md, section 2): U+0001 to U+001F and U+007F, except tab, LF and CR.
/// NUL is handled separately because it is an error everywhere.
pub(crate) fn is_forbidden_control(byte: u8) -> bool {
    (byte < 0x20 && !matches!(byte, 0 | b'\t' | b'\n' | b'\r')) || byte == 0x7F
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_and_lower_names() {
        for name in ["Player", "T", "Int", "UInt8", "\u{C9}cole", "\u{1C5}x", "\u{394}elta"] {
            assert!(is_upper_name(name), "{name}");
        }
        for name in ["score", "_unused", "c", "\u{5929}\u{6C14}", "\u{2160}x", "\u{E9}t\u{E9}"] {
            assert!(!is_upper_name(name), "{name}");
        }
    }

    #[test]
    fn identifier_characters() {
        assert!(is_ident_start('_') && is_ident_start('a') && is_ident_start('\u{E9}'));
        assert!(!is_ident_start('1') && !is_ident_start('$'));
        assert!(is_ident_continue('1') && is_ident_continue('\u{301}'));
        // Since Unicode 15.1 the zero-width joiner is XID_Continue; the word joiner is not.
        assert!(is_ident_continue('\u{200D}'));
        assert!(!is_ident_continue('-') && !is_ident_continue('\u{2060}'));
    }
}
