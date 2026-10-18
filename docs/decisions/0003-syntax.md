# 0003: Syntax: braces for structure, words for meaning

**Status:** Accepted (October 2026), revised by [0004](0004-syntax-review.md). The table below shows the current syntax. Replaces the Python-style indentation syntax in the first draft of the plan.

## Context

Quell must have its own look, not Python's, while staying as easy to learn as Python. Instead of choosing by taste, we looked at three things:

1. **What developers love most.** In the Stack Overflow Developer Survey 2025, the most admired languages were Rust (72%), Gleam (70%), Elixir (66%) and Zig (64%). Three of the four (Rust, Gleam, Zig) share the same core shape: braces, `fn`, no semicolons, and powerful pattern matching.
2. **What beginners find easiest.** Stefik and Siebert (*An Empirical Investigation into Programming Language Syntax*, ACM Transactions on Computing Education, 2013) tested novices on six languages. Novices using C-style languages (Java, Perl) were **no more accurate than with Randomo**, a language whose keywords were chosen at random. Python, Ruby and Quorum gave significantly higher accuracy. A summary of the paper also reports that non-programmers rated `for`, `while` and `foreach` as the least intuitive words for loops.
3. **What everyone already knows.** Braces are used by JavaScript, Java, C, C++, C#, Go, Swift, Kotlin, PHP, Rust, Gleam and Zig: the most widely known syntax family in the world.

**How we read this together:** the research does not single out braces. It points at the C style *as a whole*: cryptic symbols (`&&`, `||`, `!`), `for (int i = 0; i < n; i++)`, types written before names, semicolons. The most admired modern languages kept braces and dropped most of the rest. Quell goes one step further and also replaces the symbols beginners trip on with plain words.

## Decision

**Braces for structure, words for meaning.**

- The *shape* (blocks, functions, declarations, matching) comes from the most admired modern languages: Rust, Gleam, Swift, Kotlin, Zig.
- The *words* come from the languages beginners do best with: `and`, `or`, `not`, `for x in`, `repeat`.
- Where nothing existing was clear enough, Quell has its own syntax.

### Every choice, and where it comes from

| Feature | Quell | Borrowed from | Why |
|---|---|---|---|
| Blocks | `{ }`, always required | Rust, Swift, Go, Kotlin, Gleam, Zig | Known by nearly every programmer; copy-paste can't break code; always-required braces prevent "which lines belong to the `if`?" bugs. |
| Statement endings | Newline. No semicolons. | Swift, Kotlin, Go, Gleam | Semicolons add noise and errors and give nothing back. |
| Line continuation | A line continues if it ends with an operator or comma, if the next line starts with `.`, or while inside `( )` or `[ ]` | Swift, Kotlin, Go | Long expressions can be split safely, with simple rules. |
| Conditions | `if x > 5 {` (no parentheses) | Rust, Swift, Go, Gleam | Less punctuation. |
| Comments | `//` and `/* */` (nestable). Docs: `///` | C family, Rust, Swift | Universally known. |
| Functions | `fn add(a: Int, b: Int) -> Int { }` | Rust, Gleam, Zig | `fn` is shared by the 3 most admired brace languages. |
| Block values | Every block gives its last line as its value, function bodies included. `return` leaves early. | Rust, Gleam, Elixir | One rule everywhere. No semicolons means none of Rust's "a semicolon made it return nothing" trap. |
| One-expression functions | `fn square(x: Int) -> Int = x * x` (the formatter uses this whenever the body is one expression) | Kotlin, Scala | Short functions stay short, and there's still one way to write each function. |
| Variables | `let` (fixed), `var` (changes) | Swift | Two clearly different words. Typos become errors instead of new variables. |
| Type annotations | `name: Type`, return `-> Type` | Rust, Swift, Kotlin, TypeScript, Gleam | Name first, type second: reads naturally and keeps types optional. |
| Logic | `and`, `or`, `not` | Python, Ruby, Lua, Zig (`and`/`or`) | Words beat symbols for beginners. Frees `&`, `|`, `^` for bit operations without confusion. |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` | Everyone | Universal. Writing `if x = 5` is a compile error with a fix. |
| Division | `/` exact (`7 / 2 == 3.5`), `div` whole-number, `%` remainder | `/` like Python 3 and JS; `div` like Pascal and Haskell | `7 / 2 == 3` surprises every beginner. `//` can't be used because it's a comment. |
| Strings | `"Hi {name}"`, `{value:.2}` formatting | C#, Rust format strings, Python f-strings | Shortest, cleanest interpolation. `{{` writes a literal brace. The format spec starts at the first `:` outside parentheses. |
| Characters | `'a'` | C family, Rust, Swift | Single quotes for one character, double quotes for text. |
| Raw strings | `#"{"a": 1}"#` | Swift | No escapes or interpolation; perfect for JSON and regex. |
| Multi-line strings | `""" ... """`, indentation trimmed | Swift, Kotlin | Readable blocks of text. |
| Loops | `for x in items { }`, `while cond { }` | Swift, Rust, Kotlin, Python | Familiar to almost everyone. |
| Counted loop | `repeat 3 { }`, `repeat { }` forever | Quorum, Logo, Scratch | `repeat` reads naturally to beginners. Replaces Rust's `loop`. |
| Ranges | `0..<10` (stops before 10), `1..=10` (includes 10) | Swift (`..<`), Rust (`..=`) | **Quell's own rule:** there is no bare `..`, because it means different things in Rust, Kotlin and Ruby. Every range says exactly what it does. |
| Lists and maps | `[1, 2, 3]`, `["Ava": 12]`, `[:]` | Swift | `{` always means a block, so it's never ambiguous. |
| Lambdas | `x => x * 2`, `(a, b) => a + b`, `mut list => list.append(1)` | JavaScript, TypeScript, C# | The most widely known lambda syntax in the world. `mut` shows when a lambda changes its input. |
| Function types | `(Int) -> Int` | Swift, Kotlin, TypeScript | Matches how functions are declared. `->` always means "returns this type"; `=>` always means "gives this value". |
| Pattern matching | `match x { Circle(r) => ... }` | Rust, Gleam, Scala, C# | Loved in every language that has it. Must handle every case. |
| Pattern extras | `6 or 7 =>`, `1..=5 =>`, `Rainy(mm) if mm > 10.0 =>`, `["add", title] =>` | Rust, Swift | `or` instead of `|`, matching the logic words. |
| Expressions | `if` and `match` give values | Rust, Kotlin, Gleam, Swift | `let x = if a { 1 } else { 2 }`. No separate ternary operator. |
| Optionals | `String?`, `none`, `?.`, `??`, and narrowing: after `if x != none`, `x` is known to be there | Swift, Kotlin, C#, TypeScript | Check things with a normal `if`; the compiler remembers. |
| Leaving early | `let n = text.to_int() ?? return none` (also `throw`, `break`, `continue`) | Kotlin (`?:`) | Unwrap or leave, in one line. |
| Type checks | `if shape is Circle { shape.radius }` | Kotlin, TypeScript, C#, Swift | Same narrowing as optionals. |
| Errors | `throws`, `throw`, `try`, `catch` | Swift | Readable words; errors are still plain values underneath. |
| `catch` as a value | `let n = parse(s) catch 0` | **Quell's own** (inspired by Zig) | One-line fallbacks without a `try` block. |
| Structs | `struct Point { x: Float, y: Float }` (fields on separate lines need no commas), methods inside | Swift, Kotlin, Rust | Data and its functions live together. |
| Built-in abilities | `==`, printing and hashing work automatically when all fields support them | Gleam, Elixir | What beginners expect just works, with no attribute to learn. |
| Creating values | `Point(x: 1.0, y: 2.0)` or `Point(1.0, 2.0)` | Swift, Kotlin | Looks like a function call. Named arguments are optional but encouraged. |
| Enum options | `Rainy(mm: 2.5)` when the type is known, `Weather.Rainy(mm: 2.5)` always | Gleam, OCaml, Haskell | Building a value looks exactly like matching it. |
| Mutating methods | `mut fn level_up() { }` | Swift (`mutating`) | You can see which methods change things. |
| Mutation at call site | `heal(mut hero)` | **Quell's own** (inspired by C#'s `ref`) | Changes are never hidden. |
| Traits | `trait Shape { }`, `struct Circle: Shape { }` | Rust (`trait`), Swift and Kotlin (`:`) | Short and familiar. |
| Extending types | `extend Int { fn is_even() -> Bool = ... }` | Swift, Kotlin (extensions) | Add methods to any type, even ones you didn't write. |
| Generics | `List<Int>`, `fn largest<T: Compare>(...)`, `<T: Compare and Show>` | Rust, Swift, Kotlin, TypeScript, Java, C# | The most widely recognized generic syntax. Several requirements are joined with `and`. |
| Dynamic types | `List<any Shape>` | Swift | Fast by default; flexible when asked. |
| Imports | `import std.fs`, `import std.math.{pi, sqrt}` | JavaScript, Kotlin, Swift, Go | `import` is the most widely known word for this. |
| Visibility | `pub` | Rust, Gleam, Zig | Short. Private by default. |
| Attributes | `@derive(Json)`, `@entry` | Swift, Kotlin, Java, Python | `@` is the universal sign for "extra information about this". |
| Tests | `test "name" { assert x == 1 }` | Zig | Tests read like sentences and live next to the code. |
| Concurrency | `parallel for`, `group { spawn ... }` | **Quell's own** (structured concurrency) | Reads like English; can't leak tasks. |
| Naming | `snake_case` functions and variables, `PascalCase` types and enum options | Rust, Gleam, Elixir | In patterns, capitalized names are always enum options and lowercase names are always new variables, so patterns are never ambiguous. |

### Considered and rejected

- **Indentation blocks (Python):** that's Python's look, and copy-paste or mixed tabs can silently change what code does.
- **`end` keywords (Ruby, Lua, Elixir):** reads well, but it's wordy, and long files end in stacks of `end end end`.
- **`&&`, `||`, `!` for logic:** the symbols beginners struggle with most.
- **Pipe operator `|>` (Gleam, Elixir):** loved, but methods plus `extend` already give the same top-to-bottom reading, with editor autocomplete. Two ways to do one thing breaks the "one obvious way" principle.
- **`if let` and `let ... else` (Swift, Rust):** jargon for beginners. Narrowing and `??` cover the same cases with a normal `if` (see [0004](0004-syntax-review.md)).
- **`.Option` enum shorthand (Swift):** clashes with lines that start with `.` (see [0004](0004-syntax-review.md)).
- **Square-bracket generics `List[Int]` (Go, Scala):** easier to parse, but angle brackets are what most programmers already know. Swift, C# and TypeScript show the parsing problems are solvable.
- **A bare `..` range:** means "includes the end" in Kotlin and Ruby but "stops before the end" in Rust. An off-by-one trap.

## Trade-offs

- Braces mean a little more typing than indentation. Editors insert the closing brace automatically, and the formatter keeps everything tidy.
- `div` is less common than `/` for whole-number division. It's a deliberate swap: the common case (exact division) gets the familiar symbol.
- `{` inside normal strings must be written `{{`. Raw strings (`#"..."#`) cover JSON and regex.
- Angle-bracket generics need a smarter parser (Swift's approach). This affects compiler developers only.
- A method that changes its value (`scores.append(4)`) doesn't need `mut` at the call site, while a function does (`heal(mut hero)`). This is deliberate, to keep everyday code readable (see [0004](0004-syntax-review.md)).

## Sources

- Stack Overflow Developer Survey 2025, Technology: https://survey.stackoverflow.co/2025/technology/
- Stefik, A., and Siebert, S. (2013). *An Empirical Investigation into Programming Language Syntax.* ACM Transactions on Computing Education 13(4). https://doi.org/10.1145/2534973
- Summary of the above: https://neverworkintheory.org/2014/01/29/stefik-siebert-syntax.html
