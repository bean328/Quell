//! The values of literal pieces: escapes and format specs. The lexer uses these to check
//! literals; later stages use them to compute the value of a string or character.

/// The character an escape stands for, or `None` if the escape is not valid (the lexer has
/// then already reported it). `text` is the text of a `STRING_ESCAPE` token, or an escape
/// inside a `CHAR` token: `\n`, `\u{E9}`, and also the doubled braces `{{` and `}}`.
pub fn escape_value(text: &str) -> Option<char> {
    match text {
        "\\n" => Some('\n'),
        "\\r" => Some('\r'),
        "\\t" => Some('\t'),
        "\\0" => Some('\0'),
        "\\\\" => Some('\\'),
        "\\\"" => Some('"'),
        "\\'" => Some('\''),
        "{{" => Some('{'),
        "}}" => Some('}'),
        _ => {
            let digits = text.strip_prefix("\\u{")?.strip_suffix('}')?;
            if digits.is_empty() || digits.len() > 6 {
                return None;
            }
            u32::from_str_radix(digits, 16).ok().and_then(char::from_u32)
        }
    }
}

/// How a hole's value is written, from a format spec such as `.2` or `02x`
/// (spec/lexical.md, section 9.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FormatSpec {
    /// The minimum number of characters. Shorter output is padded on the left.
    pub width: Option<u32>,
    /// Pad with zeros instead of spaces (the width was written with a leading `0`).
    pub zero_pad: bool,
    /// The number of digits after the decimal point.
    pub precision: Option<u32>,
    /// How to write an integer.
    pub kind: Option<FormatKind>,
}

/// The `Kind` part of a format spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatKind {
    /// `x`: lowercase hex.
    LowerHex,
    /// `X`: uppercase hex.
    UpperHex,
    /// `b`: binary.
    Binary,
    /// `o`: octal.
    Octal,
}

/// Reads a format spec, the text after the `:` of a hole (without the `:`):
///
/// ```text
/// FormatSpec = [ Width ] [ "." Precision ] [ Kind ] ;
/// Width      = [ "0" ] NonZeroDigit { Digit } ;
/// Precision  = Digit { Digit } ;
/// Kind       = "x" | "X" | "b" | "o" ;
/// ```
///
/// Returns `None` if `spec` doesn't follow these rules, if it is empty, or if a number in it
/// is too large to be useful (more than `u32::MAX`).
pub fn parse_format_spec(spec: &str) -> Option<FormatSpec> {
    if spec.is_empty() {
        return None;
    }
    let bytes = spec.as_bytes();
    let mut pos = 0;
    let mut result = FormatSpec::default();

    if bytes.first() == Some(&b'0') {
        result.zero_pad = true;
        pos = 1;
    }
    let width_start = pos;
    if bytes.get(pos).is_some_and(|b| (b'1'..=b'9').contains(b)) {
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        result.width = Some(spec[width_start..pos].parse().ok()?);
    } else if result.zero_pad {
        // A `0` must be followed by the rest of the width.
        return None;
    }

    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        let start = pos;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        if pos == start {
            return None;
        }
        result.precision = Some(spec[start..pos].parse().ok()?);
    }

    if let Some(&b) = bytes.get(pos) {
        result.kind = Some(match b {
            b'x' => FormatKind::LowerHex,
            b'X' => FormatKind::UpperHex,
            b'b' => FormatKind::Binary,
            b'o' => FormatKind::Octal,
            _ => return None,
        });
        pos += 1;
    }

    (pos == bytes.len()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes() {
        assert_eq!(escape_value("\\n"), Some('\n'));
        assert_eq!(escape_value("\\'"), Some('\''));
        assert_eq!(escape_value("{{"), Some('{'));
        assert_eq!(escape_value("\\u{E9}"), Some('\u{E9}'));
        assert_eq!(escape_value("\\u{1F600}"), Some('\u{1F600}'));
        assert_eq!(escape_value("\\u{D800}"), None);
        assert_eq!(escape_value("\\u{110000}"), None);
        assert_eq!(escape_value("\\u{}"), None);
        assert_eq!(escape_value("\\u{0000041}"), None);
        assert_eq!(escape_value("\\q"), None);
    }

    #[test]
    fn format_specs() {
        let spec = |width, zero_pad, precision, kind| FormatSpec { width, zero_pad, precision, kind };
        assert_eq!(parse_format_spec(".2"), Some(spec(None, false, Some(2), None)));
        assert_eq!(parse_format_spec(".1"), Some(spec(None, false, Some(1), None)));
        assert_eq!(
            parse_format_spec("02x"),
            Some(spec(Some(2), true, None, Some(FormatKind::LowerHex)))
        );
        assert_eq!(parse_format_spec("10"), Some(spec(Some(10), false, None, None)));
        assert_eq!(parse_format_spec("010"), Some(spec(Some(10), true, None, None)));
        assert_eq!(parse_format_spec("X"), Some(spec(None, false, None, Some(FormatKind::UpperHex))));
        assert_eq!(
            parse_format_spec("8.3b"),
            Some(spec(Some(8), false, Some(3), Some(FormatKind::Binary)))
        );
        for bad in ["", "0", "00", ".", "2.", " .2", ".2 ", "x2", "02xx", "e", "-5", "99999999999"] {
            assert_eq!(parse_format_spec(bad), None, "{bad:?}");
        }
    }
}
