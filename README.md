# Quell

**As fast as Rust and C++. As safe as Rust. As easy as Python.**

Quell is a general-purpose programming language by Noctis Labs. It compiles to native machine code, has no garbage collector, prevents memory and concurrency bugs at compile time, and reads like Python.

```quell
print("Hello, world!")
```

## Status

Quell is in **Phase 0: design on paper**. There is no compiler yet. See the [roadmap](docs/PLAN.md#11-roadmap).

## Documents

- [Quell by Example](examples/README.md): 29 programs, from hello world to microcontrollers, that define the language.
- [Master plan](docs/PLAN.md): goals, design principles, language design, tooling, roadmap.
- [Decision records](docs/decisions/README.md): every major decision and the reasons behind it.

## Repository layout

The layout below is the target. Folders are created when the phase that needs them begins.

```text
quell/
├── README.md
├── LICENSE-APACHE, LICENSE-MIT
├── Cargo.toml            Rust workspace for the compiler and tools (Phase 1)
├── compiler/             The compiler, one Rust crate per stage (Phase 1)
│   ├── quell_syntax/     Lexer, parser, syntax tree
│   ├── quell_diagnostics/ Error messages
│   ├── quell_types/      Name resolution, type checking
│   ├── quell_qir/        Quell IR, safety checker, interpreter
│   ├── quell_codegen/    Cranelift and LLVM backends
│   └── quell_cli/        The `quell` command
├── std/                  Standard library, written in Quell (Phase 5)
├── examples/             Quell by Example programs (Phase 0)
├── spec/                 The language specification (Phase 0)
├── tests/                Compiler test suites: run, reject, errors, format
└── docs/                 Plan, decision records, guides
```

## License

Quell is free and open source, licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Quell by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

The name "Quell" and its logo are trademarks of Noctis Labs. See [the plan](docs/PLAN.md#14a-license-name-and-sustainability) for what that means.
