# 0004: Syntax review: fixes to 0003

**Status:** Accepted (October 2026). Revises [0003](0003-syntax.md); its table now shows the current syntax.

## Context

After writing the 29 examples, every syntax decision in 0003 was reviewed again, asking for each one: is this really the best choice, is it consistent with the rest, and does it clash with anything? The review found one real bug, several inconsistencies, and some gaps.

## Changes

### 1. Enum options: no leading dot (bug fix)

**Problem:** `.Sunny` was shorthand for `Weather.Sunny`, but a line starting with `.` also continues the line above (for chains like `.filter(...)`). So `.Sunny` as the last line of a block would have been read as part of the previous line. The two rules conflict.

**Fix:** when the type is known, write the option's name directly: `[Sunny, Rainy(mm: 2.5)]`, `throw Empty`. `Weather.Sunny` always works too. If a name could mean two things, the compiler asks you to write the full name.

**Why this is better:** building a value now looks exactly like matching it (`Rainy(mm) => ...`). Gleam, OCaml and Haskell work this way.

### 2. Every block gives its last line as its value

**Problem:** lambdas, `match` arms and `catch` blocks gave their last line as their value, but function bodies needed `return`. Three rules that look alike but behave differently.

**Fix:** every block, including function bodies, gives its last line as its value. `return` is only for leaving early. A function whose whole body is one expression is written with `=`, and the formatter rewrites `{ x * x }` to `= x * x`, so there is always one way to write it.

**Why this is better:** one rule everywhere. Rust, Gleam and Elixir (three of the four most admired languages in 2025) all work this way. Rust's well-known trap, where a semicolon silently turns a value into nothing, doesn't exist in Quell because Quell has no semicolons.

### 3. Optionals: check with `if`, leave early with `??`

**Problem:** there were five tools for "might be missing": `if let`, `let ... else`, `?.`, `??` and `match`. `if let` is jargon known mostly to Swift and Rust users.

**Fix:**
- **Narrowing** (from TypeScript and Kotlin): after `if x != none`, or after an `if x == none { ... }` that leaves early, the compiler knows `x` is there. The same works for types: after `if shape is Circle`, `shape.radius` is available. It applies to `let` values, parameters, and `var`s until they're changed.
- **`??` can leave early** (from Kotlin's `?:`): the right side may be `return`, `throw`, `break`, `continue`, or anything that never returns, like `exit(1)`. `let age = text.to_int() ?? throw NotANumber(text: text)`.
- `if let` and `let ... else` are removed.

**Why this is better:** two fewer special constructs, and you check things the way you'd naturally think about them.

### 4. Equality, printing and hashing are automatic

**Problem:** comparing (`==`) or printing a struct needed `@derive(Eq, Show)`, so beginners had to learn attributes very early.

**Fix:** every struct and enum automatically supports `==`, printing and hashing when all of its fields do. `@derive` is only for extra abilities such as `Json` and `Compare`. The code is only generated when it's used, so there's no cost to programs that don't use it. If a type loses one of these abilities (for example, by gaining a field that can't be compared), the automatic version checker in `quell publish` reports it as a breaking change.

**Why this is better:** what beginners expect just works. Gleam and Elixir do this.

### 5. Lambdas that change their input say `mut`

**Problem:** `log.lock(entries => entries.append(x))` changed `entries` with no visible marker, which breaks the rule that changes are always visible.

**Fix:** `log.lock(mut entries => entries.append(x))`. Leaving out `mut` is a compile error with a fix. Lambdas that only read stay as they are.

### 6. Smaller gaps

- **Newlines inside brackets:** inside `( )` and `[ ]`, a new line never ends a statement, so long calls and lists can be split safely. Go, Swift and Kotlin work the same way.
- **Several generic requirements** are joined with `and`: `<T: Compare and Show>`. This follows the "words, not symbols" rule.
- **Characters** use single quotes: `'a'` is a `Char`. Strings always use double quotes.
- **Format specs in strings:** in `"{price:.2}"` the format spec starts at the first `:` that isn't inside parentheses or brackets, so `"{greet(name: "Ava")}"` still works.
- **Sourcing:** in 0003, the claim that non-programmers rated `for`, `while` and `foreach` least intuitive is now marked as coming from a secondhand summary.

## Kept after review

- **Braces, `fn`, `let`/`var`, `and`/`or`/`not`, no semicolons:** supported by both the "most admired" data and the beginner research.
- **`..<` and `..=`, `repeat`, `/` for exact division and `div` for whole numbers:** clear, and they prevent real bugs.
- **`->` and `=>`:** consistent. `->` means "returns this type", `=>` means "gives this value".
- **One deliberate inconsistency:** functions need `mut` at the call site (`heal(mut hero)`), but methods that change their value don't (`scores.append(4)`). Requiring `mut` on every changing method call would make everyday code noisy. The `var` requirement still prevents accidental changes. Editors can additionally highlight variables that a line changes.

## How we'll know it's right

Reasoning alone can't prove a syntax is the best. Two checks are now part of the plan:

1. **Phase 1:** the parser must parse every example with no ambiguity. Any clash like #1 above is fixed in the design, not patched in the parser.
2. **Before 1.0:** a beginner study in the style of Stefik and Siebert, including a comparison group. People who have never seen Quell read and write short programs, and we measure how accurately they do it. Syntax that people misread gets changed.
