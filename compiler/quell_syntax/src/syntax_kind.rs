//! The kinds of tokens (and, later, nodes) in a Quell syntax tree.
//!
//! Every token the lexer produces has a [`SyntaxKind`]. The kinds come from
//! spec/lexical.md, section 13 and spec/grammar.md, section 1.2, plus the trivia kinds, the
//! pieces of a string literal, and the tokens that exist only to give a helpful error
//! (spec/lexical.md, section 11).
//!
//! A few classes from the grammar are not kinds of their own:
//!
//! - `IDENT` is either [`UPPER_IDENT`](SyntaxKind::UPPER_IDENT) or
//!   [`LOWER_IDENT`](SyntaxKind::LOWER_IDENT); use [`SyntaxKind::is_ident`].
//! - `STRING` is a sequence of tokens from [`STRING_START`](SyntaxKind::STRING_START) to
//!   [`STRING_END`](SyntaxKind::STRING_END) (see the lexer module docs). `PLAIN_STRING` is a
//!   string with no [`HOLE_START`](SyntaxKind::HOLE_START) between those two.
//! - The contextual keywords `any`, `type`, `parallel`, `test` and `c` are ordinary identifiers;
//!   the parser recognizes them by spelling ([`is_contextual_keyword`]).

/// Defines `SyntaxKind` and the table that turns a raw `u16` back into a kind.
macro_rules! syntax_kinds {
    ($($(#[$meta:meta])* $name:ident,)*) => {
        /// The kind of a token or node. See the module docs.
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$meta])* $name,)*
        }

        impl SyntaxKind {
            /// Every kind, in declaration order, so `ALL[k as usize] == k`.
            const ALL: &'static [SyntaxKind] = &[$(SyntaxKind::$name,)*];
        }
    };
}

syntax_kinds! {
    // ------------------------------------------------------------------
    // Trivia: kept in the lossless tree, never seen by the grammar.
    // ------------------------------------------------------------------
    /// Spaces and tabs (and, after an error, other space characters).
    WHITESPACE,
    /// A line break (LF or CR LF, or after an error a lone CR) that the newline rules removed.
    LINE_BREAK,
    /// `// ...` up to the end of the line, line break not included.
    LINE_COMMENT,
    /// `/// ...` (exactly three slashes) up to the end of the line.
    DOC_COMMENT,
    /// `/* ... */`, which may nest and span lines.
    BLOCK_COMMENT,
    /// A byte order mark at the very start of the file.
    BYTE_ORDER_MARK,
    /// `;`. Always an error; it counts as a line break for the newline rules.
    SEMICOLON,
    /// Text the lexer reported and skipped (an unknown character, a stray `*/` or `#`, ...).
    /// It is trivia, so the parser reads the code around it as if it were not there.
    ERROR,

    // ------------------------------------------------------------------
    // Layout
    // ------------------------------------------------------------------
    /// A significant line break (spec/lexical.md, section 12). Its text is the line break,
    /// or empty when the break comes from a block comment or `;`.
    NEWLINE,
    /// The end of the file. Always the last token, always empty.
    EOF,

    // ------------------------------------------------------------------
    // Names and literals
    // ------------------------------------------------------------------
    /// An identifier whose first character is an uppercase or titlecase letter.
    UPPER_IDENT,
    /// Any other identifier.
    LOWER_IDENT,
    /// The wildcard `_`.
    UNDERSCORE,
    /// An integer literal: `0`, `1_000`, `0xff`, `0b1010`, `0o17`.
    INT,
    /// A float literal: `1.52`, `2.5e-3`, `1e9`.
    FLOAT,
    /// A character literal, quotes included: `'A'`, `'\n'`.
    CHAR,

    // ------------------------------------------------------------------
    // The pieces of a string literal (see the lexer module docs)
    // ------------------------------------------------------------------
    /// The opening delimiter: `"`, `#"`, or for a multi-line string `"""` together with the
    /// spaces and the line break after it.
    STRING_START,
    /// Literal text. Its value is its text.
    STRING_TEXT,
    /// An escape such as `\n` or `\u{E9}`, or a doubled brace `{{` or `}}`.
    STRING_ESCAPE,
    /// Indentation that a multi-line string removes. Its value is empty.
    STRING_INDENT,
    /// A line break inside a multi-line string. Its value is `\n`.
    STRING_LINE_BREAK,
    /// The closing delimiter: `"`, `"#`, or for a multi-line string the last line break, the
    /// closing line's indentation and `"""`. Empty if the string is never closed.
    STRING_END,
    /// The `{` that starts a hole (an interpolated value).
    HOLE_START,
    /// `:` and the format spec after it, as in `:.2`.
    FORMAT_SPEC,
    /// The `}` that ends a hole.
    HOLE_END,

    // ------------------------------------------------------------------
    // Operators and punctuation (spec/lexical.md, section 10)
    // ------------------------------------------------------------------
    /// `+`
    PLUS,
    /// `-`
    MINUS,
    /// `*`
    STAR,
    /// `/`
    SLASH,
    /// `%`
    PERCENT,
    /// `==`
    EQ_EQ,
    /// `!=`
    BANG_EQ,
    /// `<`
    LT,
    /// `<=`
    LT_EQ,
    /// `>`
    GT,
    /// `>=`
    GT_EQ,
    /// `&`
    AMP,
    /// `|`
    PIPE,
    /// `^`
    CARET,
    /// `~`
    TILDE,
    /// `<<`
    LT_LT,
    /// `>>`
    GT_GT,
    /// `=`
    EQ,
    /// `+=`
    PLUS_EQ,
    /// `-=`
    MINUS_EQ,
    /// `*=`
    STAR_EQ,
    /// `/=`
    SLASH_EQ,
    /// `%=`
    PERCENT_EQ,
    /// `&=`
    AMP_EQ,
    /// `|=`
    PIPE_EQ,
    /// `^=`
    CARET_EQ,
    /// `<<=`
    LT_LT_EQ,
    /// `>>=`
    GT_GT_EQ,
    /// `..<`
    DOT_DOT_LT,
    /// `..=`
    DOT_DOT_EQ,
    /// `.`
    DOT,
    /// `?.`
    QUESTION_DOT,
    /// `??`
    QUESTION_QUESTION,
    /// `?`
    QUESTION,
    /// `=>`
    FAT_ARROW,
    /// `->`
    THIN_ARROW,
    /// `@`
    AT,
    /// `,`
    COMMA,
    /// `:`
    COLON,
    /// `(`
    L_PAREN,
    /// `)`
    R_PAREN,
    /// `[`
    L_BRACK,
    /// `]`
    R_BRACK,
    /// `{`
    L_BRACE,
    /// `}`
    R_BRACE,

    // ------------------------------------------------------------------
    // Tokens that exist only to give a helpful error (spec/lexical.md, section 11).
    // The lexer has already reported each one. The parser treats it as
    // `SyntaxKind::recovered` says.
    // ------------------------------------------------------------------
    /// `&&`, which acts as `and`.
    AMP_AMP,
    /// `||`, which acts as `or`.
    PIPE_PIPE,
    /// `!` not followed by `=`, which acts as `not`.
    BANG,
    /// `..` not followed by `<` or `=`, which acts as `..<`.
    DOT_DOT,
    /// `...`, which acts as `..<`.
    DOT_DOT_DOT,

    // ------------------------------------------------------------------
    // Reserved keywords (spec/lexical.md, section 6.1). Keep them together and in this
    // order: `is_keyword` checks the range from AND_KW to WHILE_KW.
    // ------------------------------------------------------------------
    /// `and`
    AND_KW,
    /// `assert`
    ASSERT_KW,
    /// `break`
    BREAK_KW,
    /// `catch`
    CATCH_KW,
    /// `comptime`
    COMPTIME_KW,
    /// `continue`
    CONTINUE_KW,
    /// `defer`
    DEFER_KW,
    /// `div`
    DIV_KW,
    /// `else`
    ELSE_KW,
    /// `enum`
    ENUM_KW,
    /// `extend`
    EXTEND_KW,
    /// `false`
    FALSE_KW,
    /// `fn`
    FN_KW,
    /// `for`
    FOR_KW,
    /// `group`
    GROUP_KW,
    /// `if`
    IF_KW,
    /// `import`
    IMPORT_KW,
    /// `in`
    IN_KW,
    /// `is`
    IS_KW,
    /// `let`
    LET_KW,
    /// `match`
    MATCH_KW,
    /// `mut`
    MUT_KW,
    /// `none`
    NONE_KW,
    /// `not`
    NOT_KW,
    /// `or`
    OR_KW,
    /// `pub`
    PUB_KW,
    /// `repeat`
    REPEAT_KW,
    /// `return`
    RETURN_KW,
    /// `self`
    SELF_KW,
    /// `spawn`
    SPAWN_KW,
    /// `static`
    STATIC_KW,
    /// `struct`
    STRUCT_KW,
    /// `throw`
    THROW_KW,
    /// `throws`
    THROWS_KW,
    /// `trait`
    TRAIT_KW,
    /// `true`
    TRUE_KW,
    /// `try`
    TRY_KW,
    /// `unsafe`
    UNSAFE_KW,
    /// `var`
    VAR_KW,
    /// `while`
    WHILE_KW,

    // ------------------------------------------------------------------
    // Node kinds (spec/grammar.md). They come after every token kind, so that
    // `is_keyword`, `is_string_piece` and the other range checks above stay correct.
    // Keep them together and in this order: `is_node` checks the range from FILE to OR_PAT.
    // Code the parser could not read is wrapped in a node of kind ERROR (the same kind as the
    // trivia token for skipped text; rowan allows a node and a token to share a kind).
    // ------------------------------------------------------------------
    /// A whole file (grammar.md, 2.1): items and statements, then `EOF`.
    FILE,

    // Items (grammar.md, 3)
    /// `@name` or `@name(args)` before an item or member (3.1).
    ATTRIBUTE,
    /// A function or method (3.3).
    FN_DECL,
    /// The `( )` of a function's parameters.
    PARAM_LIST,
    /// One parameter: a name, an optional type and an optional default value.
    PARAM,
    /// `-> Type` after the parameters.
    RET_TYPE,
    /// `throws` with an optional error type.
    THROWS_CLAUSE,
    /// The `< >` of a declaration's generic parameters (3.4).
    GENERIC_PARAM_LIST,
    /// One generic parameter with its requirements: `T: Compare and Show`.
    GENERIC_PARAM,
    /// `struct Name { ... }` (3.5).
    STRUCT_DECL,
    /// `enum Name { ... }` (3.6).
    ENUM_DECL,
    /// `trait Name { ... }` (3.7).
    TRAIT_DECL,
    /// `extend Type { ... }` (3.8).
    EXTEND_DECL,
    /// `: Trait and Other` after a struct, enum or extend header.
    CONFORMANCE,
    /// The `{ }` body of a struct, enum, trait or extend, with its members.
    MEMBER_LIST,
    /// A struct field: `name: Type = default`.
    FIELD,
    /// An enum option: `Sunny` or `Rainy(mm: Float)`.
    ENUM_OPTION,
    /// The `( )` of an enum option's fields.
    OPTION_FIELD_LIST,
    /// One field of an enum option: `mm: Float`.
    OPTION_FIELD,
    /// `import std.math.{pi}` or `import c "header.h"` (3.9).
    IMPORT_DECL,
    /// The dotted module path of an import: `std.math`.
    MODULE_PATH,
    /// The `{ }` list of names of an import.
    IMPORT_LIST,
    /// `test "name" { ... }` (3.10).
    TEST_DECL,
    /// `comptime { ... }` (3.11).
    COMPTIME_BLOCK,
    /// A `let` or `var` at the top level of a file (3.12).
    GLOBAL_LET,

    // Blocks and statements (grammar.md, 4)
    /// `{ statements }`, as a body or as a value (4.1, 6.14).
    BLOCK,
    /// `let` or `var` inside a block (4.3).
    LET_STMT,
    /// `place = value` and the compound assignments (4.4).
    ASSIGN_STMT,
    /// `for pattern in items { ... }` (4.5).
    FOR_STMT,
    /// `while condition { ... }` (4.6).
    WHILE_STMT,
    /// `repeat count { ... }` or `repeat { ... }` (4.7).
    REPEAT_STMT,
    /// `defer { ... }` (4.8).
    DEFER_STMT,
    /// `assert condition, message` (4.9).
    ASSERT_STMT,
    /// An expression used as a statement (4.10).
    EXPR_STMT,

    // Expressions (grammar.md, 6)
    /// A number, a character, `true`, `false` or `none`.
    LITERAL,
    /// A string literal: `STRING_START` ... `STRING_END`, with its holes (6.13).
    STRING_LITERAL,
    /// One hole in a string: `{`, an expression, an optional format spec, `}`.
    HOLE,
    /// A name, with optional generic arguments: `score`, `List<Int>`.
    NAME_EXPR,
    /// `self`.
    SELF_EXPR,
    /// An expression in parentheses: `(a + b)`.
    PAREN_EXPR,
    /// A tuple: `(low, high)`.
    TUPLE_EXPR,
    /// A list: `[1, 2, 3]` or `[]`.
    LIST_EXPR,
    /// A map: `["Ava": 12]` or `[:]`.
    MAP_EXPR,
    /// One `key: value` entry of a map.
    MAP_ENTRY,
    /// `if condition { ... } else { ... }` (6.15).
    IF_EXPR,
    /// `match value { arms }` (6.16).
    MATCH_EXPR,
    /// The `{ }` of a `match`, with its arms.
    MATCH_ARM_LIST,
    /// One arm: `pattern if guard => value`.
    MATCH_ARM,
    /// The `if guard` of an arm.
    MATCH_GUARD,
    /// `parallel for pattern in items { ... }` (6.17).
    PARALLEL_FOR_EXPR,
    /// `group { ... }`.
    GROUP_EXPR,
    /// `unsafe { ... }`.
    UNSAFE_EXPR,
    /// `spawn { ... }` or `spawn call()` (6.9).
    SPAWN_EXPR,
    /// `-x`, `~x` or `not x`.
    PREFIX_EXPR,
    /// `try x`.
    TRY_EXPR,
    /// Two operands and an operator: arithmetic, bitwise, comparison, `and`, `or` and `??`.
    BINARY_EXPR,
    /// `a..<b` or `a..=b` (6.7).
    RANGE_EXPR,
    /// `value is Type` (6.4).
    IS_EXPR,
    /// `value catch fallback` or `value catch name { ... }` (6.5).
    CATCH_EXPR,
    /// A call: `f(args)`.
    CALL_EXPR,
    /// The `( )` of a call, or of an attribute.
    ARG_LIST,
    /// One argument: an optional `name:`, then a value, a `mut` place or a lambda.
    ARG,
    /// `.name` or `?.name` after a value, with optional generic arguments.
    MEMBER_EXPR,
    /// `value[index]`.
    INDEX_EXPR,
    /// The `< >` of generic arguments, in an expression or a type (6.11).
    GENERIC_ARG_LIST,
    /// `params => body` (6.18).
    LAMBDA_EXPR,
    /// The `( )` of a lambda's parameters.
    LAMBDA_PARAM_LIST,
    /// One lambda parameter: optional `mut`, a name and an optional type.
    LAMBDA_PARAM,
    /// `return` with an optional value (6.19).
    RETURN_EXPR,
    /// `throw value`.
    THROW_EXPR,
    /// `break`.
    BREAK_EXPR,
    /// `continue`.
    CONTINUE_EXPR,

    // Types (grammar.md, 7)
    /// A type name or path, with optional generic arguments: `Int`, `http.Request`, `List<T>`.
    PATH_TYPE,
    /// `any Trait`.
    ANY_TYPE,
    /// `type`, the type of types.
    TYPE_TYPE,
    /// `Type?`.
    OPTIONAL_TYPE,
    /// A type in parentheses: `(Int)`.
    PAREN_TYPE,
    /// `(Int, String)`.
    TUPLE_TYPE,
    /// `(Int) -> Int`.
    FN_TYPE,
    /// `mut Type`, only in a parameter's type (7.2).
    MUT_TYPE,

    // Patterns (grammar.md, 8)
    /// `_`.
    WILDCARD_PAT,
    /// A new name: `mm`, `title`.
    BIND_PAT,
    /// A literal: `6`, `-1`, `"add"`, `'a'`, `true`, `none`.
    LITERAL_PAT,
    /// `1..=5`, `'a'..='z'`.
    RANGE_PAT,
    /// An enum option, bare or qualified, with an optional payload: `Rainy(mm)`.
    OPTION_PAT,
    /// The `( )` of an option pattern.
    PAYLOAD_LIST,
    /// A named payload field: `width: w`.
    PAYLOAD_FIELD,
    /// A pattern in parentheses: `(A or B)`.
    PAREN_PAT,
    /// `(i, task)`.
    TUPLE_PAT,
    /// `["add", title]`.
    LIST_PAT,
    /// `6 or 7`.
    OR_PAT,
}

use SyntaxKind::*;

/// The 40 reserved keywords with their kinds, in the order of spec/lexical.md, section 6.1.
const KEYWORDS: [(&str, SyntaxKind); 40] = [
    ("and", AND_KW),
    ("assert", ASSERT_KW),
    ("break", BREAK_KW),
    ("catch", CATCH_KW),
    ("comptime", COMPTIME_KW),
    ("continue", CONTINUE_KW),
    ("defer", DEFER_KW),
    ("div", DIV_KW),
    ("else", ELSE_KW),
    ("enum", ENUM_KW),
    ("extend", EXTEND_KW),
    ("false", FALSE_KW),
    ("fn", FN_KW),
    ("for", FOR_KW),
    ("group", GROUP_KW),
    ("if", IF_KW),
    ("import", IMPORT_KW),
    ("in", IN_KW),
    ("is", IS_KW),
    ("let", LET_KW),
    ("match", MATCH_KW),
    ("mut", MUT_KW),
    ("none", NONE_KW),
    ("not", NOT_KW),
    ("or", OR_KW),
    ("pub", PUB_KW),
    ("repeat", REPEAT_KW),
    ("return", RETURN_KW),
    ("self", SELF_KW),
    ("spawn", SPAWN_KW),
    ("static", STATIC_KW),
    ("struct", STRUCT_KW),
    ("throw", THROW_KW),
    ("throws", THROWS_KW),
    ("trait", TRAIT_KW),
    ("true", TRUE_KW),
    ("try", TRY_KW),
    ("unsafe", UNSAFE_KW),
    ("var", VAR_KW),
    ("while", WHILE_KW),
];

/// The contextual keywords (spec/lexical.md, section 6.2). The lexer produces identifiers for
/// them; the parser recognizes them by spelling in the one position where each is a keyword.
pub const CONTEXTUAL_KEYWORDS: [&str; 5] = ["any", "type", "parallel", "test", "c"];

/// Whether `text` is one of the contextual keywords `any`, `type`, `parallel`, `test` or `c`.
pub fn is_contextual_keyword(text: &str) -> bool {
    CONTEXTUAL_KEYWORDS.contains(&text)
}

impl SyntaxKind {
    /// The kind with this raw number, if there is one.
    pub fn from_raw(raw: u16) -> Option<SyntaxKind> {
        SyntaxKind::ALL.get(usize::from(raw)).copied()
    }

    /// The reserved keyword spelled `text`, if it is one. Contextual keywords are not included.
    pub fn from_keyword(text: &str) -> Option<SyntaxKind> {
        let kind = match text {
            "and" => AND_KW,
            "assert" => ASSERT_KW,
            "break" => BREAK_KW,
            "catch" => CATCH_KW,
            "comptime" => COMPTIME_KW,
            "continue" => CONTINUE_KW,
            "defer" => DEFER_KW,
            "div" => DIV_KW,
            "else" => ELSE_KW,
            "enum" => ENUM_KW,
            "extend" => EXTEND_KW,
            "false" => FALSE_KW,
            "fn" => FN_KW,
            "for" => FOR_KW,
            "group" => GROUP_KW,
            "if" => IF_KW,
            "import" => IMPORT_KW,
            "in" => IN_KW,
            "is" => IS_KW,
            "let" => LET_KW,
            "match" => MATCH_KW,
            "mut" => MUT_KW,
            "none" => NONE_KW,
            "not" => NOT_KW,
            "or" => OR_KW,
            "pub" => PUB_KW,
            "repeat" => REPEAT_KW,
            "return" => RETURN_KW,
            "self" => SELF_KW,
            "spawn" => SPAWN_KW,
            "static" => STATIC_KW,
            "struct" => STRUCT_KW,
            "throw" => THROW_KW,
            "throws" => THROWS_KW,
            "trait" => TRAIT_KW,
            "true" => TRUE_KW,
            "try" => TRY_KW,
            "unsafe" => UNSAFE_KW,
            "var" => VAR_KW,
            "while" => WHILE_KW,
            _ => return None,
        };
        Some(kind)
    }

    /// Whether the grammar never sees this kind: whitespace, removed line breaks, comments, a
    /// byte order mark, `;` (reported, and counted as a line break) and skipped error text.
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            WHITESPACE
                | LINE_BREAK
                | LINE_COMMENT
                | DOC_COMMENT
                | BLOCK_COMMENT
                | BYTE_ORDER_MARK
                | SEMICOLON
                | ERROR
        )
    }

    /// Whether this is one of the 40 reserved keywords.
    pub fn is_keyword(self) -> bool {
        (AND_KW..=WHILE_KW).contains(&self)
    }

    /// Whether this is the kind of a syntax tree node (`FILE` to `OR_PAT`). `ERROR` is both a
    /// token kind and a node kind, and returns `false` here.
    pub fn is_node(self) -> bool {
        (FILE..=OR_PAT).contains(&self)
    }

    /// Whether this is an identifier (`UPPER_IDENT` or `LOWER_IDENT`).
    pub fn is_ident(self) -> bool {
        matches!(self, UPPER_IDENT | LOWER_IDENT)
    }

    /// Whether this is one of the tokens that make up a string literal.
    pub fn is_string_piece(self) -> bool {
        (STRING_START..=HOLE_END).contains(&self)
    }

    /// Whether this kind exists only to give a helpful error (spec/lexical.md, section 11).
    pub fn is_error_token(self) -> bool {
        (AMP_AMP..=DOT_DOT_DOT).contains(&self) || matches!(self, SEMICOLON | ERROR)
    }

    /// The kind the parser should treat this token as. The lexer has already reported these
    /// tokens with a fix; recovering this way lets parsing continue as if the fix were applied:
    /// `&&` acts as `and`, `||` as `or`, `!` as `not`, and `..` and `...` as `..<`.
    /// Every other kind is returned unchanged.
    pub fn recovered(self) -> SyntaxKind {
        match self {
            AMP_AMP => AND_KW,
            PIPE_PIPE => OR_KW,
            BANG => NOT_KW,
            DOT_DOT | DOT_DOT_DOT => DOT_DOT_LT,
            kind => kind,
        }
    }

    /// Whether this is `(`, `[` or `{`.
    pub fn is_opening_bracket(self) -> bool {
        matches!(self, L_PAREN | L_BRACK | L_BRACE)
    }

    /// Whether this is `)`, `]` or `}`.
    pub fn is_closing_bracket(self) -> bool {
        matches!(self, R_PAREN | R_BRACK | R_BRACE)
    }

    /// The closing kind that matches an opening one: `(` and `)`, `[` and `]`, `{` and `}`,
    /// a hole's `{` and `}`, and the start and end of a string.
    pub fn closing_partner(self) -> Option<SyntaxKind> {
        match self {
            L_PAREN => Some(R_PAREN),
            L_BRACK => Some(R_BRACK),
            L_BRACE => Some(R_BRACE),
            HOLE_START => Some(HOLE_END),
            STRING_START => Some(STRING_END),
            _ => None,
        }
    }

    /// The continue-after set (spec/lexical.md, section 12.3): a line that ends with one of
    /// these tokens continues on the next line. Call it on the [`recovered`](Self::recovered)
    /// kind. A keyword that directly follows `.` or `?.` is a member name and never continues
    /// a line; the lexer checks that, this method does not.
    pub fn continues_after(self) -> bool {
        matches!(
            self,
            // arithmetic
            PLUS | MINUS | STAR | SLASH | PERCENT | DIV_KW
            // comparison (not `>`, see `newline_left_to_parser`)
            | EQ_EQ | BANG_EQ | LT | LT_EQ | GT_EQ
            // bitwise (not `>>`)
            | AMP | PIPE | CARET | TILDE | LT_LT
            // logic
            | AND_KW | OR_KW | NOT_KW
            // other operators
            | DOT_DOT_LT | DOT_DOT_EQ | QUESTION_QUESTION | IS_KW | CATCH_KW
            // prefix keywords
            | TRY_KW | SPAWN_KW
            // assignment
            | EQ | PLUS_EQ | MINUS_EQ | STAR_EQ | SLASH_EQ | PERCENT_EQ
            | AMP_EQ | PIPE_EQ | CARET_EQ | LT_LT_EQ | GT_GT_EQ
            // member access
            | DOT | QUESTION_DOT
            // arrows
            | FAT_ARROW | THIN_ARROW
            // separators
            | COMMA
            // opening brackets
            | L_PAREN | L_BRACK | L_BRACE
        )
    }

    /// The continue-before set (spec/lexical.md, section 12.4): a line that starts with one of
    /// these tokens continues the line above. Call it on the [`recovered`](Self::recovered) kind.
    pub fn continues_before(self) -> bool {
        matches!(self, DOT | QUESTION_DOT | ELSE_KW | CATCH_KW | QUESTION_QUESTION | AND_KW | OR_KW)
    }

    /// `>` and `>>` (spec/lexical.md, section 12.5). The lexer keeps a `NEWLINE` after these,
    /// because a line often ends with a generic type. When the parser uses one of them as a
    /// binary operator (a comparison or a shift), it skips a `NEWLINE` that directly follows.
    pub fn newline_left_to_parser(self) -> bool {
        matches!(self, GT | GT_GT)
    }

    /// The fixed spelling of a keyword, operator or punctuation token, for messages such as
    /// "expected `)`". `None` for kinds whose text varies.
    pub fn fixed_text(self) -> Option<&'static str> {
        if self.is_keyword() {
            return KEYWORDS.iter().find(|(_, kind)| *kind == self).map(|&(text, _)| text);
        }
        let text = match self {
            UNDERSCORE => "_",
            HOLE_START => "{",
            HOLE_END => "}",
            SEMICOLON => ";",
            PLUS => "+",
            MINUS => "-",
            STAR => "*",
            SLASH => "/",
            PERCENT => "%",
            EQ_EQ => "==",
            BANG_EQ => "!=",
            LT => "<",
            LT_EQ => "<=",
            GT => ">",
            GT_EQ => ">=",
            AMP => "&",
            PIPE => "|",
            CARET => "^",
            TILDE => "~",
            LT_LT => "<<",
            GT_GT => ">>",
            EQ => "=",
            PLUS_EQ => "+=",
            MINUS_EQ => "-=",
            STAR_EQ => "*=",
            SLASH_EQ => "/=",
            PERCENT_EQ => "%=",
            AMP_EQ => "&=",
            PIPE_EQ => "|=",
            CARET_EQ => "^=",
            LT_LT_EQ => "<<=",
            GT_GT_EQ => ">>=",
            DOT_DOT_LT => "..<",
            DOT_DOT_EQ => "..=",
            DOT => ".",
            QUESTION_DOT => "?.",
            QUESTION_QUESTION => "??",
            QUESTION => "?",
            FAT_ARROW => "=>",
            THIN_ARROW => "->",
            AT => "@",
            COMMA => ",",
            COLON => ":",
            L_PAREN => "(",
            R_PAREN => ")",
            L_BRACK => "[",
            R_BRACK => "]",
            L_BRACE => "{",
            R_BRACE => "}",
            AMP_AMP => "&&",
            PIPE_PIPE => "||",
            BANG => "!",
            DOT_DOT => "..",
            DOT_DOT_DOT => "...",
            _ => return None,
        };
        Some(text)
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> u16 {
        kind as u16
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind as u16)
    }
}

/// The Quell language for rowan: it ties rowan's raw kinds to [`SyntaxKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuellLanguage {}

impl rowan::Language for QuellLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        // Every tree is built by this crate from `SyntaxKind`s, so every raw kind is known.
        SyntaxKind::from_raw(raw.0).unwrap_or(ERROR)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// A node of a Quell syntax tree.
pub type SyntaxNode = rowan::SyntaxNode<QuellLanguage>;
/// A token of a Quell syntax tree.
pub type SyntaxToken = rowan::SyntaxToken<QuellLanguage>;
/// A node or a token of a Quell syntax tree.
pub type SyntaxElement = rowan::SyntaxElement<QuellLanguage>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_numbers_round_trip() {
        for (index, &kind) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(kind as usize, index);
            assert_eq!(SyntaxKind::from_raw(kind as u16), Some(kind));
        }
        assert_eq!(SyntaxKind::from_raw(u16::MAX), None);
    }

    #[test]
    fn node_kinds_come_after_token_kinds() {
        assert!(FILE.is_node() && OR_PAT.is_node() && BINARY_EXPR.is_node());
        assert!(!ERROR.is_node() && !WHILE_KW.is_node() && !EOF.is_node());
        for &kind in SyntaxKind::ALL {
            if kind.is_node() {
                assert!(!kind.is_keyword() && !kind.is_trivia() && !kind.is_string_piece());
                assert_eq!(kind.fixed_text(), None);
            }
        }
        use rowan::Language;
        assert_eq!(QuellLanguage::kind_from_raw(QuellLanguage::kind_to_raw(MATCH_ARM)), MATCH_ARM);
        assert_eq!(QuellLanguage::kind_from_raw(rowan::SyntaxKind(u16::MAX)), ERROR);
    }

    #[test]
    fn keywords() {
        assert_eq!(KEYWORDS.len(), 40);
        for (text, kind) in KEYWORDS {
            assert!(kind.is_keyword(), "{text}");
            assert_eq!(SyntaxKind::from_keyword(text), Some(kind));
            assert_eq!(kind.fixed_text(), Some(text));
        }
        let keyword_count = SyntaxKind::ALL.iter().filter(|kind| kind.is_keyword()).count();
        assert_eq!(keyword_count, 40);
        for word in CONTEXTUAL_KEYWORDS {
            assert_eq!(SyntaxKind::from_keyword(word), None);
            assert!(is_contextual_keyword(word));
        }
        assert_eq!(SyntaxKind::from_keyword("Self"), None);
    }

    #[test]
    fn recovered_kinds() {
        assert_eq!(AMP_AMP.recovered(), AND_KW);
        assert_eq!(PIPE_PIPE.recovered(), OR_KW);
        assert_eq!(BANG.recovered(), NOT_KW);
        assert_eq!(DOT_DOT.recovered(), DOT_DOT_LT);
        assert_eq!(DOT_DOT_DOT.recovered(), DOT_DOT_LT);
        assert_eq!(PLUS.recovered(), PLUS);
    }

    #[test]
    fn newline_sets_match_the_spec() {
        // `>` and `>>` are left to the parser; `>=` and `>>=` continue.
        assert!(!GT.continues_after() && !GT_GT.continues_after());
        assert!(GT_EQ.continues_after() && GT_GT_EQ.continues_after());
        assert!(GT.newline_left_to_parser() && GT_GT.newline_left_to_parser());
        // `?`, `)` and the jumps never continue a line.
        for kind in [QUESTION, R_PAREN, R_BRACK, R_BRACE, RETURN_KW, THROW_KW, BREAK_KW] {
            assert!(!kind.continues_after(), "{kind:?}");
        }
        // Nothing in the continue-before set except `catch`, `and` and `or` continues after.
        assert!(!ELSE_KW.continues_after());
        assert!(!MINUS.continues_before());
    }
}
