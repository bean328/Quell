# Quell lexical structure (v0.1 draft)

This chapter says how Quell source text is cut into tokens. It covers characters, whitespace, comments, names, keywords, literals, operators, and the newline rules that decide where a statement ends.

The grammar in [grammar.md](grammar.md) is written in terms of the tokens defined here. The notation is explained in [README.md](README.md#how-to-read-the-grammar).

Contents:

1. [Overview: from text to tokens](#1-overview-from-text-to-tokens)
2. [Source text](#2-source-text)
3. [Whitespace and line breaks](#3-whitespace-and-line-breaks)
4. [Comments](#4-comments)
5. [Identifiers and the naming rule](#5-identifiers-and-the-naming-rule)
6. [Keywords](#6-keywords)
7. [Number literals](#7-number-literals)
8. [Character literals](#8-character-literals)
9. [String literals](#9-string-literals)
10. [Operators and punctuation](#10-operators-and-punctuation)
11. [Tokens that exist only to give a helpful error](#11-tokens-that-exist-only-to-give-a-helpful-error)
12. [The newline rules](#12-the-newline-rules)
13. [Summary of token kinds](#13-summary-of-token-kinds)

---

## 1. Overview: from text to tokens

Lexing happens in two steps.

1. **Scanning.** The lexer reads the source from start to end and produces *raw tokens*. It always takes the longest token it can (this is called "maximal munch"); the few exceptions are listed where they apply. Whitespace, line breaks and comments are kept as *trivia* (see [decision 0005](../docs/decisions/0005-front-end-architecture.md): the syntax tree keeps every character), but trivia is never seen by the grammar.
2. **Newline marking.** Every token remembers whether at least one line break came before it (since the previous token). The [newline rules](#12-the-newline-rules) then decide, for each such line break, whether it is *significant*. A significant line break becomes a `NEWLINE` token in the stream the parser reads. All other line breaks stay trivia.

The parser therefore sees a stream of tokens in which `NEWLINE` appears only where a statement (or a member, or a `match` arm) may end. The stream always ends with one `EOF` token.

When the lexer finds a mistake, it reports it, produces the most likely intended token (for example `and` for `&&`), and keeps going. The program does not compile, but the user sees all mistakes at once.

---

## 2. Source text

- A Quell source file is a sequence of Unicode characters encoded as **UTF-8**. A file that is not valid UTF-8 is an error, reported at the first bad byte.
- A UTF-8 byte order mark (U+FEFF) at the very start of a file is ignored. Anywhere else it is an error.
- The character U+0000 (NUL) is an error anywhere, even inside comments and strings.
- Other control characters (U+0001 to U+001F and U+007F, except tab U+0009, line feed U+000A and carriage return U+000D) are errors outside comments. Inside strings they must be written with escapes.
- Source files use the extension `.ql`.

---

## 3. Whitespace and line breaks

- **Whitespace** is the space (U+0020) and the tab (U+0009). It separates tokens and has no other meaning. Any other Unicode space character outside a comment or string is an error with a fix ("this is a non-breaking space; use a normal space").
- A **line break** is either a line feed (LF, U+000A) or a carriage return followed by a line feed (CR LF). Both count as one line break. A carriage return that is not followed by a line feed is an error.
- Several line breaks in a row, and lines that hold only whitespace or comments, count the same as a single line break. Blank lines never change meaning.

Indentation has no meaning in Quell. Blocks are always marked with `{ }`. The one place where indentation matters is the trimming of [multi-line strings](#95-multi-line-strings).

**Spaces between tokens never change how code parses.** Only line breaks do. When this spec says one token "directly follows" another, or says "the next token", it means the next token in the stream, with or without spaces between them. So `Stack <String>()` is the same as `Stack<String>()`, and the pattern `- 1` is the same as `-1`. The formatter normalizes the spacing. The one exception is the `count++` and `count--` check in [grammar.md, section 11](grammar.md#11-rejected-syntax), which looks for two `+` or two `-` characters written next to each other.

---

## 4. Comments

Comments are trivia: the parser never sees them. There are three kinds.

### 4.1 Line comments

`//` starts a comment that runs to the end of the line. The line break that ends it is a normal line break.

```quell
let total = price * 3    // this is a comment
```

### 4.2 Block comments

`/*` starts a block comment and `*/` ends it. Block comments **nest**: every `/*` inside a block comment needs its own `*/`. This makes it safe to comment out code that already contains block comments.

```quell
/* outer /* inner */ still a comment */
```

- An unclosed block comment is an error, reported at the `/*` that was never closed.
- Text inside a block comment is not lexed. Quotes inside it have no special meaning.
- A `*/` outside a comment is an error ("`*/` without a matching `/*`").
- **Line breaks:** a block comment that contains at least one line break counts as a line break for the newline rules. A block comment on one line counts as whitespace. So `let a = 1 /* note */ let b = 2` is an error (two statements on one line), while a multi-line comment between two statements separates them.

### 4.3 Doc comments

A comment that starts with **exactly three** slashes, `///`, is a doc comment. It runs to the end of the line. `////` (four or more slashes) is an ordinary line comment.

- Doc comments are trivia like other comments, so they never change how code parses. The parser attaches each run of consecutive doc comment lines to the element that starts right after them: an item, a struct field, a method, an enum option or a trait member.
- A doc comment that is not followed by one of those elements (for example, before a statement inside a function body) is reported as a warning: "this doc comment doesn't document anything".
- The text of a doc comment is not lexed as Quell. Code examples inside it are extracted and run by `quell test` (PLAN 6.3, [examples/21_library](../examples/21_library/src/lib.ql)).

**Code examples in doc comments.** In one run of consecutive doc comment lines, a **fence** is a line whose text (after `///` and any spaces) starts with ```` ``` ````. The lines between an opening fence and the next fence are one code example. Its source text is made like this:

1. From each line, remove `///` and at most one space after it.
2. Join the lines with line breaks.

The result is lexed and parsed as a separate file with the `File` rule ([grammar.md, 2.1](grammar.md#21-file-structure)), in the **entry-file role** ([grammar.md, 2.2](grammar.md#22-entry-files-and-library-files)). So top-level statements such as `assert Color.from_hex("orange") == none` are valid, and they run in order as one test. Errors are reported at their place in the original file. Which names the example can see (for example `Color` and `mix` from the file it documents) is decided by name resolution, not by this spec. An opening fence with no closing fence is an error.

---

## 5. Identifiers and the naming rule

### 5.1 Syntax

```
IDENT          = IdentStart { IdentContinue } ;
IdentStart     = a character with the Unicode property XID_Start, or "_" ;
IdentContinue  = a character with the Unicode property XID_Continue ;
```

- This is the default identifier syntax of Unicode Standard Annex #31. ASCII letters, digits and `_` are included, so `snake_case`, `PascalCase`, `rp2040` and `sqlite3_libversion` are all identifiers.
- Identifiers must be in Unicode Normalization Form C (NFC). An identifier that is not is an error with a fix that shows the normalized spelling.
- An identifier cannot start with a digit (`2fast` is a number followed by a name, which is an error; see [7.5](#75-what-may-follow-a-number)).
- A lone `_` is **not** an identifier. It is the wildcard token `_` (used in patterns and for unused lambda parameters). `_x` and `__` are identifiers.
- Reserved keywords (section 6) are not identifiers. Contextual keywords are.
- All keywords are ASCII, so a keyword can never be spelled with look-alike characters by accident.

### 5.2 Upper and lower names

Every identifier is in exactly one of two classes, decided **only by its first character**:

- **UPPER_IDENT**: the first character is an uppercase or titlecase letter (Unicode general category Lu or Lt). Examples: `Player`, `Rainy`, `T`, `Int`, `UInt8`.
- **LOWER_IDENT**: every other identifier. The first character is a lowercase letter, a letter from a script without case, or `_`. Examples: `score`, `take_damage`, `_unused`, `c`.

The grammar uses `IDENT` when either class is allowed, and `UPPER_IDENT` or `LOWER_IDENT` when only one is.

A name that starts with a letter from a script without case (for example Chinese or Japanese) is always a `LOWER_IDENT`. Such a name works for variables, functions and fields, but it can't be a type or an enum option name, because those must start with an uppercase letter (5.3). This follows from the settled naming rule and is listed as an [open question](README.md#open-questions).

### 5.3 The naming rule

Decision 0003 sets the naming style:

- **snake_case** for values: variables, constants, parameters, fields, functions and methods (`max_users`, `take_damage`).
- **PascalCase** for types and enum options (`Player`, `AgeError`, `NotANumber`).

Two parts of this rule have different status:

1. **The first letter is part of the syntax.** The parser checks it wherever a name is *declared* and reports an error with a fix if it is wrong. The table below lists every declaration site.
2. **The rest of the style** (no `camelCase`, no `SCREAMING_CASE` for Quell names) is not a syntax rule. It is checked by a later stage. Whether that check is an error or a warning is an [open question](README.md#open-questions).

| Declaration site | First letter must be | Example |
|---|---|---|
| `struct`, `enum`, `trait` name | upper | `struct Player` |
| enum option | upper | `Rainy(mm: Float)` |
| generic parameter | upper | `fn largest<T: Compare>` |
| parameter whose type is `type` | upper | `comptime fn field_names(T: type)` |
| function, method | lower | `fn take_damage` |
| other parameter, lambda parameter | lower | `amount: Int`, `m => m.year` |
| struct field, enum option field | lower | `health: Int`, `Rainy(mm: Float)` |
| `let`, `var`, `for` and pattern bindings | lower | `let (low, high) = ...` |
| `catch` binding | lower | `catch err { ... }` |

A parameter of type `type` holds a type, so it is named like a type (example 27 writes `T: type`). This follows the naming rule rather than breaking it.

Names that come from outside Quell (for example C functions and types reached through `c.` after `import c "header.h"`) are never declared in Quell source, so no check applies to them.

### 5.4 What the parser relies on

Besides the declaration checks, the parser uses the upper/lower class in exactly two places. Nowhere else does capitalization change how code parses.

1. **Patterns.** In a pattern, a bare (undotted) name is read by its class:
   - an `UPPER_IDENT` is always an **enum option** that already exists (`Sunny`, `Rainy(mm)`), as decision 0003 says;
   - a `LOWER_IDENT` is always a **new binding** that captures the matched value (`mm`, `title`).

   A dotted name in a pattern (`Weather.Rainy`, `Int.max`) is always a reference, whatever its case. Whether the name really is an enum option is checked by name resolution. Taking a struct apart in a pattern (`Point(x, y)`) is not in v0.1 (an [open question](README.md#open-questions)). A bare `LOWER_IDENT` followed by `(` is an error ("a new name can't have `( )`; option names start with a capital letter"). Because of this rule, a pattern never needs to know what names are in scope to be parsed, and `Sunny =>` can never accidentally mean "bind anything to a variable called Sunny". To compare against an existing lowercase value, use a guard: `x if x == limit => ...`.

2. **The `catch` binding form.** `expr catch name { ... }` binds the error only when `name` is a `LOWER_IDENT` (or `_`). An `UPPER_IDENT` after `catch` is a fallback value, such as an enum option (`load() catch Empty`). See [grammar.md, catch](grammar.md#65-catch).

In expressions, the parser does **not** look at capitalization. `Player(name: "Ava")`, `Sunny`, `Int.max` and `print(x)` are all plain names, calls and member accesses. Deciding whether `Player(...)` builds a struct or calls a function is done later, by name resolution.

---

## 6. Keywords

### 6.1 Reserved keywords

These 40 words are **reserved**: they can never be declared as names (variables, functions, parameters, fields, types and so on).

There is one place where a reserved word is read as a name: **right after `.` or `?.`** in an expression ([grammar.md, 6.10](grammar.md#610-postfix-operators-and-calls)). Only a name can follow `.` there, so this is never ambiguous. It lets Quell code reach names that come from outside Quell and happen to be keywords, such as the C function `div` from `stdlib.h` (`c.div(7, 2)`). The lexer still produces the keyword token; the parser accepts it as a member name. Declaring a Quell field or method with a keyword name is not possible in v0.1 (an [open question](README.md#open-questions)).

| Keyword | Used for |
|---|---|
| `and` | logical and; joins generic requirements and trait conformances |
| `assert` | assert statement |
| `break` | leave a loop |
| `catch` | handle an error |
| `comptime` | compile-time `let`, `fn` and blocks |
| `continue` | next round of a loop |
| `defer` | run a block when the current block ends |
| `div` | whole-number division |
| `else` | other branch of `if` |
| `enum` | enum declaration |
| `extend` | add methods to an existing type |
| `false` | boolean literal |
| `fn` | function declaration |
| `for` | loop over items; also `parallel for` |
| `group` | structured task group |
| `if` | condition; also match guards |
| `import` | import declaration |
| `in` | separates the loop variable from the items in `for` |
| `is` | type test |
| `let` | fixed variable |
| `match` | pattern matching |
| `mut` | mutating method, mutable parameter, mutable argument, mutating lambda |
| `none` | the missing value |
| `not` | logical not |
| `or` | logical or; alternatives in patterns |
| `pub` | public visibility |
| `repeat` | counted or endless loop |
| `return` | leave a function early |
| `self` | the current value inside a method |
| `spawn` | start a task |
| `static` | method that belongs to the type |
| `struct` | struct declaration |
| `throw` | fail with an error |
| `throws` | marks a function that can fail |
| `trait` | trait declaration |
| `true` | boolean literal |
| `try` | pass an error up |
| `unsafe` | unsafe block |
| `var` | changeable variable |
| `while` | loop while a condition holds |

`self` is reserved in lowercase only. `Self` is an ordinary identifier in v0.1.

### 6.2 Contextual keywords

These 5 words are ordinary identifiers everywhere except in one specific position, where they act as keywords. In that position the identifier reading could never be valid, so making them contextual costs nothing and keeps useful names available.

| Word | Keyword only when | Why it is contextual |
|---|---|---|
| `any` | it starts a type and the next token is an identifier: `List<any Shape>` | `any` is a common method name (`items.any(x => x > 3)`). In type position a lowercase name can never be a user type (types are PascalCase), so there is no clash. |
| `type` | it starts a type and the next token is not `.` or `<`: `T: type`, `type?` | `type` is a very common field and variable name (`token.type`). In type position a lowercase name can never be a user type. |
| `parallel` | it starts an expression and is directly followed by `for` (no line break between) | An identifier is never directly followed by the keyword `for`, so `parallel for` cannot mean anything else. `parallel` stays usable as a name, for example for a setting. |
| `test` | it starts a top-level item and is directly followed by a string literal: `test "name" { }` | An identifier is never directly followed by a string literal. `test` is a natural variable name in testing code (`let test = load_case()`). |
| `c` | it comes right after `import` and is followed by a string literal: `import c "sqlite3.h"` | `c` is an extremely common short name, and example 28 uses it as a value (`c.sqlite3_libversion()`). `import c` followed by a line break is an ordinary import of a module named `c`. |

The lexer always produces `IDENT` for these words. The parser recognizes them by spelling in the positions above.

### 6.3 Why the others are reserved

Some reserved words could in theory have been contextual. They are reserved because the identifier reading would be ambiguous somewhere:

- `group`, `defer`, `unsafe`, `comptime` and `spawn` can be followed by `{`. If `group` were a name, `if group { ... }` could be either "if the variable `group` is true" or "if (the result of a `group` block)".
- `assert` can be followed by `(`: `assert (a == b)` would look like a call to a function named `assert`.
- `is`, `div`, `and`, `or`, `catch` and `in` sit between two expressions, so they must never be names.
- `self`, `true`, `false` and `none` are values.

No other words are reserved in v0.1. Reserving more words later breaks programs that use them as names, so whether to reserve some words now for future features is an [open question](README.md#open-questions).

### 6.4 Names that look like keywords but are not

These are ordinary identifiers defined by the standard library or the compiler, not keywords: `Int`, `Float`, `Bool`, `String`, `Char`, `List`, `Map`, `Never`, `Ok`, `Err`, `print`, `exit`, `channel`, `derive`, `entry`. Shadowing them is a matter for name resolution, not for the parser.

---

## 7. Number literals

Number literals are always non-negative. In `-5`, the `-` is a separate token (the negation operator, or the sign of a [literal pattern](grammar.md#8-patterns)).

### 7.1 Integer literals

```
INT          = DecimalInt | HexInt | BinaryInt | OctalInt ;
DecimalInt   = "0" | NonZeroDigit { [ "_" ] Digit } ;
HexInt       = "0x" HexDigit { [ "_" ] HexDigit } ;
BinaryInt    = "0b" BinDigit { [ "_" ] BinDigit } ;
OctalInt     = "0o" OctDigit { [ "_" ] OctDigit } ;
Digit        = "0" to "9" ;
NonZeroDigit = "1" to "9" ;
HexDigit     = "0" to "9" | "a" to "f" | "A" to "F" ;
BinDigit     = "0" | "1" ;
OctDigit     = "0" to "7" ;
```

- **Underscores** make long numbers readable (`1_000_000`). Each underscore must sit between two digits: `1__000`, `1_`, `0x_ff` and `_1` are not allowed (`_1` is an identifier).
- **Prefixes** are lowercase: `0x`, `0b`, `0o`. `0X10` is an error with the fix `0x10`. Hex digits may be either case (`0xFF`, `0xff`).
- **Leading zeros** in decimal numbers are an error: `007` gets the message "numbers can't start with 0; for an octal number write `0o7`". This prevents the C trap where `010` means 8.
- A prefix with no digits (`0x`) or a digit that doesn't fit the base (`0b102`) is an error.
- The lexer stores the exact value with no size limit. Whether the value fits its type (`Int`, `UInt8`, ...) is checked later.
- **Negative literals.** Number literals are never negative, so `-9223372036854775808` (the smallest `Int`) is the prefix `-` applied to `9223372036854775808`, which on its own does not fit in `Int`. Rule: when a prefix `-` is applied directly to an `INT` or `FLOAT` literal (`-` followed by the literal, with no parentheses), the later range check uses the negated value. This matches negative literal patterns ([grammar.md, 8.1](grammar.md#81-pattern-syntax)), where `-` and the number are read as one value.

### 7.2 Float literals

```
FLOAT         = IntPart "." DecimalDigits [ Exponent ]
              | IntPart Exponent ;
IntPart       = "0" | NonZeroDigit { [ "_" ] Digit } ;
DecimalDigits = Digit { [ "_" ] Digit } ;
Exponent      = ( "e" | "E" ) [ "+" | "-" ] DecimalDigits ;
```

- A float has **digits on both sides of the dot**: `1.52`, `0.5`, `8.0`. `.5` and `5.` are not floats.
- An exponent is allowed with or without a dot: `2.5e-3`, `1e9`, `6.02E23`. A number with an exponent is always a float.
- The part before the dot follows the integer rule for leading zeros: `0.5` is fine, `00.5` is an error. The part after the dot may have any digits (`1.05`).
- There are no hex, binary or octal floats.

### 7.3 The dot rule

After the digits of a decimal number, the lexer looks at the next two characters:

- If they are `.` followed by a **decimal digit**, the dot belongs to the number, which becomes a float.
- Otherwise the number ends before the dot.

This one rule makes all of these lex correctly:

| Source | Tokens | Meaning |
|---|---|---|
| `1.52` | `FLOAT(1.52)` | a float |
| `4.is_even()` | `INT(4)` `.` `is_even` `(` `)` | a method call on the integer 4 |
| `0..<3` | `INT(0)` `..<` `INT(3)` | a range |
| `1..=10` | `INT(1)` `..=` `INT(10)` | a range |
| `0.5..<1.5` | `FLOAT(0.5)` `..<` `FLOAT(1.5)` | a range of floats |
| `1.5.round()` | `FLOAT(1.5)` `.` `round` `(` `)` | a method call on a float |
| `2.e` | `INT(2)` `.` `e` | member `e` of 2 (an error later, but it lexes) |

An exponent is recognized only when `e` or `E` is followed by a digit, or by `+` or `-` and then a digit. In every other case the `e` would start a name directly after a number, which is an error (next section).

### 7.4 Booleans and none

`true`, `false` and `none` are keywords, not identifiers. They are listed here because they are literals in the grammar.

### 7.5 What may follow a number

A number literal must not be immediately followed (with no space between) by an identifier character or another digit-like character. `3px`, `10ms` and `0x1g` are errors: "a number can't be directly followed by a letter". Quell has no number suffixes; the type comes from context or a conversion such as `UInt8(255)`.

---

## 8. Character literals

```
CHAR      = "'" ( CharChar | Escape ) "'" ;
CharChar  = any character except "'", "\" and a line break ;
Escape    = "\" ( "n" | "r" | "t" | "0" | "\" | '"' | "'" )
          | "\u{" HexDigit [ HexDigit ] [ HexDigit ] [ HexDigit ] [ HexDigit ] [ HexDigit ] "}" ;
```

- A character literal holds exactly **one Unicode scalar value**: `'A'`, `'é'`, `'\n'`, `'\''`.
- `''` is an error ("a character literal needs exactly one character").
- More than one character is an error with a fix: `'ab'` gets the message: single quotes hold one character; for text, use double quotes, as in `"ab"`.
- The escapes are the same as in strings (section 9.2). `\"` is allowed but not needed in a character literal.
- Whether a character made of several scalar values (such as an emoji with a skin tone) can be a `Char` is a question for the type system, not the lexer. The lexer accepts exactly one scalar value.

---

## 9. String literals

Quell has three kinds of string literal. All of them produce a `STRING` token.

| Kind | Delimiters | Escapes | Interpolation | Lines |
|---|---|---|---|---|
| normal | `"..."` | yes | yes | one |
| multi-line | `"""` ... `"""` | yes | yes | many |
| raw | `#"..."#`, `##"..."##`, ... and `#"""` ... `"""#` | no | no | one, or many with `"""` |

A `STRING` token is a sequence of **segments**. Each segment is either *text* (the characters after escapes have been processed) or a *hole* (an interpolated expression, with an optional format spec). A string with no holes is called a **plain string**. Raw strings are always plain. The grammar requires a plain string in a few places (test names, `import c`, patterns).

### 9.1 Normal strings

```
NormalString = '"' { StringChar | Escape | "{{" | "}}" | Hole } '"' ;
StringChar   = any character except '"', "\", "{", "}" and a line break ;
Hole         = "{" HoleTokens [ ":" FormatSpec ] "}" ;
HoleTokens   = the tokens of one expression, ended by the depth rule of 9.3 ;
```

**Order of the alternatives.** In string text (outside a hole), the lexer checks for `{{` and `}}` **before** it checks for a hole. So `{{` is always a literal `{`, and a hole can never start with a second `{` written right after the first. This removes the two readings of `"{{name}}"`: it is always the text `{name}`, never a hole. Inside a hole the lexer reads ordinary tokens, so there `}}` is simply two `}` tokens.

A normal string starts with `"` and ends at the next `"` that is not escaped and not inside a hole. It cannot contain a raw line break; use `\n`. An unclosed string is an error reported at its opening quote, and the string is assumed to end at the end of the line.

### 9.2 Escapes

| Escape | Meaning |
|---|---|
| `\n` | line feed |
| `\r` | carriage return |
| `\t` | tab |
| `\0` | NUL character |
| `\\` | backslash |
| `\"` | double quote |
| `\'` | single quote |
| `\u{X...}` | the Unicode scalar value with hex code `X...` (1 to 6 hex digits, for example `\u{E9}` or `\u{1F600}`) |

- Any other character after `\` is an error that names the valid escapes.
- `\u{...}` must name a Unicode scalar value: surrogates (D800 to DFFF) and values above 10FFFF are errors.
- There is no `\{` escape. A literal brace is written by doubling it (next section). `\{` is an error with that fix.

### 9.3 Interpolation

Inside normal and multi-line strings, `{` starts a **hole**: an expression whose value is written into the string.

```quell
"{name} is {age} years old"
"2 + 2 = {2 + 2}"
"{greet(name: "Ava")}"           // a string inside a hole is fine
"Total: {total:.2}"              // a format spec
"#{self.r:02x}{self.g:02x}"      // `#` before `{` is just text
```

**Literal braces.** In string text, `{{` is a literal `{` and `}}` is a literal `}`, and they are checked before holes (9.1). A single `}` that does not close a hole is an error with the fix "write `}}` for a literal brace". So `"Write {{name}} to insert a value"` is the text `Write {name} to insert a value`.

Because `{{` is always a literal brace, a hole whose expression starts with `{` (a block) needs a space after the hole's own `{`: `"{ {x} }"` is a hole, while `"{{x}}"` is the text `{x}`. Likewise `"{{ if ok { 1 } else { 2 } }}"` is plain text. The formatter keeps the space in `"{ {x} }"`.

**How the lexer finds the end of a hole.** After the opening `{`, the lexer reads ordinary tokens (with all the usual rules, including nested strings and character literals), and keeps a depth counter:

- `(`, `[` and `{` add one to the depth; `)`, `]` and `}` subtract one.
- At **depth 0**, a `:` token ends the expression and starts the **format spec** (section 9.4).
- At **depth 0**, a `}` ends the hole.

This is the rule from decision 0004: "the format spec starts at the first `:` that isn't inside parentheses or brackets". It is extended to count braces too, because a `{` among the hole's tokens (after the hole's own opening `{`) always opens a block (for example `"{if ok { "yes" } else { "no" }}"`), so a colon inside it can't be a format spec either. Colons elsewhere in Quell expressions (named arguments, map entries, typed lambda parameters) only occur inside `( )` or `[ ]`, so the rule never cuts an expression short:

| Hole | Expression | Format spec |
|---|---|---|
| `{price:.2}` | `price` | `.2` |
| `{self.area():.1}` | `self.area()` | `.1` |
| `{greet(name: "Ava")}` | `greet(name: "Ava")` | none |
| `{["a": 1]["a"] ?? 0}` | `["a": 1]["a"] ?? 0` | none |

More rules for holes:

- A hole must contain an expression. `{}` and `{ }` are errors ("empty `{}`; write `{{}}` for literal braces"). A hole with only a format spec (`{:.2}`) is also an error.
- A hole cannot contain a line break, even in a multi-line string. Inside a hole, the [newline rules](#12-the-newline-rules) treat the hole like a pair of parentheses.
- **A hole that is never closed.** If the end of the line comes before the `}` that would close a hole, the hole is unclosed. The lexer reports the error at the hole's `{`: "this `{` starts a value inside the string, but it is never closed. For a literal brace write `{{`. For JSON, use a raw string such as `#"{"a": 1}"#`." It then goes back to just after that `{` and reads the rest of the line again as string text. So in `print("Use { to start a block")` the user gets this one message, and the string and the call still end where they appear to.
- **A likely JSON mistake.** If a format spec is invalid (9.4) and the hole's expression is a single string literal, as in `"{"a": 1}"`, the error also gets the raw-string hint above.
- Comments are not allowed inside a hole. `//` or `/*` inside a hole is an error ("comments can't go inside `{ }` in a string").
- The tokens of the hole are kept inside the `STRING` token. The parser parses them as exactly one expression ([grammar.md, 6.13](grammar.md#613-string-literals-and-holes)).

### 9.4 Format specs

The format spec is the text between the depth-0 `:` and the `}` that closes the hole. It is not made of tokens; it is checked by this small grammar, and an invalid spec is an error at lexing time:

```
FormatSpec = [ Width ] [ "." Precision ] [ Kind ] ;
Width      = [ "0" ] NonZeroDigit { Digit } ;
Precision  = Digit { Digit } ;
Kind       = "x" | "X" | "b" | "o" ;
```

- The spec must not be empty (`{x:}` is an error) and must not contain spaces.
- **Width** is the minimum number of characters. Shorter output is padded on the left with spaces, or with zeros when the width starts with `0`.
- **Precision** is the number of digits after the decimal point.
- **Kind** writes an integer as lowercase hex (`x`), uppercase hex (`X`), binary (`b`) or octal (`o`).

The specs used in the examples:

| Spec | Parts | Example | Result |
|---|---|---|---|
| `.2` | precision 2 | `"{14.97:.2}"`, `"{4.99 * 3:.2}"` | `14.97` |
| `.1` | precision 1 | `"{3.14159:.1}"` | `3.1` |
| `02x` | zero-padded width 2, lowercase hex | `"{136:02x}"`, `"{0:02x}"` | `88`, `00` |

Which specs a value's type accepts (for example, a precision on a `String`) is checked later, not by the lexer. Alignment, signs, scientific notation and widths taken from variables are not in v0.1 (see [open questions](README.md#open-questions)).

### 9.5 Multi-line strings

A multi-line string starts with `"""` and ends with `"""`.

```quell
let poem = """
    Roses are red,
    Quell is fast.
    """
```

The exact rules:

1. The opening `"""` must be followed by a line break. Only spaces and tabs may come between them. The first line of content is the line after the opening delimiter.
2. The closing `"""` must be on its own line, with only spaces and tabs before it. Those spaces and tabs are the **indentation prefix**.
3. Every content line must start with exactly the indentation prefix (the same characters, tabs and spaces matched one by one). The prefix is removed. A content line that has fewer leading characters than the prefix, or different ones, is an error pointing at that line, with the message: this line is less indented than the closing `"""`.
4. A content line that holds only spaces and tabs (or nothing) never causes the error of rule 3. If it starts with the prefix, the prefix is removed as usual; otherwise it becomes an empty line.
5. The line break right before the closing line is not part of the string. All other line breaks in the content become `\n`, even if the file uses CR LF.
6. Spaces at the end of content lines are kept.
7. Escapes and holes work as in normal strings. A hole must still fit on one line.
8. `"` and `""` can appear in the content. `"""` cannot; write `\"""`.
9. The whole multi-line string is **one token**. The line breaks inside it belong to the token, so the [newline rules](#12-the-newline-rules) never see them. The same is true of multi-line raw strings.

So the example above is the string `"Roses are red,\nQuell is fast."`, and `print(poem)` prints two lines, as in [example 09](../examples/09_strings.ql).

### 9.6 Raw strings

A raw string has no escapes and no holes: every character between the delimiters is part of the string. It is perfect for JSON and regular expressions.

```quell
let json = #"{"name": "Ava"}"#
let tricky = ##"a raw string can contain "# like this"##
```

- A raw string starts with one or more `#` followed by `"`. It ends at the first `"` that is followed by **the same number** of `#`. Using more `#` lets the content contain `"#`.
- A single-line raw string cannot contain a line break.
- `#"""` followed by nothing but spaces and tabs up to the end of the line starts a **multi-line raw string**, which ends with `"""#` (with the same number of `#`). If anything else follows `#"""` on the same line, it is a single-line raw string whose text starts with `""`: `#"""hi"""#` is the text `""hi""`. A normal string has no such second reading: `"""` followed by other text on the same line is the error of rule 1 in 9.5. It follows the multi-line rules of 9.5 (line break after the opening, trimming to the closing line's indentation) but has no escapes and no holes. This combines two settled features in the obvious way; Swift does the same.
- Outside a string, `#` is only valid when it starts a raw string. Any other `#` is an error.

Inside a normal string, `#` has no special meaning: `"#ff8800"` and `"#{i + 1}"` are a normal string whose text starts with `#`.

---

## 10. Operators and punctuation

### 10.1 The full list

| Token | Name | Used in |
|---|---|---|
| `+` | plus | addition, string joining |
| `-` | minus | subtraction, negation, negative literal patterns |
| `*` | star | multiplication |
| `/` | slash | exact division |
| `%` | percent | remainder |
| `==` `!=` | equal, not equal | comparison |
| `<` `<=` `>` `>=` | less, less or equal, greater, greater or equal | comparison; `<` and `>` also open and close generic arguments |
| `&` | ampersand | bitwise and |
| <code>&#124;</code> | bar | bitwise or |
| `^` | caret | bitwise exclusive or |
| `~` | tilde | bitwise not (prefix) |
| `<<` `>>` | shift left, shift right | bit shifts |
| `=` | equals | `let`, assignment, default values, `=` function bodies |
| `+=` `-=` `*=` `/=` `%=` | compound assignment | arithmetic update |
| `&=` <code>&#124;=</code> `^=` `<<=` `>>=` | compound assignment | bitwise update |
| `..<` | range up to | range that stops before the end |
| `..=` | range through | range that includes the end |
| `.` | dot | member access, paths, imports |
| `?.` | optional dot | member access that gives `none` if the value is missing |
| `??` | default | fallback for a missing value |
| `?` | question mark | optional type suffix (`String?`) only |
| `=>` | fat arrow | match arms and lambdas: "gives this value" |
| `->` | thin arrow | return types and function types: "returns this type" |
| `@` | at | attributes (`@derive(Json)`, `@entry`) |
| `,` | comma | separators |
| `:` | colon | type annotations, named arguments, map entries, conformances, format specs |
| `(` `)` | parentheses | grouping, calls, tuples, parameters |
| `[` `]` | brackets | lists, maps, indexing, list patterns |
| `{` `}` | braces | blocks, bodies, import lists |
| `_` | underscore | wildcard pattern, unused lambda parameter |

The word operators `and`, `or`, `not`, `div`, `is` and `catch` are keywords (section 6).

Bitwise not is written `~`, as in C, Python, JavaScript and Zig. Decision 0003 frees `&`, `|` and `^` for bit operations but does not name a symbol for bitwise not; `!` is not available because it is reserved for the "use `not`" error. This choice is recorded in the [README](README.md#decisions-made-in-this-spec).

### 10.2 Maximal munch and its two exceptions

The lexer always takes the longest token that matches. For example `>>=` is one token, `..<` is one token, and `a<=b` is `a` `<=` `b`.

There are two exceptions.

1. **Ranges.** After `..`, the lexer looks at the next character: `<` gives `..<` and `=` gives `..=`. Anything else is the error token for a bare `..` (section 11). `...` is also an error token.
2. **Closing generic arguments.** The lexer produces `>>`, `>=` and `>>=` as single tokens, but when the parser needs a `>` to close a list of generic arguments, it **splits** the token: it takes the first `>` and leaves the rest (`>`, `=` or `>=`) as the next token. This is how `List<List<Int>>` and `var x: List<Int>= []` work. The lexer itself never needs to know about generics. The exact rule is in [grammar.md, 7.3](grammar.md#73-closing-angle-brackets).

---

## 11. Tokens that exist only to give a helpful error

The lexer recognizes these character sequences so that it can explain the Quell way. Each one is reported as an error, and the lexer then produces the token in the "recovers as" column so parsing can continue.

| Source | Message and fix | Recovers as |
|---|---|---|
| `&&` | "Quell uses the word `and`: write `a and b`" | `and` |
| <code>&#124;&#124;</code> | "Quell uses the word `or`: write `a or b`" | `or` |
| `!` (not followed by `=`) | "Quell uses the word `not`: write `not done`" | `not` |
| `..` (not followed by `<` or `=`) | "a range must say whether it includes the end: write `0..<10` (stops before 10) or `1..=10` (includes 10)" | `..<` |
| `...` | the same message as `..` | `..<` |
| `;` | "Quell doesn't use semicolons: put each statement on its own line" | a line break |
| `*/` outside a comment | "`*/` without a matching `/*`" | nothing |
| `#` not starting a raw string | "`#` is only used for raw strings, such as `#"{"a": 1}"#`" | nothing |
| any other character that cannot start a token (for example `$` or a backtick) | "Quell doesn't use this character here" | nothing |

`&&` and `||` can never be valid Quell, because `&` and `|` are binary operators and a binary operator can never be followed by another one. The same is true of `!`, which has no meaning on its own.

Rejected syntax that is made of valid tokens (such as `if let`, a ternary `a ? b : c`, or an enum option written `.Sunny`) is reported by the parser instead. The full list is in [grammar.md, section 11](grammar.md#11-rejected-syntax).

---

## 12. The newline rules

This is the most important part of the lexical structure. In Quell, **a line break ends a statement**, unless one of a few simple rules says the statement continues. There are no semicolons.

### 12.1 Where `NEWLINE` matters

The grammar uses `NEWLINE` as the separator between:

- statements in a block and at the top level of a file,
- items at the top level,
- fields and methods in a struct, options in an enum, and members of traits and `extend` blocks,
- arms of a `match`,
- the end of a trait requirement (a function with no body).

It is also allowed in two smaller places: after an attribute that stands on its own line (`@entry` above `fn main()`), and before the `}` that closes an import list. Finally, the parser skips a `NEWLINE` that directly follows a `>` or `>>` it uses as a binary operator (12.5).

Everywhere else, a `NEWLINE` token is a syntax error. That is why the rules below remove line breaks wherever a statement clearly isn't finished.

### 12.2 The algorithm

The lexer keeps a stack of open brackets. Opening `(`, `[` or `{` pushes it; the matching closer pops it. (A closer that does not match the top of the stack is an error; the lexer reports it and pops back to the matching opener if there is one.) A hole in a string counts as a `(` while its tokens are being lexed.

The lexer also **records, for every `(`, `[` and `{`, the position of its matching closer** (or that it has none). The parser uses this to look past a pair of parentheses in one step instead of scanning it ([grammar.md, 6.1](grammar.md#61-the-top-rule)).

For each token **T** that has at least one line break before it, let **P** be the token before it (ignoring trivia). The line break before T becomes a `NEWLINE` token **unless one of these holds**:

1. **Start or end of file.** There is no P (T is the first token), or T is `EOF`. A file never starts or ends with `NEWLINE`.
2. **Inside parentheses or brackets.** The innermost open bracket is `(` or `[`. (Checked before T is processed, so a line break right before `)` or `]` is also removed.) Inside `( )` and `[ ]`, a new line never ends a statement.
3. **P says "more to come".** P is one of the tokens in the *continue-after* set below.
4. **T says "I continue the line above".** T is one of the tokens in the *continue-before* set below.

Several line breaks before one token give at most one `NEWLINE`. Line breaks *inside* a token (a multi-line string) are part of that token and are never considered. A block comment that spans lines counts as a line break before the next token (section 4.2).

In pseudocode:

```
for each token T in order:
    if T.has_line_break_before
       and P exists and T is not EOF
       and innermost_open_bracket() not in { "(", "[" }
       and P.kind not in CONTINUE_AFTER
       and T.kind not in CONTINUE_BEFORE:
        emit NEWLINE
    emit T
    update the bracket stack with T
    P = T
```

**The innermost bracket decides.** A `{` inside `( )` makes line breaks significant again until its `}`. This is what lets a lambda with a block body sit inside a call:

```quell
items.each(x => {
    print(x)          // a NEWLINE ends this line: the innermost bracket is `{`
    print(x * 2)
})
```

### 12.3 The continue-after set

A line that **ends** with one of these tokens continues on the next line:

| Group | Tokens |
|---|---|
| arithmetic | `+` `-` `*` `/` `%` `div` |
| comparison | `==` `!=` `<` `<=` `>=` (not `>`, see 12.5) |
| bitwise | `&` <code>&#124;</code> `^` `~` `<<` (not `>>`, see 12.5) |
| logic | `and` `or` `not` |
| other operators | `..<` `..=` `??` `is` `catch` |
| assignment | `=` `+=` `-=` `*=` `/=` `%=` `&=` <code>&#124;=</code> `^=` `<<=` `>>=` |
| member access | `.` `?.` |
| arrows | `=>` `->` |
| separators | `,` |
| opening brackets | `(` `[` `{` |

This is the decision 0003 rule ("a line continues if it ends with an operator or comma") made exact. Every token in the set needs something after it, so a line that ends with one can never be a finished statement. The word operators `div`, `and`, `or`, `not`, `is` and `catch` are operators too, so they are in the set. `?` (the optional type suffix in `email: String?`) is not an operator and is not in the set, so a field or annotation can end with `?`. Keywords that start an expression, such as `return`, `try`, `throw` and `spawn`, are not in the set: a line ending with `return` returns nothing.

### 12.4 The continue-before set

A line that **starts** with one of these tokens continues the line above:

| Token | Why it is safe |
|---|---|
| `.` | Decision 0003 and 0004. Since 0004 removed the `.Sunny` shorthand and floats can't start with a dot, nothing can start with `.`. |
| `?.` | The same as `.`: it continues a chain, as in `find_user(42)?.name` split over lines. |
| `else` | Nothing starts with `else`. This allows `}` and `else {` on separate lines. |
| `catch` | Nothing starts with `catch`. This allows a long call followed by `catch fallback` on the next line. |
| `??` | Nothing starts with `??`. This allows a fallback on its own line. |
| `and`, `or` | Nothing starts with these. This allows long conditions to be broken before the logic word. In a `match`, a line starting with `or` continues the pattern above, which is also what you'd want. |

None of these tokens can start a statement, an item, a member, an enum option or a match arm pattern. So treating them as continuations can never swallow a line that was meant to stand alone. This is exactly the kind of clash decision 0004 fixed for `.Sunny`; the rule "a continue-before token must never be able to start anything" is what keeps it from coming back.

Other binary operators are **not** in this set. In particular `-` can start an expression (`-1`), so a line starting with `-` is always a new statement. To keep one rule for all arithmetic, `+`, `*`, `/` and the rest also do not continue from the start of a line. Put the operator at the end of the first line instead.

### 12.5 Lines that end with a greater-than sign

`>` and `>>` are missing from the continue-after set on purpose. They also close generic arguments, and a line often ends with a generic type:

```quell
struct Stack<T> {
    items: List<T>          // this line must end here
    size: Int
}
```

If a line ending with `>` always continued, this struct would read as `items: List<T> size: Int`, which is an error. So the lexer leaves the `NEWLINE` after `>` and `>>` in place, and the **parser** handles the other case: when the parser uses a `>` or `>>` token as a *binary operator* (comparison or shift), it skips a `NEWLINE` that directly follows it. The result is that a line ending with a comparison `>` continues, exactly like the other operators, while a line ending with a generic `>` ends.

`>=` and `>>=` stay in the continue-after set. When they are split to close generic arguments (`List<Int>= []`), the leftover `=` or `>=` would continue the line anyway.

### 12.6 Worked examples

In these examples, `⏎` marks a line break that becomes a `NEWLINE` token, and `·` marks a line break that is removed.

**A method chain (example 22).**

```quell
let top_titles = movies·
    .filter(m => m.rating >= 8.0)·
    .sort_by(m => m.rating, descending: true)·
    .map(m => "{m.title} ({m.year})")⏎

for title in top_titles {·
```

Each line break before a line starting with `.` is removed by rule 4. The parser sees one statement: `let top_titles = movies.filter(...).sort_by(...).map(...)`. The line break after the last `)` is kept, so the `let` ends there. The blank line counts as nothing. The line break after `{` is removed by rule 3.

A leading `.` attaches to whatever comes before it, exactly as if it were on the same line. So:

```quell
let f = x => x * 2·
    .to_string()
```

means `let f = x => x * 2.to_string()`, where `.to_string()` applies to `2`. To call a method on a whole expression, wrap it in parentheses. The same happens after any operator:

```quell
let name = user.nickname ?? user.name·
    .upper()
```

means `user.nickname ?? (user.name.upper())`, so a nickname is never made uppercase. Both programs are valid, so the parser gives a **warning**: when a `.` or `?.` that starts a line attaches to an operand that is the right side of a binary operator (including `??` and `catch`), the operand of a prefix operator, the body of a lambda or the value of a jump, it says "this `.upper()` applies only to `user.name`. To apply it to the whole expression, wrap the expression in parentheses." To keep the meaning and silence the warning, put the right side in parentheses: `user.nickname ?? (user.name` and `.upper())` on the next line. A plain chain such as `movies` followed by `.filter(...)` lines never gets the warning, because its left side is a postfix chain, not the right side of an operator.

**A multi-line list with a trailing comma (examples 13 and 22).**

```quell
let movies = [·
    Movie(title: "Spirited Away", year: 2001, rating: 8.6),·
    Movie(title: "Up", year: 2009, rating: 8.3),·
]⏎
```

Inside `[ ]` every line break is removed (rule 2). The trailing comma before `]` is allowed by the grammar. The line break after `]` ends the statement.

**A multi-line call (example 21).**

```quell
pub fn mix(a: Color, b: Color) -> Color = Color(·
    r: average(a.r, b.r),·
    g: average(a.g, b.g),·
    b: average(a.b, b.b),·
)⏎
```

**An import list split over lines.**

```quell
import http.{·
    Server,·
    Request,·
    Response⏎
}⏎
```

The `{` of an import list is a brace, so the innermost bracket is `{` and line breaks are significant inside it. The breaks after `{` and after each comma are removed by rule 3. The break before `}` stays, and the import grammar allows it ([grammar.md, 3.9](grammar.md#39-imports)). A trailing comma after `Response` would remove it instead; both are fine.

**A struct with fields on separate lines and on one line.**

```quell
struct Movie {·
    title: String⏎
    year: Int, rating: Float⏎
}⏎
```

Each `NEWLINE` separates two members, and so does the comma. The `NEWLINE` before `}` is an allowed trailing separator.

**A line ending with an operator.**

```quell
let total = first_price +       // `+` is in the continue-after set ·
    second_price⏎
```

A comment after the operator does not change anything: comments are trivia, so P is still `+`.

**The trap: an operator at the start of a line.**

```quell
let total = first_price⏎
    - discount
```

This is **two** statements: `let total = first_price` and the expression `-discount`. The fix is to end the first line with `-`.

This is a trap even when it type-checks. If the line starting with `-` is the **last line of a block**, it becomes the block's value:

```quell
fn score() -> Int {·
    compute_base(x)⏎
        - penalty⏎
}
```

Here the function returns `-penalty`. Because the newline rule is settled and `-y` alone on a last line is valid code, this is not an error. Instead the parser gives a **warning**, decided from tokens alone: a statement that starts with a prefix `-` or `~`, when the statement before it in the same block (or file) is an expression statement, a `let` or `var`, or an assignment, gets "this line starts with `-`, so it is a new statement. To continue the line above, end that line with `-`. To make a negative value here on purpose, write `(-penalty)`." Writing the parentheses silences the warning. `match` arms are not statements, so an arm such as `-1 => "minus one"` never gets it.

**`if` and `else` on separate lines.**

```quell
if temperature > 30 {·
    print("It's hot!")⏎
}·
else {·
    print("Stay inside.")⏎
}⏎
```

The line break before `else` is removed (rule 4). `} else {` on one line is the usual style, but both work.

**A long condition.**

```quell
if temperature > 15·
    and not raining {·
```

The line break before `and` is removed (rule 4). Breaking after `and` works too, by rule 3.

**Fallbacks on their own line.**

```quell
let name = find_user(42)·
    ?.name·
    ?? "guest"⏎
let n = parse_age(text)·
    catch 0⏎
```

**Generic types and optionals at the end of a line.**

```quell
struct User {·
    name: String⏎
    email: String?        // `?` is not an operator: the field ends ⏎
    tags: List<String>    // a generic `>`: the field ends ⏎
}⏎
```

**A comparison `>` at the end of a line.**

```quell
if score >⏎
    best {
```

The lexer keeps the `NEWLINE` after `>`, but the parser uses this `>` as a comparison and skips the `NEWLINE` (12.5). The condition is `score > best`.

**`return` at the end of a line.**

```quell
if done {·
    return⏎
    cleanup()⏎
}
```

`return` is not in the continue-after set, so this returns nothing, and `cleanup()` is a separate statement (a later stage warns that it can never run). This is true outside `( )` and `[ ]` only. Inside them line breaks are removed (rule 2), so a `return` at the end of a line inside a call takes the next line as its value ([grammar.md, 6.19](grammar.md#619-jump-expressions)).

**Match arms, including a negative number.**

```quell
match n {·
    0 => "zero"⏎
    -1 => "minus one"⏎
    1..=9 => "small"⏎
    _ => "big"⏎
}
```

Each arm ends with a `NEWLINE`. The arm starting with `-` is safe because `-` is not in the continue-before set.

**A block comment that spans lines.**

```quell
let a = 1 /* a comment
             that spans two lines */ let b = 2
```

The block comment contains a line break, so it counts as one (section 4.2). The `NEWLINE` before `let b` is kept, and these are two statements. This is legal but the formatter would split them.

**A brace on the next line.**

```quell
if ready⏎
{
```

The line break after `ready` is significant, so the parser reports: "the `{` that opens this block must be on the same line as the `if`". The same applies to every block (functions, loops, `match`, structs). This keeps the rule simple and makes the style uniform.

Inside `( )` and `[ ]` the line break is removed (rule 2), so the newline rules alone would accept `(if ready` with `{ 1 } else { 2 })` on the next line. To keep one rule everywhere, the parser also checks the line-break flag that every token carries: a `{` that opens the **body of a keyword construct** must not have a line break before it, inside brackets or not. The constructs are `if`, `else`, `for`, `parallel for`, `while`, `repeat`, `match`, `fn`, `struct`, `enum`, `trait`, `extend`, `test`, `defer`, `group`, `unsafe`, `spawn`, `comptime` and the binding form of `catch`. A block that is just a value (for example after `=` or `=>`) may start on the next line where the newline rules allow it.

---

## 13. Summary of token kinds

The parser receives these kinds of tokens:

| Kind | Examples |
|---|---|
| `IDENT` (split into `UPPER_IDENT` and `LOWER_IDENT`) | `score`, `Player`, `T`, `any` |
| reserved keywords | `fn`, `let`, `match`, ... (section 6.1) |
| `INT` | `0`, `1_000_000`, `0xff`, `0b1010`, `0o17` |
| `FLOAT` | `1.52`, `8.0`, `2.5e-3`, `1e9` |
| `CHAR` | `'A'`, `'\n'` |
| `STRING` | `"Hi {name}"`, `"""..."""`, `#"..."#` |
| operators and punctuation | section 10.1 |
| `_` | the wildcard |
| `NEWLINE` | a significant line break (section 12) |
| `EOF` | end of file |

Comments, doc comments, whitespace and removed line breaks are trivia. They are kept in the lossless syntax tree but are invisible to the grammar.
