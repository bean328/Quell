# Quell project briefing

Quell is a programming language by Noctis Labs: as fast as Rust and C++, as safe as Rust, as easy as Python. This file is the briefing for anyone (human or AI) starting work on the repo. The repo is the project's memory: if something matters, it is written down here, in `docs/`, or in a test.

## Where things are

- `docs/PLAN.md`: the master plan (goals, design principles, roadmap with phases and exit criteria).
- `docs/decisions/`: one record per major decision, with reasons and trade-offs. Read the relevant ones before changing anything they cover.
- `spec/`: the language specification (v0.1 covers syntax only: `lexical.md`, `grammar.md`).
- `examples/`: 29 programs that define the language. Each ends with an `// Output:` block that becomes a test.
- `compiler/`: the Rust workspace. `quell_diagnostics` (error messages), `quell_syntax` (lexer, parser, lossless tree), `quell_cli` (the `quell` command).

## Rules

- **No emojis, no em dashes, no en dashes** anywhere: code, comments, docs, commit messages, chat. Use commas, colons, parentheses or separate sentences.
- **Decisions are recorded.** A change to the language or architecture needs a new record in `docs/decisions/` (records are never deleted; a new one supersedes or revises an old one) and must follow the design principles in `docs/PLAN.md` section 2.
- **The settled syntax** is in `docs/decisions/0003-syntax.md` as revised by `0004-syntax-review.md`. The spec and parser implement it; they don't change it.
- **Error messages are a feature.** Every error says what happened, where, and how to fix it, ideally with an exact edit. See `docs/PLAN.md` section 5.2.
- **Plain, clear English** in docs and messages. The audience includes beginners.
- Write code that reads like the surrounding code. Keep the workspace free of warnings.

## Before every commit

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Every example in `examples/` must keep parsing with zero errors once the parser exists.

## Git

- Work on `main` unless told otherwise; push to `https://github.com/bean328/Quell.git`.
- Commit messages: a short summary line, then what changed and why.
- Never add `Co-Authored-By` lines or any other AI attribution to commits.
