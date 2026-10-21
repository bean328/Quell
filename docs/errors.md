# Error codes

Every error and warning the Quell compiler reports has a stable code, such as `Q0034`. This page lists them all. A code always means the same problem: codes are never reused, and a code that is no longer used stays in the table, marked as retired.

| Range | Reported by |
|---|---|
| `Q0001` to `Q0099` | the lexer (errors) |
| `Q0100` to `Q0399` | the parser (errors) |
| `Q0400` to `Q0499` | the parser (warnings) |

Sections refer to the syntax specification: `lexical.md` is [spec/lexical.md](../spec/lexical.md) and `grammar.md` is [spec/grammar.md](../spec/grammar.md).

## Lexer errors

The codes are defined in `compiler/quell_syntax/src/lexer/codes.rs`. Keep the two lists in sync; a test checks that every code there is listed here.

| Code | Title | Spec section |
|---|---|---|
| `Q0001` | The file is not valid UTF-8 | lexical.md 2 |
| `Q0002` | A byte order mark that is not at the very start of the file | lexical.md 2 |
| `Q0003` | A NUL character (U+0000), anywhere | lexical.md 2 |
| `Q0004` | A control character outside comments and strings | lexical.md 2 |
| `Q0005` | A carriage return that is not followed by a line feed | lexical.md 3 |
| `Q0006` | A space character that is not a space or a tab (such as a non-breaking space) | lexical.md 3 |
| `Q0007` | A block comment that is never closed | lexical.md 4.2 |
| `Q0008` | A code block in a doc comment that is never closed | lexical.md 4.3 |
| `Q0009` | A name that is not in Unicode Normalization Form C | lexical.md 5.1 |
| `Q0010` | An uppercase number prefix (`0X10`) | lexical.md 7.1 |
| `Q0011` | A number that starts with `0` (`007`) | lexical.md 7.1, 7.2 |
| `Q0012` | A number prefix with no digits (`0x`) | lexical.md 7.1 |
| `Q0013` | A digit that doesn't fit the base (`0b102`) | lexical.md 7.1 |
| `Q0014` | An underscore that is not between two digits (`1__000`, `1_`) | lexical.md 7.1, 7.2 |
| `Q0015` | A number directly followed by a letter (`3px`) | lexical.md 7.5 |
| `Q0020` | An empty character literal (`''`) | lexical.md 8 |
| `Q0021` | A character literal with more than one character (`'ab'`) | lexical.md 8 |
| `Q0022` | A character literal that is never closed | lexical.md 8 |
| `Q0030` | An escape that doesn't exist (`\q`) | lexical.md 9.2 |
| `Q0031` | The escape `\{` or `\}`, which doesn't exist | lexical.md 9.2 |
| `Q0032` | A badly written `\u{...}` escape | lexical.md 9.2 |
| `Q0033` | A `\u{...}` escape that is not a Unicode scalar value | lexical.md 9.2 |
| `Q0034` | A string that is never closed | lexical.md 9.1, 9.5, 9.6 |
| `Q0035` | A control character written directly in a string or character literal | lexical.md 2 |
| `Q0036` | A single `}` in string text | lexical.md 9.3 |
| `Q0037` | An empty hole (`{}` or `{ }`) in a string | lexical.md 9.3 |
| `Q0038` | A hole with a format spec but no value (`{:.2}`) | lexical.md 9.3 |
| `Q0039` | A hole that is never closed | lexical.md 9.3 |
| `Q0040` | A comment inside a hole | lexical.md 9.3 |
| `Q0041` | An invalid format spec | lexical.md 9.4 |
| `Q0042` | Text on the same line after an opening `"""` | lexical.md 9.5 |
| `Q0043` | A closing `"""` that is not on its own line | lexical.md 9.5 |
| `Q0044` | A line of a multi-line string that is less indented than the closing `"""` | lexical.md 9.5 |
| `Q0050` | `&&` instead of `and` | lexical.md 11 |
| `Q0051` | <code>&#124;&#124;</code> instead of `or` | lexical.md 11 |
| `Q0052` | `!` instead of `not` | lexical.md 11 |
| `Q0053` | A range written `..` or `...` | lexical.md 10.2, 11 |
| `Q0054` | A semicolon | lexical.md 11 |
| `Q0055` | `*/` outside a comment | lexical.md 4.2, 11 |
| `Q0056` | `#` that doesn't start a raw string | lexical.md 9.6, 11 |
| `Q0057` | A character that can't start any token (such as `$` or a backtick) | lexical.md 11 |
| `Q0058` | A character that looks like an ASCII one but isn't (such as a curly quote) | lexical.md 11 |
| `Q0060` | A closing bracket that doesn't close anything | lexical.md 12.2 |
| `Q0061` | An opening bracket that is never closed | lexical.md 12.2 |
| `Q0070` | A file of 4 GiB or more | lexical.md 2 |

## Parser errors and warnings

None yet. The parser adds its codes here, errors from `Q0100` and warnings from `Q0400`.
