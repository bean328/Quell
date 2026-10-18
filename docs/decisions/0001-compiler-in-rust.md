# 0001: Write the first compiler in Rust, self-host later

**Status:** Accepted (October 2026)

## Context

Quell needs a compiler before it can compile itself. The language used to write that first compiler affects its speed, correctness and how long it lasts before a rewrite.

Options considered:

- **Python:** fastest to prototype, but too slow for a production compiler, so it would have to be rewritten.
- **C++:** fast, but memory bugs in the compiler would undermine a language whose main promise is safety.
- **Rust:** fast and memory-safe, with mature libraries for exactly this job.

## Decision

Write the compiler and tools in Rust. Once Quell is mature enough, rewrite the compiler in Quell itself ("self-hosting").

## Why

- **Quality over speed of start:** there is no rush, so writing it once, properly, beats a throwaway prototype.
- **Safety:** the compiler itself can't have memory-corruption bugs.
- **Speed:** a fast compiler is required for the "dev mode feels instant" goal.
- **Libraries:** Cranelift and LLVM bindings for code generation, salsa for incremental compilation, mature tools for parsing and error reporting.
- **Self-hosting later** proves Quell can handle a large, serious program, and lets the community work on the compiler in Quell.

## Trade-offs

- Rust compiles slowly, so building the compiler itself takes a while. This affects only people working on Quell, not Quell users.
- Contributors need to know Rust until the self-hosted compiler exists.
- Self-hosting is a large future project. It happens only when Quell is ready, not before.
