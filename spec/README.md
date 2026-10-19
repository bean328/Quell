# Quell language specification

This folder is the specification of the Quell programming language. It says exactly which programs are valid Quell and how they are read.

**Version 0.1, draft. Syntax only.**

| File | What it covers |
|---|---|
| [README.md](README.md) | This page: status, scope, how to read the grammar, the index, the choices this spec made and the open questions |
| [lexical.md](lexical.md) | Lexical structure: characters, comments, names, keywords, literals, operators and the newline rules |
| [grammar.md](grammar.md) | The grammar: files, items, statements, expressions, types, patterns, ambiguities and rejected syntax |

---

## Status

- **Version 0.1, draft (October 2026).** Quell is in Phase 0 (design on paper). There is no compiler yet. This spec is what the Phase 1 lexer and parser will be built from ([decision 0005](../docs/decisions/0005-front-end-architecture.md)).
- **Syntax only.** The spec covers lexing (text to tokens) and parsing (tokens to a syntax tree). It does not cover type checking, name resolution, ownership, compile-time evaluation or what programs do when they run. Those come in later versions. Where a rule has a meaning that a later stage checks, the text says "checked later" and stops there.
- **It implements settled decisions.** Every syntax choice in [decision 0003](../docs/decisions/0003-syntax.md) and its revision [decision 0004](../docs/decisions/0004-syntax-review.md) is settled. This spec makes those choices exact. It does not change them. If this spec and a decision ever disagree, the decision wins and the spec has a bug. Changing settled syntax needs a new decision record.
- **The examples are the test.** Every program in [examples/](../examples/README.md) (29 examples, 31 `.ql` files) must be derivable from this grammar with no errors. [grammar.md, section 12](grammar.md#12-example-coverage) maps each file to the rules it uses. For v0.1, this was checked by hand and with a small throwaway parser written from these files alone. In Phase 1 the real parser takes over this check. Comment lines in the examples that show code which "is a compile error" are comments, not program text.

### Who this is for

- **Compiler engineers** should be able to write a lexer and a recursive-descent parser from `lexical.md` and `grammar.md` alone. The most important parts are [the newline rules](lexical.md#12-the-newline-rules), [the precedence table](grammar.md#5-precedence-and-associativity) and [the list of ambiguities](grammar.md#10-ambiguities-and-how-they-are-resolved).
- **Everyone else** (beginners, teachers, library authors) can read the plain-English notes after each rule and skip the rules themselves. The notes say the same thing in words.

### What is normative

- The grammar rules, the token definitions and the plain-English notes are the specification.
- Code examples, "why" explanations and the error message texts are there to help. Error messages are shown in a short form; their final wording belongs to the compiler's diagnostics.

---

## How to read the grammar

The spec uses one EBNF notation in both files. `lexical.md` uses it for **lexical rules**, which describe characters. `grammar.md` uses it for **syntax rules**, which describe tokens.

### Notation

| Notation | Meaning | Example |
|---|---|---|
| `Rule = ... ;` | defines the rule named `Rule` | `WhileStmt = "while" Expr Block ;` |
| `"fn"`, `"=>"`, `"{"` | exactly this text: a keyword, operator or punctuation token in syntax rules, or these characters in lexical rules | `"repeat"` |
| `'"'` | the double quote character (single quotes are used only to write this one character) | `NormalString = '"' ... '"' ;` |
| `"any"`, `"type"`, `"parallel"`, `"test"`, `"c"` | a **contextual keyword**: an `IDENT` token spelled exactly like this ([lexical.md, 6.2](lexical.md#62-contextual-keywords)) | `"parallel" "for"` |
| `IDENT`, `INT`, `STRING`, `NEWLINE`, ... | a **token kind** (written in capitals) from [lexical.md, 13](lexical.md#13-summary-of-token-kinds) and [grammar.md, 1.2](grammar.md#12-token-classes-used-in-the-grammar) | `Field = LOWER_IDENT ":" Type ;` |
| `A B` | `A` followed by `B` | `"defer" Block` |
| <code>A &#124; B</code> | `A` or `B` | <code>"let" &#124; "var"</code> |
| `[ A ]` | `A` is optional (zero or one time) | `[ "pub" ]` |
| `{ A }` | `A` repeated zero or more times | `{ "," Param }` |
| `( A B )` | grouping | <code>( "+" &#124; "-" )</code> |
| `"0" to "9"` | any one character in this range (lexical rules only) | `Digit = "0" to "9" ;` |
| plain English | a set of characters that is easier to say in words (lexical rules only) | `any character except "'", "\" and a line break` |

More details:

- **Sequence binds tighter than `|`.** `A B | C` means `( A B ) | C`.
- **Rule names** are written in `PascalCase` (`IfExpr`, `MatchArm`). Token kinds are written in capitals (`IDENT`, `NEWLINE`).
- **Braces and brackets in quotes are tokens.** `"{" Block "}"` uses the brace token; `{ A }` without quotes is repetition.
- **Whitespace and comments never appear in syntax rules.** They are trivia, removed before parsing. `NEWLINE` appears only where the grammar allows a line break to end something ([grammar.md, 1.3](grammar.md#13-newlines-in-the-grammar)). Lexical rules describe characters with nothing allowed between them.
- **Declaration names.** `UPPER_IDENT` and `LOWER_IDENT` at a place that declares a name are checked loosely: the parser accepts any `IDENT` and reports a naming error with a fix ([grammar.md, 1.2](grammar.md#12-token-classes-used-in-the-grammar)).

### How the rules are meant to be used

The syntax rules are written for a hand-written **recursive-descent** parser with **precedence climbing** (Pratt parsing) for expressions, as decision 0005 requires. Each rule can be turned into one parsing function.

- In almost every rule, the next token (sometimes the next two) decides which alternative to take. The note after the rule says how.
- Two places look further ahead, and the notes explain exactly how far: recognizing a lambda with parameters in parentheses ([grammar.md, 6.1](grammar.md#61-the-top-rule)) and telling generic arguments from less-than ([grammar.md, 6.11](grammar.md#611-generic-arguments-in-expressions)).
- Where the rules alone allow two readings, [grammar.md, section 10](grammar.md#10-ambiguities-and-how-they-are-resolved) gives the deciding rule. A parser must follow those rules.
- The binary operator rules in [grammar.md, 6.2 to 6.8](grammar.md#62-logic-or-and-not) are written as a chain of rules, one per precedence level. They describe the same language as the [precedence table](grammar.md#5-precedence-and-associativity), which is the easier form to implement.

### Reading a rule: an example

```
RepeatStmt    = "repeat" [ Expr ] Block ;
```

Read it as: the keyword `repeat`, then optionally an expression (the count), then a block. The note after the rule says what decides whether the count is there: if the token after `repeat` is `{`, it always starts the block. So `repeat 3 { ... }` repeats three times and `repeat { ... }` repeats forever.

### Markers in examples

In the worked examples of [lexical.md, 12.6](lexical.md#126-worked-examples), `⏎` marks a line break that becomes a `NEWLINE` token and `·` marks a line break that is removed. They are not part of Quell.

---

## Index

### [lexical.md](lexical.md): from text to tokens

1. [Overview: from text to tokens](lexical.md#1-overview-from-text-to-tokens)
2. [Source text](lexical.md#2-source-text): UTF-8, forbidden characters
3. [Whitespace and line breaks](lexical.md#3-whitespace-and-line-breaks)
4. [Comments](lexical.md#4-comments): `//`, nestable `/* */`, doc comments `///`
5. [Identifiers and the naming rule](lexical.md#5-identifiers-and-the-naming-rule): upper and lower names, [what the parser relies on](lexical.md#54-what-the-parser-relies-on)
6. [Keywords](lexical.md#6-keywords): [reserved](lexical.md#61-reserved-keywords) and [contextual](lexical.md#62-contextual-keywords)
7. [Number literals](lexical.md#7-number-literals): integers, floats, [the dot rule](lexical.md#73-the-dot-rule)
8. [Character literals](lexical.md#8-character-literals)
9. [String literals](lexical.md#9-string-literals): [escapes](lexical.md#92-escapes), [interpolation](lexical.md#93-interpolation), [format specs](lexical.md#94-format-specs), [multi-line](lexical.md#95-multi-line-strings), [raw](lexical.md#96-raw-strings)
10. [Operators and punctuation](lexical.md#10-operators-and-punctuation)
11. [Tokens that exist only to give a helpful error](lexical.md#11-tokens-that-exist-only-to-give-a-helpful-error): `&&`, `||`, `!`, `..`, `;`
12. [The newline rules](lexical.md#12-the-newline-rules): [the algorithm](lexical.md#122-the-algorithm), [continue-after](lexical.md#123-the-continue-after-set), [continue-before](lexical.md#124-the-continue-before-set), [lines ending in `>`](lexical.md#125-lines-that-end-with-a-greater-than-sign), [worked examples](lexical.md#126-worked-examples)
13. [Summary of token kinds](lexical.md#13-summary-of-token-kinds)

### [grammar.md](grammar.md): from tokens to a syntax tree

1. [Conventions](grammar.md#1-conventions)
2. [Files](grammar.md#2-files): entry files and library files
3. [Items](grammar.md#3-items): [attributes](grammar.md#31-items-attributes-and-doc-comments), [visibility](grammar.md#32-visibility), [functions](grammar.md#33-functions), [generic parameters](grammar.md#34-generic-parameters), [structs](grammar.md#35-structs), [enums](grammar.md#36-enums), [traits](grammar.md#37-traits), [extend](grammar.md#38-extend), [imports](grammar.md#39-imports), [tests](grammar.md#310-tests), [comptime](grammar.md#311-compile-time-items), [top-level let](grammar.md#312-top-level-let-and-var)
4. [Blocks and statements](grammar.md#4-blocks-and-statements): [block values](grammar.md#41-blocks-and-their-value), `let`, assignment, `for`, `while`, `repeat`, `defer`, `assert`, [`if x = 5`](grammar.md#411-if-x--5-is-a-compile-error)
5. [Precedence and associativity](grammar.md#5-precedence-and-associativity)
6. [Expressions](grammar.md#6-expressions): [catch](grammar.md#65-catch), [??](grammar.md#66-defaults-), [calls](grammar.md#610-postfix-operators-and-calls), [generic arguments](grammar.md#611-generic-arguments-in-expressions), [lists and maps](grammar.md#612-primary-expressions), [if](grammar.md#615-if), [match](grammar.md#616-match), [concurrency](grammar.md#617-concurrency-and-unsafe-expressions), [lambdas](grammar.md#618-lambdas), [jumps](grammar.md#619-jump-expressions)
7. [Types](grammar.md#7-types)
8. [Patterns](grammar.md#8-patterns)
9. [Errors the parser reports](grammar.md#9-errors-the-parser-reports)
10. [Ambiguities and how they are resolved](grammar.md#10-ambiguities-and-how-they-are-resolved)
11. [Rejected syntax](grammar.md#11-rejected-syntax)
12. [Example coverage](grammar.md#12-example-coverage)

---

## Decisions made in this spec

Decisions 0003 and 0004 settle the look of Quell, but a grammar has to be exact in places they don't mention. These are the choices this spec made to fill those gaps. Each one fits the settled decisions and the examples. Any of them can still change before v1.0; a change to one of them is a spec change, not a new language decision.

### Lexing

1. **Bitwise not is `~`** ([lexical.md, 10.1](lexical.md#101-the-full-list)). Decision 0003 frees `&`, `|` and `^` for bit operations but names no symbol for bitwise not. `!` is taken by the "use `not`" error, and `~` is what C, Python, JavaScript and Zig use.
2. **Reserved and contextual keywords** ([lexical.md, 6](lexical.md#6-keywords)). 40 words are reserved. `any`, `type`, `parallel`, `test` and `c` are contextual, because each is a useful name and is a keyword in only one position, where a name could never appear.
3. **Number literals** ([lexical.md, 7](lexical.md#7-number-literals)): lowercase prefixes `0x`, `0b`, `0o`; underscores only between digits; leading zeros (`007`) are an error, avoiding C's octal trap; no number suffixes; a float needs digits on both sides of the dot.
4. **Escapes** ([lexical.md, 9.2](lexical.md#92-escapes)): `\n \r \t \0 \\ \" \'` and `\u{...}`. There is no `\{`; a literal brace is `{{`, as decision 0003 says.
5. **Format specs** ([lexical.md, 9.4](lexical.md#94-format-specs)): a small mini-language, `[width][.precision][kind]`, that covers `.2`, `.1` and `02x` from the examples and nothing more.
6. **Holes count braces too** ([lexical.md, 9.3](lexical.md#93-interpolation)). Decision 0004 says the format spec starts at the first `:` outside parentheses and brackets. This spec also skips a `:` inside braces. A brace inside a hole always opens a block, and a hole has no line breaks, so such a block holds one statement. No valid program reads differently because of this extension; it only turns some confusing errors into working code.
7. **Holes are one line and have no comments** ([lexical.md, 9.3](lexical.md#93-interpolation)).
8. **Multi-line strings** ([lexical.md, 9.5](lexical.md#95-multi-line-strings)): the exact trimming rules, and multi-line raw strings `#"""` ... `"""#`, which combine two settled features the way Swift does.
9. **Doc comments are exactly three slashes** ([lexical.md, 4.3](lexical.md#43-doc-comments)); `////` is an ordinary comment.
10. **A block comment that spans lines counts as a line break** ([lexical.md, 4.2](lexical.md#42-block-comments)).

### Newlines

11. **The innermost bracket decides** ([lexical.md, 12.2](lexical.md#122-the-algorithm)). Decision 0004 says new lines never end a statement inside `( )` and `[ ]`. A lambda with a block body inside a call (`items.each(x => { ... })`) needs line breaks to end statements inside the block. So a `{` inside parentheses makes line breaks significant again until its `}`.
12. **The continue-after set** ([lexical.md, 12.3](lexical.md#123-the-continue-after-set)) lists every operator, including the word operators `div`, `and`, `or`, `not`, `is` and `catch`, plus `,`, `=`, `=>`, `->`, `.`, `?.` and opening brackets.
13. **The continue-before set** ([lexical.md, 12.4](lexical.md#124-the-continue-before-set)) is `.`, `?.`, `else`, `catch`, `??`, `and` and `or`. Decision 0003 names only `.`. The others were added because none of them can start a statement, so they can never swallow a line that was meant to stand alone. That is the lesson of the `.Sunny` clash fixed in decision 0004.
14. **A line ending in `>` or `>>`** ([lexical.md, 12.5](lexical.md#125-lines-that-end-with-a-greater-than-sign)). These tokens are left out of the continue-after set because a line often ends with a generic type (`items: List<T>`). Instead, the parser skips the line break after a `>` or `>>` that it uses as an operator. The effect is the one decision 0003 asks for: a line ending with an operator continues.
15. **The `{` of a block must be on the same line** as the code before it ([lexical.md, 12.6](lexical.md#126-worked-examples)). This follows from the newline rules and keeps the style uniform.

### Grammar

16. **The precedence table** ([grammar.md, 5](grammar.md#5-precedence-and-associativity)): bitwise operators bind tighter than comparisons (avoiding C's `a & mask == 0` trap); `not` binds looser than comparisons, as in Python; `??` and `catch` sit between ranges and comparisons; `try` and `spawn` cover one postfix chain.
17. **Comparisons, ranges and `is` don't chain.** `a < b < c` is an error with a fix.
18. **Lambdas and jumps only as a whole expression** ([grammar.md, 6.1](grammar.md#61-the-top-rule)). A jump may also be the right side of `??` or `catch`, as decision 0004 requires.
19. **Generic arguments versus less-than** ([grammar.md, 6.11](grammar.md#611-generic-arguments-in-expressions)): a speculative parse with three conditions, in the style of Swift and C#.
20. **The `catch` rule** ([grammar.md, 6.5](grammar.md#65-catch)): `catch` followed by a lowercase name (or `_`) and `{` is the binding form; anything else is a fallback.
21. **`if x = 5` is reported by the parser** ([grammar.md, 4.11](grammar.md#411-if-x--5-is-a-compile-error)), with the fix to write `==`.
22. **Commas may separate `match` arms**, as well as line breaks ([grammar.md, 6.16](grammar.md#616-match)). This mirrors struct fields and is needed for a `match` inside a string hole, which can't contain line breaks.
23. **Several conformances are joined with `and`** (`struct Dog: Describe and Compare`), like several generic requirements ([grammar.md, 3.5](grammar.md#35-structs)).
24. **Enums may contain methods** ([grammar.md, 3.6](grammar.md#36-enums)), like structs.
25. **Enum option fields are always named**, and an option with no fields has no parentheses ([grammar.md, 3.6](grammar.md#36-enums)).
26. **Payload patterns may name fields** (`Rect(width: w, height: _)`), mirroring how values are built ([grammar.md, 8.1](grammar.md#81-pattern-syntax)).
27. **A top-level `let` or `var` is always an item** ([grammar.md, 2.1](grammar.md#21-file-structure)), so entry files and library files parse the same way. The parser is told the file's role and reports top-level statements in library files.
28. **Assignment is a statement**, not an expression, and `let`/`var` always need a starting value ([grammar.md, 4.3 and 4.4](grammar.md#44-assignment)).
29. **`spawn` takes a block or a call** ([grammar.md, 6.9](grammar.md#69-prefix-operators)).
30. **`return` takes a value only if one starts on the same line** ([grammar.md, 6.19](grammar.md#619-jump-expressions)).
31. **`test`, `comptime { }` and `comptime let` are top-level items only**, and no items are declared inside blocks ([grammar.md, 3.10 and 3.11](grammar.md#311-compile-time-items)).
32. **A parameter of type `type` has an uppercase name** (`T: type`, from example 27), because it holds a type ([lexical.md, 5.3](lexical.md#53-the-naming-rule)). For the same reason a named argument may be uppercase (`field_names(T: Point)`).
33. **A type can be optional only once** (`Int??` is an error) ([grammar.md, 7.1](grammar.md#71-type-syntax)).
34. **Extra helpful errors** for habits from other languages: `|` between patterns, struct literals with braces, `::` paths and `++`/`--` ([grammar.md, 11](grammar.md#11-rejected-syntax)).

---

## Open questions

These are left open on purpose. v0.1 either rejects the syntax or leaves it out, so deciding later can't break existing programs. Item 1 is different: it comes from the settled decisions themselves.

1. **Where `mut` goes in lambdas and in function parameters.** The settled decisions write a changeable function parameter as `counter: mut Counter` (`mut` in the type) and a changeable lambda parameter as `mut entries => ...` (`mut` before the name). A typed lambda parameter therefore reads `(mut list: List<Int>) => ...`, while the same function parameter reads `list: mut List<Int>`. This spec implements both exactly as decided and makes the mixed form an error with a fix ([grammar.md, 6.18](grammar.md#618-lambdas)). Whether the two should be made consistent needs a new decision.
2. **The naming style beyond the first letter.** The first letter of a declared name is checked by the parser. Whether the rest of the style (no `camelCase`, no `SCREAMING_CASE`) is an error or a warning is not decided ([lexical.md, 5.3](lexical.md#53-the-naming-rule)).
3. **Reserving words for future features.** Reserving a word later breaks programs that use it as a name ([lexical.md, 6.3](lexical.md#63-why-the-others-are-reserved)).
4. **More format specs:** alignment, signs, scientific notation, and widths taken from variables ([lexical.md, 9.4](lexical.md#94-format-specs)).
5. **Re-exports** with `pub import` ([grammar.md, 3.2](grammar.md#32-visibility)), and import aliases (`import x as y`) ([grammar.md, 3.9](grammar.md#39-imports)).
6. **Whether imports must come first** in a file ([grammar.md, 3.9](grammar.md#39-imports)).
7. **Supertraits** such as `trait Compare: Equal` ([grammar.md, 3.7](grammar.md#37-traits)).
8. **Extending a generic type for every type argument**, for example all `List<T>` ([grammar.md, 3.8](grammar.md#38-extend)), and `where` clauses ([grammar.md, 3.4](grammar.md#34-generic-parameters)).
9. **`comptime` inside function bodies** ([grammar.md, 3.11](grammar.md#311-compile-time-items)).
10. **Items declared inside blocks**, such as a helper function inside a function ([grammar.md, 4.2](grammar.md#42-statements)).
11. **The unit value `()` and one-element tuples** ([grammar.md, 6.12](grammar.md#612-primary-expressions)).
12. **Numbered access to tuple elements** such as `pair.0` ([grammar.md, 6.10](grammar.md#610-postfix-operators-and-calls)).
13. **Function types that can throw**, such as a parameter that takes a throwing function ([grammar.md, 7.1](grammar.md#71-type-syntax)).
14. **Rest patterns in lists** (the first element and the rest). The obvious spelling would need the banned bare `..` ([grammar.md, 8.1](grammar.md#81-pattern-syntax)).
15. **Open-ended ranges** such as "5 and up" ([grammar.md, 6.7](grammar.md#67-ranges)), for the same reason.
16. **Loop labels and `break` with a value** ([grammar.md, 6.19](grammar.md#619-jump-expressions)).
17. **Declaring a lambda's return type** ([grammar.md, 6.18](grammar.md#618-lambdas)).
18. **Compound assignment for `div`** ([grammar.md, 4.4](grammar.md#44-assignment)).
19. **Tuple assignment** such as `(a, b) = (b, a)` ([grammar.md, 4.4](grammar.md#44-assignment)).
20. **Characters made of several code points**, such as an emoji with a skin tone. The lexer accepts one Unicode scalar value per character literal; whether `Char` should hold more is a type system question ([lexical.md, 8](lexical.md#8-character-literals)).
