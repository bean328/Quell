//! The stable codes of every lexer error. The registry with titles and spec sections is
//! docs/errors.md; keep the two in sync. A code is never reused for a different problem.

// Source text (spec/lexical.md, section 2)
pub(crate) const INVALID_UTF8: &str = "Q0001";
pub(crate) const STRAY_BYTE_ORDER_MARK: &str = "Q0002";
pub(crate) const NUL_CHARACTER: &str = "Q0003";
pub(crate) const CONTROL_CHARACTER: &str = "Q0004";

// Whitespace and line breaks (section 3)
pub(crate) const LONE_CARRIAGE_RETURN: &str = "Q0005";
pub(crate) const UNUSUAL_SPACE: &str = "Q0006";

// Comments (section 4)
pub(crate) const UNCLOSED_BLOCK_COMMENT: &str = "Q0007";
pub(crate) const UNCLOSED_DOC_FENCE: &str = "Q0008";

// Identifiers (section 5)
pub(crate) const NOT_NFC: &str = "Q0009";

// Numbers (section 7)
pub(crate) const UPPERCASE_PREFIX: &str = "Q0010";
pub(crate) const LEADING_ZERO: &str = "Q0011";
pub(crate) const PREFIX_WITHOUT_DIGITS: &str = "Q0012";
pub(crate) const DIGIT_OUTSIDE_BASE: &str = "Q0013";
pub(crate) const MISPLACED_UNDERSCORE: &str = "Q0014";
pub(crate) const NUMBER_SUFFIX: &str = "Q0015";

// Characters (section 8)
pub(crate) const EMPTY_CHAR: &str = "Q0020";
pub(crate) const MULTI_CHAR: &str = "Q0021";
pub(crate) const UNCLOSED_CHAR: &str = "Q0022";

// Strings (section 9)
pub(crate) const UNKNOWN_ESCAPE: &str = "Q0030";
pub(crate) const BRACE_ESCAPE: &str = "Q0031";
pub(crate) const MALFORMED_UNICODE_ESCAPE: &str = "Q0032";
pub(crate) const INVALID_UNICODE_SCALAR: &str = "Q0033";
pub(crate) const UNCLOSED_STRING: &str = "Q0034";
pub(crate) const CONTROL_IN_STRING: &str = "Q0035";
pub(crate) const LONE_CLOSING_BRACE: &str = "Q0036";
pub(crate) const EMPTY_HOLE: &str = "Q0037";
pub(crate) const HOLE_WITHOUT_VALUE: &str = "Q0038";
pub(crate) const UNCLOSED_HOLE: &str = "Q0039";
pub(crate) const COMMENT_IN_HOLE: &str = "Q0040";
pub(crate) const INVALID_FORMAT_SPEC: &str = "Q0041";
pub(crate) const TEXT_AFTER_MULTILINE_OPENING: &str = "Q0042";
pub(crate) const MULTILINE_CLOSING_NOT_ALONE: &str = "Q0043";
pub(crate) const LESS_INDENTED_LINE: &str = "Q0044";

// Tokens that exist only to give a helpful error (section 11)
pub(crate) const DOUBLE_AMPERSAND: &str = "Q0050";
pub(crate) const DOUBLE_BAR: &str = "Q0051";
pub(crate) const EXCLAMATION_MARK: &str = "Q0052";
pub(crate) const BARE_RANGE: &str = "Q0053";
pub(crate) const SEMICOLON: &str = "Q0054";
pub(crate) const UNMATCHED_COMMENT_CLOSE: &str = "Q0055";
pub(crate) const STRAY_HASH: &str = "Q0056";
pub(crate) const UNKNOWN_CHARACTER: &str = "Q0057";
pub(crate) const LOOK_ALIKE_CHARACTER: &str = "Q0058";

// Brackets (section 12.2)
pub(crate) const UNMATCHED_CLOSING_BRACKET: &str = "Q0060";
pub(crate) const UNCLOSED_BRACKET: &str = "Q0061";

// Limits
pub(crate) const FILE_TOO_LARGE: &str = "Q0070";
