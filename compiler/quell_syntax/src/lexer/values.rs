//! The values of literal tokens: numbers, characters, escapes and format specs. The lexer uses
//! some of these to check literals; later stages use them to compute values.
//!
//! Each function returns `None` for a literal that the lexer has reported as an error.

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

/// The exact value of an integer literal, with no size limit (spec/lexical.md, section 7.1):
/// its base and its digits, without the prefix and the underscores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntValue {
    /// 10, 16, 2 or 8.
    pub radix: u32,
    /// The digits in that base, most significant first, with no leading zeros (`"0"` for zero).
    /// Hex digits are lowercase.
    pub digits: String,
}

impl IntValue {
    /// The value, if it fits in a `u128`. Whether it fits its Quell type is checked later.
    pub fn to_u128(&self) -> Option<u128> {
        u128::from_str_radix(&self.digits, self.radix).ok()
    }
}

/// The value of an `INT` token's text, such as `1_000`, `0xFF` or `0b1010`.
pub fn int_value(text: &str) -> Option<IntValue> {
    let (radix, digits) = match text.get(..2) {
        Some("0x") => (16, &text[2..]),
        Some("0b") => (2, &text[2..]),
        Some("0o") => (8, &text[2..]),
        _ => (10, text),
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return None;
    }
    if radix == 10 && digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    let mut clean = String::with_capacity(digits.len());
    for c in digits.chars() {
        match c {
            '_' => {}
            _ if c.is_digit(radix) => clean.push(c.to_ascii_lowercase()),
            _ => return None,
        }
    }
    let trimmed = clean.trim_start_matches('0');
    let digits = if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() };
    Some(IntValue { radix, digits })
}

/// The value of a `FLOAT` token's text, such as `1.52`, `2.5e-3` or `1_000.5`, as the nearest
/// `f64`.
pub fn float_value(text: &str) -> Option<f64> {
    let valid = text
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'e' | b'E' | b'+' | b'-'));
    if !valid || text.contains("__") || text.starts_with(['_', '.']) {
        return None;
    }
    let clean: String = text.chars().filter(|&c| c != '_').collect();
    clean.parse().ok()
}

/// The value of a `CHAR` token's text, quotes included: `'A'`, `'\n'`, `'\u{E9}'`.
pub fn char_value(text: &str) -> Option<char> {
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.starts_with('\\') {
        return match escape_value(inner)? {
            '{' | '}' => None,
            c => Some(c),
        };
    }
    let mut chars = inner.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
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
    fn numbers() {
        let int = |text| int_value(text).map(|value| (value.radix, value.digits));
        assert_eq!(int("0"), Some((10, "0".into())));
        assert_eq!(int("1_000_000"), Some((10, "1000000".into())));
        assert_eq!(int("0xFF"), Some((16, "ff".into())));
        assert_eq!(int("0x00ff"), Some((16, "ff".into())));
        assert_eq!(int("0b1010"), Some((2, "1010".into())));
        assert_eq!(int("0o17"), Some((8, "17".into())));
        assert_eq!(int_value("0o17").and_then(|v| v.to_u128()), Some(15));
        // No size limit: the digits are kept exactly.
        let big = "340282366920938463463374607431768211456";
        assert_eq!(int(big), Some((10, big.into())));
        assert_eq!(int_value(big).and_then(|v| v.to_u128()), None);
        for bad in ["", "007", "1__0", "1_", "0x", "0x_f", "0b102", "3px", "0X1"] {
            assert_eq!(int_value(bad), None, "{bad:?}");
        }
        assert_eq!(float_value("1.52"), Some(1.52));
        assert_eq!(float_value("2.5e-3"), Some(0.0025));
        assert_eq!(float_value("1e9"), Some(1e9));
        assert_eq!(float_value("1_000.5"), Some(1000.5));
        assert_eq!(float_value("1.5px"), None);
    }

    #[test]
    fn characters() {
        assert_eq!(char_value("'A'"), Some('A'));
        assert_eq!(char_value("'\u{E9}'"), Some('\u{E9}'));
        assert_eq!(char_value("'\\n'"), Some('\n'));
        assert_eq!(char_value("'\\''"), Some('\''));
        assert_eq!(char_value("'\\u{1F600}'"), Some('\u{1F600}'));
        assert_eq!(char_value("''"), None);
        assert_eq!(char_value("'ab'"), None);
        assert_eq!(char_value("'a"), None);
        assert_eq!(char_value("'\\q'"), None);
    }

    #[test]
    fn format_specs() {
        let spec =
            |width, zero_pad, precision, kind| FormatSpec { width, zero_pad, precision, kind };
        assert_eq!(parse_format_spec(".2"), Some(spec(None, false, Some(2), None)));
        assert_eq!(parse_format_spec(".1"), Some(spec(None, false, Some(1), None)));
        assert_eq!(
            parse_format_spec("02x"),
            Some(spec(Some(2), true, None, Some(FormatKind::LowerHex)))
        );
        assert_eq!(parse_format_spec("10"), Some(spec(Some(10), false, None, None)));
        assert_eq!(parse_format_spec("010"), Some(spec(Some(10), true, None, None)));
        assert_eq!(
            parse_format_spec("X"),
            Some(spec(None, false, None, Some(FormatKind::UpperHex)))
        );
        assert_eq!(
            parse_format_spec("8.3b"),
            Some(spec(Some(8), false, Some(3), Some(FormatKind::Binary)))
        );
        for bad in ["", "0", "00", ".", "2.", " .2", ".2 ", "x2", "02xx", "e", "-5", "99999999999"]
        {
            assert_eq!(parse_format_spec(bad), None, "{bad:?}");
        }
    }
}
