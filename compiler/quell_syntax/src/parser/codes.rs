//! The stable codes of every parser error and warning. The registry with titles and spec
//! sections is docs/errors.md; keep the two in sync. A code is never reused for a different
//! problem.

// ----------------------------------------------------------------------
// Errors: Q0100 to Q0399
// ----------------------------------------------------------------------

// Something is missing or out of place (the plain "expected X" errors)
pub(crate) const EXPECTED_EXPRESSION: &str = "Q0100";
pub(crate) const EXPECTED_TYPE: &str = "Q0101";
pub(crate) const EXPECTED_PATTERN: &str = "Q0102";
pub(crate) const EXPECTED_NAME: &str = "Q0103";
pub(crate) const EXPECTED_TOKEN: &str = "Q0104";
pub(crate) const UNEXPECTED_TOKEN: &str = "Q0105";
pub(crate) const NESTED_TOO_DEEPLY: &str = "Q0106";

// The errors of spec/grammar.md, section 9.1
pub(crate) const ASSIGNMENT_IN_CONDITION: &str = "Q0110";
pub(crate) const BRACE_ON_NEXT_LINE: &str = "Q0111";
pub(crate) const STATEMENTS_ON_ONE_LINE: &str = "Q0112";
pub(crate) const CHAINED_COMPARISON: &str = "Q0113";
pub(crate) const CHAINED_RANGE_OR_IS: &str = "Q0114";
pub(crate) const NEEDS_PARENTHESES: &str = "Q0115";
pub(crate) const NOT_A_PLACE: &str = "Q0116";
pub(crate) const NAMED_ARGUMENT_WITH_EQUALS: &str = "Q0117";
pub(crate) const NO_STARTING_VALUE: &str = "Q0118";
pub(crate) const NAME_NEEDS_UPPERCASE: &str = "Q0119";
pub(crate) const NAME_NEEDS_LOWERCASE: &str = "Q0120";
pub(crate) const NAME_WITHOUT_CASE: &str = "Q0121";
pub(crate) const PUB_NOT_ALLOWED: &str = "Q0122";
pub(crate) const MODIFIER_OUTSIDE_TYPE: &str = "Q0123";
pub(crate) const FUNCTION_WITHOUT_BODY: &str = "Q0124";
pub(crate) const MUT_IN_TYPE: &str = "Q0125";
pub(crate) const MUT_IN_LAMBDA_TYPE: &str = "Q0126";
pub(crate) const SPAWN_WITHOUT_CALL: &str = "Q0127";
pub(crate) const JUMP_WITH_VALUE: &str = "Q0128";
pub(crate) const UNNAMED_OPTION_FIELD: &str = "Q0129";
pub(crate) const EMPTY_OPTION_PARENTHESES: &str = "Q0130";
pub(crate) const COMMA_AFTER_METHOD: &str = "Q0131";
pub(crate) const UNIT_OR_ONE_ELEMENT_TUPLE: &str = "Q0132";
pub(crate) const HOLE_IN_PLAIN_STRING: &str = "Q0133";
pub(crate) const ITEM_IN_BLOCK: &str = "Q0134";
pub(crate) const STATEMENT_IN_LIBRARY: &str = "Q0135";
pub(crate) const OPTIONAL_TWICE: &str = "Q0136";
pub(crate) const LOWERCASE_NAME_WITH_PAYLOAD: &str = "Q0137";
pub(crate) const CATCH_BRACE_ON_NEXT_LINE: &str = "Q0138";
pub(crate) const FALLBACK_WITH_ARITHMETIC: &str = "Q0139";
pub(crate) const UNCLEAR_JUMP_VALUE: &str = "Q0140";
pub(crate) const OPERATOR_AFTER_BINDING_CATCH: &str = "Q0141";
pub(crate) const MUT_ARGUMENT_NOT_A_PLACE: &str = "Q0142";
pub(crate) const RETURN_BLOCK: &str = "Q0143";
pub(crate) const FORGOTTEN_COMMA: &str = "Q0144";
pub(crate) const IS_NOT: &str = "Q0145";
pub(crate) const IS_NONE: &str = "Q0146";
pub(crate) const NOT_AFTER_OPERATOR: &str = "Q0147";
pub(crate) const COALESCE_ASSIGNMENT: &str = "Q0148";
pub(crate) const ASSIGNMENT_AS_BODY: &str = "Q0149";

// The rejected syntax of spec/grammar.md, section 11
pub(crate) const IF_LET: &str = "Q0150";
pub(crate) const LET_ELSE: &str = "Q0151";
pub(crate) const DOT_OPTION: &str = "Q0152";
pub(crate) const TERNARY: &str = "Q0153";
pub(crate) const QUESTION_MARK_OPERATOR: &str = "Q0154";
pub(crate) const BAR_BETWEEN_PATTERNS: &str = "Q0155";
pub(crate) const STRUCT_LITERAL_WITH_BRACES: &str = "Q0156";
pub(crate) const DOUBLE_COLON: &str = "Q0157";
pub(crate) const INCREMENT_DECREMENT: &str = "Q0158";

// More errors from the rules of spec/grammar.md
pub(crate) const ASSIGNMENT_AS_VALUE: &str = "Q0160";
pub(crate) const SEVERAL_VALUES_IN_HOLE: &str = "Q0161";
pub(crate) const LIST_AND_MAP_MIXED: &str = "Q0162";
pub(crate) const MODIFIER_ORDER: &str = "Q0163";
pub(crate) const COMPTIME_VAR: &str = "Q0164";
pub(crate) const NOT_A_BINDING_PATTERN: &str = "Q0165";
pub(crate) const NUMBERED_FIELD: &str = "Q0166";
pub(crate) const MISSING_SEPARATOR: &str = "Q0167";
pub(crate) const BINDING_CATCH_IN_HEADER: &str = "Q0168";
pub(crate) const MISPLACED_MUT: &str = "Q0169";
pub(crate) const MEMBER_NOT_ALLOWED: &str = "Q0170";
pub(crate) const UNDERSCORE_AS_VALUE: &str = "Q0171";

// Habits from other languages, each with the Quell way as the fix (see `habits.rs`)
pub(crate) const COLON_BLOCK: &str = "Q0180";
pub(crate) const FUNCTION_KEYWORD: &str = "Q0181";
pub(crate) const CLASS_KEYWORD: &str = "Q0182";
pub(crate) const IMPL_KEYWORD: &str = "Q0183";
pub(crate) const INTERFACE_KEYWORD: &str = "Q0184";
pub(crate) const ELSE_IF_WORD: &str = "Q0185";
pub(crate) const CONST_KEYWORD: &str = "Q0186";
pub(crate) const LET_MUT: &str = "Q0187";
pub(crate) const SELF_PARAMETER: &str = "Q0188";
pub(crate) const STRING_PREFIX: &str = "Q0189";
pub(crate) const FROM_IMPORT: &str = "Q0190";
pub(crate) const TRIPLE_EQUALS: &str = "Q0191";
pub(crate) const NEW_KEYWORD: &str = "Q0192";
pub(crate) const PYTHON_CONDITIONAL: &str = "Q0193";
pub(crate) const TYPE_BEFORE_NAME: &str = "Q0194";

// ----------------------------------------------------------------------
// Warnings: Q0400 to Q0499 (spec/grammar.md, section 9.2)
// ----------------------------------------------------------------------

pub(crate) const LINE_STARTS_WITH_MINUS: &str = "Q0400";
pub(crate) const DOT_ON_RIGHT_OPERAND: &str = "Q0401";
pub(crate) const FORGOTTEN_COMMA_WARNING: &str = "Q0402";
pub(crate) const METHOD_ON_NEGATIVE_LITERAL: &str = "Q0403";
pub(crate) const METHOD_ON_RANGE_END: &str = "Q0404";
pub(crate) const TRY_BEFORE_CATCH: &str = "Q0405";
pub(crate) const GENERIC_ARGUMENTS_OVER_LINES: &str = "Q0406";
pub(crate) const JUMP_VALUE_ON_NEXT_LINE: &str = "Q0407";
pub(crate) const DOC_COMMENT_DOCUMENTS_NOTHING: &str = "Q0408";
