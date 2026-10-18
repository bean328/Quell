# 0005: Front-end architecture: hand-written parser, lossless syntax tree

**Status:** Accepted (October 2026)

## Context

Phase 1 builds the part of the compiler that reads code: the lexer, the parser and the error messages. The same code will later power the formatter (`quell fmt`) and editor support (the language server), so the design has to serve all three from the start.

Options considered:

- **A parser generator** (LALR like yacc, or PEG): less code to write, but error messages and error recovery are much worse, and Quell's newline rules are awkward to express.
- **A hand-written parser producing an abstract syntax tree** (AST): good errors, but comments and spacing are thrown away, so the formatter and editor tools would have to recover them from the source text. rustfmt is known to suffer from this.
- **A hand-written parser producing a lossless concrete syntax tree** (every character of the source is kept), with a typed AST view on top. This is how rust-analyzer, Swift's new parser (swift-syntax) and Roslyn (C#) work.

## Decision

- **Lexer:** hand-written. It produces every token, including spaces, newlines and comments ("trivia"), so nothing is lost. It reports problems like `&&`, a lone `!` or a bare `..` with a helpful fix, then keeps going.
- **Parser:** hand-written recursive descent, with Pratt parsing (precedence climbing) for expressions. It applies the newline rules from `spec/lexical.md`.
- **Event-based:** the parser emits a flat list of events (start node, token, finish node, error). A separate tree builder turns the events into the tree. This keeps the parser simple and makes error recovery cheap.
- **Lossless tree:** built with [rowan](https://github.com/rust-analyzer/rowan) 0.16, the library behind rust-analyzer (licensed MIT OR Apache-2.0, like Quell). For every file, the tree's text is exactly the source text, byte for byte.
- **Typed view:** small typed wrappers (`FnDef`, `MatchExpr`, ...) over the tree, so later stages don't work with raw nodes.
- **Error recovery:** the parser never stops at the first error. It wraps what it couldn't understand in an `ERROR` node, reports a diagnostic, and continues from a safe point (the next line at the same level, a closing brace, or the next `fn`/`struct`/`enum` keyword). Users see all their mistakes at once.
- **Error messages:** Quell's own `quell_diagnostics` crate renders them, in the format shown in the plan (§5.2). Error output is a core promise of Quell, so we keep full control over it instead of depending on a rendering library.

## Testing

- Every program in `examples/` must parse with **zero errors**, and the tree must give back the exact source text.
- Snapshot tests (with `insta`) record the tree and the error messages for a set of small inputs. Any change to them is reviewed like code.
- A robustness test feeds thousands of broken variations of the examples (tokens deleted, swapped or duplicated) to the parser. It must never crash or hang, only report errors.

## Why

- **Best possible error messages:** a hand-written parser knows what it was trying to parse when something goes wrong, so it can say "this `if` is missing its `{`" instead of "unexpected token".
- **One tree for everything:** compiler, formatter and editor share it, so they can never disagree.
- **Proven:** the same design runs in the editors of millions of Rust, Swift and C# developers.

## Trade-offs

- More code than a generated parser, and the grammar lives in two places (`spec/grammar.md` and the parser). The example and snapshot tests keep them in sync.
- A lossless tree uses more memory than a minimal AST. rowan shares identical subtrees, which keeps this small, and the efficiency budgets in the plan (§7A) still apply.
