# Decision records

Every major decision about Quell is written down here: what we decided, why, and what we gave up. When someone asks "why does Quell do it this way?", the answer is in this folder.

## Format

One file per decision, numbered in order: `NNNN-short-title.md`. Each has:

- **Status:** Accepted, Superseded by NNNN, or Rejected.
- **Context:** the problem and what forced a decision.
- **Decision:** what we chose.
- **Why:** the reasons, checked against the design principles in the [plan](../PLAN.md#2-design-principles-the-quell-constitution).
- **Trade-offs:** what we gave up, honestly.

Decisions are never deleted. If one changes, a new record supersedes it.

## Index

| # | Decision | Status |
|---|---|---|
| [0001](0001-compiler-in-rust.md) | Write the first compiler in Rust, self-host later | Accepted |
| [0002](0002-license.md) | Fully open source under MIT + Apache-2.0 | Accepted |
| [0003](0003-syntax.md) | Syntax: braces for structure, words for meaning | Accepted |
