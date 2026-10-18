# Quell: Master Plan

**Noctis Labs** · Draft 0.1 · October 2026

> *Quell: as fast as Rust and C++, as safe as Rust, as easy as Python.*

---

## 1. Mission

Build a programming language that people can learn in a weekend and use for the rest of their careers: for scripts, games, servers, tools, science, the web, and small devices.

Here's what "best language of all time" means for us. No language is the best at *everything*, and anyone claiming that is selling something. Our goal is more useful: **Quell should be the best *default* choice for the widest range of people and projects.** A beginner, a game developer and a backend engineer should all reach for it first.

We get there by never accepting a trade-off the user can see:

| Goal | Concrete target |
|---|---|
| **Fast** | Release builds within ~1.2× of C on standard benchmarks. No garbage-collector pauses. |
| **Easy** | A Python programmer is productive in a day. A total beginner finishes the official tour in a weekend. |
| **Safe** | Safe Quell code *cannot* cause memory corruption, use-after-free, null crashes or data races. Ever. |
| **Efficient** | Tiny programs, tiny memory use, zero CPU when idle. Hello world is as light as C's. |
| **Library-friendly** | From `quell new --lib` to a published, documented package in under 5 minutes. |
| **Everywhere** | Linux, macOS, Windows, WebAssembly and microcontrollers, from one language. |

---

## 2. Design principles: the Quell constitution

Every future decision gets checked against these. When two principles conflict, **the higher one wins**.

1. **Safe by default.** Unsafe things are possible, but only explicitly, visibly, and never by accident.
2. **Clear over clever.** Code is read 10× more than it's written. If a feature makes code shorter but harder to read, we say no.
3. **Fast by default.** The *obvious* way to write something should also be the fast way. Speed should never require ugly code.
4. **One obvious way.** One formatter, one build tool, one package manager, one testing framework. No configuration debates.
5. **Errors are teachers.** Every compiler error explains what happened, why, and how to fix it. A confusing error message is a bug.
6. **Progressive disclosure.** Beginners see a small, friendly language. Experts unlock more power *without* switching languages.
7. **Pay only for what you use.** No hidden runtime costs. A program that doesn't use threads doesn't pay for threads. A program that does nothing uses nothing: no CPU, no extra memory. (See §7A.)
8. **Stability is a feature.** After 1.0, code that compiles keeps compiling. Libraries don't rot.

Why safety is ranked above speed: a fast program that crashes or gets hacked is worthless. In practice they rarely conflict, because Quell's safety checks happen at **compile time** and cost nothing when the program runs.

---

## 3. Quell at a glance

### Hello world (the entire program)
```quell
print("Hello, world!")
```

### A real program
```quell
from std.math import pi

## A shape we can measure.
enum Shape:
    Circle(radius: Float)
    Rect(width: Float, height: Float)

fn area(shape: Shape) -> Float:
    match shape:
        case Circle(radius):
            return pi * radius * radius
        case Rect(width, height):
            return width * height

let shapes = [Shape.Circle(2.0), Shape.Rect(3.0, 4.0)]

var total = 0.0
for s in shapes:
    total += area(s)

print("Total area: {total}")
```

No memory management, no null, no type declarations except where they help, and it compiles to native machine code.

### Structs and methods
```quell
struct Player:
    name: String
    health: Int = 100

    fn is_alive() -> Bool:
        return self.health > 0

    mut fn take_damage(amount: Int):
        self.health = max(0, self.health - amount)

var hero = Player(name="Ava")
hero.take_damage(30)
print("{hero.name} has {hero.health} HP")    # Ava has 70 HP
```

### Errors without crashes
```quell
fn read_config(path: String) -> Config throws ConfigError:
    let text = try fs.read_text(path)     # if this fails, the error is passed up
    return try Config.parse(text)

# Simple: fall back to a default
let config = read_config("app.toml") catch Config.default()

# Detailed: handle each case
match read_config("app.toml"):
    case Ok(config):          run(config)
    case Err(NotFound(path)): print("No config file at {path}")
    case Err(e):              print("Config problem: {e}")
```

### Values that might be missing (no null!)
```quell
fn find_user(id: Int) -> User?:
    ...

if let user = find_user(42):
    print("Welcome back, {user.name}")
else:
    print("No such user")

let name = find_user(42)?.name ?? "guest"
```

### Safe, easy parallelism
```quell
let pages = parallel for url in urls:
    http.get(url)

group:
    spawn save_to_disk(report)
    spawn email_to_team(report)
# Every task inside the group is guaranteed finished here. No forgotten threads.
```

### Tests live next to the code
```quell
test "damage never goes below zero":
    var p = Player(name="Test", health=5)
    p.take_damage(100)
    assert p.health == 0
```

---

## 4. Core language design decisions

Each decision has a **why**, so we never forget the reasoning.

### 4.1 Syntax: indentation-based, Python-flavored
- Blocks are defined by indentation and `:`, like Python. Comments use `#`, doc comments use `##`.
- `match` / `case` uses Python 3.10's syntax, so Python users already know it.
- String interpolation is built into every string: `"Hi {name}"`. There's no `f` prefix to forget, and `{{` gives a literal brace.
- **Why:** Python is the most widely taught language on Earth. Borrowing its look removes the biggest barrier for millions of people.
- **Guardrail:** the formatter enforces 4 spaces. Tabs and mixed indentation produce a friendly error with an automatic fix.

### 4.2 Variables: `let` and `var`
```quell
let name = "Ava"     # can't be reassigned
var score = 0        # can change
```
- **Why:** this is the one place we deliberately differ from Python. In Python a typo (`scroe = 5`) silently creates a new variable. In Quell it's a compile error ("did you mean `score`?"). It also shows at a glance what can change.

### 4.3 Types: strong, static, mostly invisible
- Every value has a type known at compile time, which is where the speed comes from.
- Types are **inferred inside functions**: you almost never write them in local code.
- **Public library functions (`pub fn`) must declare their parameter and return types.** Private functions and scripts don't have to.
- **Why the `pub` rule:** it makes library APIs self-documenting and stable, gives much better error messages, and lets the compiler check each file independently. That keeps rebuilds fast. Haskell and Rust both learned this lesson the hard way.
- Built-in types: `Int`, `Float`, `Bool`, `String`, `List[T]`, `Map[K, V]`, `Set[T]`, plus sized types (`Int8`…`Int64`, `UInt8`…`UInt64`, `Float32`) for those who need them.

### 4.4 Numbers that don't lie
- `Int` is 64-bit, and **overflow is an error, not a silent wraparound**, in both dev and release builds. Swift does this and stays fast. Wrapping math is available explicitly (`a.wrapping_add(b)`).
- `/` always gives a `Float` (`7 / 2 == 3.5`) and `//` is integer division, exactly like Python 3.
- No silent lossy conversions. `Float(x)` and `Int(y)` are explicit, but number literals adapt automatically (`2 * 3.5` just works).

### 4.5 Data: structs, enums, traits. No inheritance.
- `struct` is a group of named fields plus methods. It feels like a Python class.
- `enum` is a set of possible *variants*, each of which can carry data (`Circle(radius)`). Combined with `match`, the compiler guarantees you handle every case.
- `trait` is a set of abilities a type can have (`Describe`, `Compare`, `Hash`). Types opt in:
  ```quell
  trait Describe:
      fn describe() -> String

  struct Dog is Describe:
      name: String
      fn describe() -> String:
          return "{self.name} the dog"

  extend Int is Describe:          # add abilities to types you didn't write
      fn describe() -> String:
          return "the number {self}"
  ```
- **No class inheritance.** It's the source of countless confusing designs. Composition plus traits covers every real use case more clearly.

### 4.6 Generics: simple on the surface, fast underneath
```quell
fn introduce(thing: Describe):              # short form: works for any Describe type
    print("Meet {thing.describe()}")

fn largest[T: Compare](items: List[T]) -> T?:   # full form when you need it
    ...

let zoo: List[any Describe] = [Dog(name="Rex"), 42]   # `any` = mix different types
```
- Generics are compiled into specialized fast code for each type (monomorphization), with no runtime cost.
- `any Trait` explicitly asks for dynamic dispatch when you want a mixed list. Fast is the default and flexible is opt-in.

### 4.7 Errors: values, not exceptions
- Functions that can fail say so: `-> Config throws ConfigError`.
- `try` passes an error up, `catch` provides a fallback, and `match` handles each case.
- Under the hood, a throwing function returns a `Result[T, E]`, so errors are plain values. There's no hidden control flow, no stack unwinding, and **zero cost when nothing fails**.
- **Why not exceptions:** with exceptions, you can't tell from reading code what might fail. With Quell, every `try` marks a spot that can fail.
- Bugs (index out of bounds, overflow, `assert` failure) are a separate thing called a **panic**. A panic stops the program with a clear message and location. It never silently corrupts data.

### 4.8 No null, ever
- A value that might be missing has type `T?`, and the compiler makes you deal with the missing case.
- Tools: `if let`, `match`, `?.` (safe access) and `??` (default value).
- This removes the "billion-dollar mistake" entirely.

### 4.9 Memory: the heart of Quell

This is the hardest and most important part of the whole project. Here is how Quell is **as safe as Rust** with **no garbage collector**, without making you write Rust's lifetimes and borrow annotations.

**Rule 1: everything behaves like a value.**
```quell
var a = [1, 2, 3]
var b = a          # b is its own list
b.append(4)
print(a)           # [1, 2, 3]   (a never changes behind your back)
print(b)           # [1, 2, 3, 4]
```
In Python, `a` would have changed too, which is a classic beginner bug. In Quell, things only change where you can *see* them change. The same idea is known as *mutable value semantics*; Swift and Hylo use it.

**Rule 2: the compiler removes the copies.** Values *behave* as if copied, but the compiler almost never actually copies:
- **Last use = move.** If `a` is never used again, `b = a` just hands the memory over, for free.
- **Read-only parameters are borrowed.** Passing a big list to a function passes a pointer under the hood.
- **Copy-on-write.** Big collections are only copied if two owners really exist *and* one of them changes.

**Rule 3: mutation is visible at the call site.**
```quell
fn heal(mut p: Player, amount: Int):
    p.health += amount

heal(mut hero, 20)    # you can SEE that `hero` may be changed
```

**Rule 4: references are temporary (second-class).** You can borrow something for the length of a function call or a block, but you can't store a borrowed reference inside a struct or return it. **This single rule is why Quell never needs lifetime annotations:** the compiler can always see exactly where a borrow starts and ends.

**Rule 5: sharing is explicit, and safe.** When you genuinely need shared data (graphs, caches, UI trees):
- `Shared[T]`: shared, read-only data with automatic reference counting. The compiler removes most count updates; Lean 4 and Koka show this works.
- `Shared[Mutex[T]]` / `Atomic[T]`: shared *changeable* data, safe across threads.
- `Arena[T]` + handles: the fast, easy pattern for graphs and game entities.
- `Weak[T]`: breaks reference cycles.

**Rule 6: cleanup is automatic and predictable.** Memory is freed at a value's last use, like Rust and unlike a garbage collector. Files, sockets and locks close themselves when they go out of scope. `defer` exists for custom cleanup.

**The honest trade-off:** a few patterns are harder than in C++, such as a struct that holds a pointer into another struct. Quell steers those toward indices, `Shared`, or arenas, which are usually cleaner designs anyway. We **test this model early with real programs** (Phase 2) before building anything on top of it.

### 4.10 Concurrency: structured and race-free
- **`parallel for`** for data parallelism (process a million items on all CPU cores).
- **`group:` + `spawn`** for structured tasks. Tasks can't outlive their group, so no task leaks.
- **Lightweight tasks, no `async`/`await` split.** Like Go, any function can do I/O without being "colored" as async. Millions of tasks are cheap.
- **Data races are impossible.** A spawned task can only take values it owns, read-only `Shared` data, or properly locked data (`Mutex`, `Atomic`, channels). The compiler checks this.
- Microcontroller builds can leave the task scheduler out entirely (see §7).

### 4.11 Compile-time code instead of macros
```quell
@derive(Eq, Hash, Show, Json)
struct Point:
    x: Float
    y: Float
```
- `comptime` runs normal Quell code during compilation, the way Zig does. It can generate code, check configs, or build lookup tables.
- `@derive(...)` and custom attributes are written in plain Quell using compile-time reflection.
- **Why not macros:** macros are a second language inside the language and very hard to learn. With `comptime`, library authors use the Quell they already know.

### 4.12 Unsafe: locked behind a door with a sign on it
- `unsafe:` blocks allow raw pointers and direct hardware or C access.
- A package must declare `unsafe = true` in its manifest to use them, and the package registry shows a badge. You always know which dependencies use unsafe code.

### 4.13 Modules and visibility
- One file = one module. One folder of files with a `quell.toml` = one package.
- Imports look like Python: `import std.fs`, `from std.math import sqrt, pi`.
- Everything is **private by default**. `pub` makes it part of your public API.
- Top-level code (scripts) runs only in the program's entry file. Libraries can't run code on import, so importing is always safe and fast.

---

## 5. Easy to learn, built for professionals

### 5.1 Learning levels (progressive disclosure)
The language is one thing, but it's taught in layers. Each level is a complete, useful language on its own.

| Level | You learn | You can build |
|---|---|---|
| 1. Basics | `print`, variables, `if`, loops, lists, functions | Calculators, games in the terminal |
| 2. Modeling | `struct`, `enum`, `match`, errors, `?` | Real tools, file processors |
| 3. Organizing | modules, packages, traits, generics, tests | Libraries, larger apps |
| 4. Power | parallelism, `Shared`, arenas, performance tuning | Servers, games, engines |
| 5. Expert | `comptime`, `unsafe`, C interop, embedded | Drivers, bindings, frameworks |

### 5.2 World-class error messages
Inspired by Elm and Rust, the two gold standards. Every error has:
- a clear title in plain English,
- the exact code location with a highlight,
- *why* it's a problem,
- a suggested fix (often applied automatically by the editor),
- an error code with a long explanation: `quell explain Q0142`.

```text
error[Q0207]: `score` can't be changed
  --> game.ql:12:5
   |
 3 | let score = 0
   |     ----- declared with `let`, so it can't change
...
12 |     score += 10
   |     ^^^^^ trying to change it here
   |
help: if `score` should change, declare it with `var`:
   |
 3 | var score = 0
   |
```

Error messages are **snapshot-tested** in CI. Changing an error message is a reviewed change, just like changing code.

### 5.3 Learning resources (official, free, versioned)
- **The Tour of Quell:** interactive, runs in the browser.
- **The Quell Book:** the complete guide, from zero to professional.
- **Quell by Example:** short, copy-paste-ready recipes.
- **Playground:** the compiler compiled to WebAssembly, running Quell in any browser, shareable by link.
- **REPL and notebooks:** `quell repl`, plus Jupyter kernel support for data work.

### 5.4 Professional from day one
- `quell fmt` has **zero options**, like Go's `gofmt`. All Quell code looks the same, so the style arguments never start.
- `quell lint` offers optional advice beyond errors.
- Full editor support through a language server: autocomplete, go-to-definition, rename, inline errors, inline type hints, quick fixes. VS Code, JetBrains, Neovim, Zed and others.
- Debugger support (standard debug info, so `gdb`, `lldb` and VS Code debugging just work).
- A built-in profiler and benchmarks: `quell bench`.

---

## 6. Libraries: the best package experience of any language

Libraries decide whether a language lives or dies, so this is designed in from the start, not added later.

### 6.1 One tool for everything
```text
quell new my_app             # new program, ready to run
quell new colors --lib       # new library, ready to publish
quell run                    # dev mode: instant
quell build --release        # release mode: maximum speed
quell test                   # runs all `test` blocks + doc examples
quell fmt                    # formats everything
quell doc --open             # builds and opens beautiful docs
quell add http               # adds a dependency
quell publish                # publishes to the registry
```

### 6.2 A tiny manifest
```toml
[package]
name = "@ava/colors"
version = "0.1.0"
description = "Mix, blend and convert colors."
license = "MIT"

[dependencies]
"@quell/json" = "1.2"
```

### 6.3 Features that make libraries effortless and trustworthy
- **Automatic semantic versioning.** On `quell publish`, the tool compares your public API with the last release. If you broke something, it **refuses to publish it as a minor update** and tells you exactly what changed. (Elm proved this works.) Users can upgrade without fear.
- **Docs are automatic.** `##` comments become documentation, hosted on the registry for every version of every package.
- **Examples in docs are tests.** Code inside doc comments is compiled and run by `quell test`, so documentation can't go stale.
  ````quell
  ## Mixes two colors evenly.
  ##
  ## ```
  ## assert mix(red, blue) == Color(r=127, g=0, b=127)
  ## ```
  pub fn mix(a: Color, b: Color) -> Color:
      ...
  ````
- **Namespaced names** (`@ava/colors`) end name-squatting and confusion.
- **Package permissions (supply-chain safety).** Packages declare what they can touch, and the compiler enforces it:
  ```toml
  [permissions]
  network = false
  filesystem = false
  unsafe = false
  ```
  A "color mixing" library that suddenly tries to use the network won't compile. Most languages have nothing like this.
- **No arbitrary build scripts.** Build-time logic is sandboxed `comptime` Quell, not "run any program on your machine".
- **Reproducible builds** with a lockfile, plus a shared build cache so dependencies compile once.
- **Editions.** The language can evolve (`edition = "2028"`) without breaking any existing library. Each package picks its edition, and they all work together. (Rust proved this works.)

### 6.4 Wrapping existing code (instant ecosystem)
- **C libraries:** `import c "sqlite3.h"` reads a C header directly, so every C library in the world is usable on day one. `quell bind` generates a safe Quell wrapper skeleton.
- **Python interop:** Quell libraries can be compiled into Python modules automatically. **This is our adoption strategy:** Python developers keep their code and speed up the slow parts with a language that looks just like Python.
- Calling *from* other languages: Quell can export a C-compatible interface, so Rust, Go, Swift, C# and others can use Quell libraries.

---

## 7. Built for everything

"Everything" comes from the ecosystem over time. The core language just has to make nothing impossible. That means: no mandatory runtime, a C-compatible ABI, and cross-compilation built in.

### Targets
| Target | Status goal |
|---|---|
| Linux, macOS, Windows (x86-64 and ARM64) | First-class from the first release |
| WebAssembly (browser and server) | First-class |
| iOS, Android | Via C ABI, then official support |
| Microcontrollers (ARM Cortex-M, RISC-V, ESP32) | "Bare" profile: no OS, no allocator required |

```text
quell build --target wasm
quell build --target linux-arm64     # cross-compile with no extra setup
```

### Profiles
- **`full`** (default): everything, including tasks, the allocator and the OS library.
- **`no-tasks`**: no task scheduler, for tiny command-line tools and plugins.
- **`bare`**: only `core`, with no heap required, for microcontrollers and kernels.

### Domains we design for
| Domain | What Quell provides |
|---|---|
| Scripts and automation | `quell run script.ql`, shebang support, instant dev mode |
| CLI tools | Single static binaries, fast startup, `@quell/cli` arg parsing |
| Web servers and APIs | Lightweight tasks, `@quell/http`, safe concurrency |
| Games | No GC pauses, arenas, SIMD, C interop for engines and graphics |
| Data and science | Python interop, notebooks, fast arrays, SIMD, GPU later |
| Systems and embedded | `bare` profile, `unsafe`, precise memory layout |
| Web front-end | WebAssembly target |

---

## 7A. Efficiency: light on memory, CPU, battery and disk

Speed is how fast a program finishes. **Efficiency** is how little it *costs* while running. Quell must be excellent at both. Many "fast" languages still ship huge programs, eat memory, or burn CPU in the background. Quell won't.

### Hello world must be as light as C's
| Measure (release build, Linux) | Quell target | For comparison |
|---|---|---|
| Program size | **under 100 KB** | C ~16 KB · Rust ~300 KB · Go ~1.5-2 MB |
| Memory used while running | **~1 MB** (basically just what the OS itself needs) | C ~1 MB · Python ~10 MB · Java/Node 30-50 MB |
| Startup time | **under 1 millisecond** | C <1 ms · Python ~20 ms · Java ~50+ ms |
| CPU used | **just the instant it takes to print** | Same as C |
| Background threads | **zero** | Go and Java start several |

### How Quell stays light
- **No garbage collector.** Nothing scans memory in the background, and memory is freed as soon as it's no longer needed (§4.9).
- **No hidden runtime.** A Quell program contains only what it uses. Hello world has no task scheduler, no thread pool, no reflection data, no extra machinery.
- **The task scheduler starts lazily.** It only starts the first time a program uses `spawn` or `parallel for`, and it creates at most one worker per CPU core.
- **Zero CPU when idle.** A server waiting for connections, or a program waiting for input, **sleeps** using the operating system's event system (epoll / kqueue / IOCP). There's no busy-waiting or polling loop anywhere in the runtime, ever. Idle tasks cost a few hundred bytes each.
- **Compact data.** The compiler automatically lays out struct fields with as little padding as possible. Optional values like `User?` take no extra space when they can be stored in a pointer that's never null. Short strings are stored inline with no separate allocation, and enums are packed tightly.
- **Unused code is deleted.** Dead-code elimination and link-time optimization remove every function your program never calls, including ones from libraries.
- **Value semantics save memory.** Because values aren't secretly shared, the compiler knows exactly when memory can be reused in place (the Lean 4 / Koka technique), which avoids most allocations.
- **Battery-friendly.** All of the above means less energy. In published studies of energy use by language, C and Rust sit at the top and Python near the bottom. Quell's goal is to rank alongside C and Rust.

### The tools are efficient too
Some language tools are notorious resource hogs; for example, editor plugins that use gigabytes of RAM. Quell's tools have budgets as well:
- **Compiling uses CPU briefly**, like every compiled language. But `quell run` caches results, so re-running unchanged code doesn't recompile anything.
- **The language server (editor support)** targets well under 500 MB of memory on a large project, and uses no CPU while you aren't typing.
- The `quell` tool itself starts instantly and never runs background services.

### Enforced, not hoped for
Efficiency budgets are **tested automatically on every change**, just like correctness. If hello world grows past 100 KB, starts slower, or uses more memory, the build fails and we find out why before the change goes in. CI tracks:
- binary size of a set of sample programs,
- peak memory and startup time,
- CPU use of an idle server (must be ~0%),
- compiler and language-server memory use on a large test project.

---

## 8. The standard library

Three layers, so the core stays small and stable while the useful stuff can move fast.

| Layer | Contents | Stability |
|---|---|---|
| **`core`** | Numbers, `String`, `List`, `Map`, `Option`, `Result`, traits, math. No OS needed. | Extremely stable |
| **`std`** | Files, paths, processes, networking, time, threads/tasks, environment, random, I/O | Stable after 1.0 |
| **Official packages** (`@quell/…`) | `json`, `http`, `regex`, `cli`, `toml`, `csv`, `crypto` (wrapping audited libraries), `test-extras` | Maintained by the core team, versioned separately |

Python's "batteries included" is a big part of why beginners love it. Quell gives the same experience: official packages are one `quell add` away and documented in the Book. They just aren't frozen into the language.

---

## 9. How the compiler works

### 9.1 Written in Rust
- It's fast, memory-safe, and has excellent compiler-building libraries.
- This **replaces my earlier "prototype in Python" suggestion.** Since we're not rushing, writing it once, properly, beats writing it twice.
- Long-term milestone: **self-hosting**, meaning the Quell compiler rewritten in Quell, which proves the language is ready for serious work.

### 9.2 The pipeline
```text
your_code.ql
   │
   ├─ 1. Lexer ............ text → tokens (including indentation)
   ├─ 2. Parser ........... tokens → syntax tree (keeps going after errors,
   │                         so you see ALL problems at once)
   ├─ 3. Name resolution .. which `x` is which? builds the module graph
   ├─ 4. Type checker ..... infers types, checks traits
   ├─ 5. Lowering ......... → QIR (Quell's internal representation)
   ├─ 6. Safety checker ... moves, borrows, mutation, thread safety
   ├─ 7. Optimizer ........ removes copies, reference counts, bounds checks
   │
   ├─ 8a. Cranelift backend → dev mode    (compiles in a blink)
   ├─ 8b. LLVM backend ...... release mode (maximum speed)
   └─ 8c. WebAssembly ....... browser and server
           │
           └─ Linker → your program
```
Plus a **QIR interpreter**, which powers `comptime`, the REPL, and serves as a second implementation that tests the compiler's correctness.

### 9.3 Built for speed and editors from the start
- **Query-based and incremental** (the technique behind rust-analyzer): when you change one function, only what depends on it is rechecked. The same engine powers the command line and the editor, so they never disagree.
- **Parallel:** modules compile on all CPU cores.
- **Two backends:** dev mode uses Cranelift (very fast compilation), release mode uses LLVM (the optimizer behind Clang, Rust and Swift). Both run the full safety checks, so dev mode never teaches bad habits.

---

## 10. Quality: how we "don't mess this up"

1. **A written specification**, kept alongside the compiler. Every behavior is defined on paper, not just "whatever the compiler does".
2. **Decision records:** every major design choice is written down with its reasons and alternatives (like §4 of this plan).
3. **Thousands of tests, in categories:**
   - *run tests*: programs that must produce exact output,
   - *reject tests*: unsafe or wrong programs that **must** fail to compile,
   - *error snapshot tests*: error messages must stay clear,
   - *formatter tests*: formatting is stable and idempotent.
4. **Fuzzing:** random and mutated programs are fed to the compiler continuously. It must never crash, only give errors.
5. **Differential testing:** the interpreter and the compiled program must agree on every test.
6. **Performance tracking:** benchmarks run on every change. If something gets slower, we know immediately.
7. **Real-program dogfooding:** at each phase we build real things in Quell (a game, a web server, a JSON library, a ray tracer). If writing them feels bad, the language changes.
8. **Beginner testing:** real newcomers try the Tour while we watch where they get stuck. Their confusion is our bug list.
9. **RFC process** (after the public preview): language changes are proposed in writing, discussed, then accepted or rejected.

---

## 11. Roadmap

Every phase ends with **exit criteria**. We don't move on until they're met. Each phase also produces something that *works*, so progress is always real.

### Phase 0: Design on paper
- Write **"Quell by Example"**: about 25 programs from beginner to expert that define what Quell *is*.
- Write the core grammar and spec v0.1.
- Settle the open questions in §14.
- **Exit:** we both look at the examples and say "yes, that's Quell."

### Phase 1: Reading code (front-end)
- Lexer (with indentation handling), error-recovering parser, syntax tree.
- The error-message engine (pretty, colorful, with suggestions).
- `quell fmt`.
- **Exit:** every example parses; broken code produces helpful errors; the formatter is stable; the parser survives fuzzing.

### Phase 2: Understanding code (semantic core) (the make-or-break phase)
- Name resolution, type inference, traits, generics.
- The memory/safety checker (§4.9).
- The QIR interpreter, so Quell programs **run** for the first time.
- Stress-test the memory model by writing real programs: a text adventure, a JSON parser, a graph algorithm, a particle simulation.
- **Exit:** all examples run; every program in the "must reject" suite is rejected; the stress programs feel *natural* to write. If they don't, we fix the model **here**, before anything is built on top of it.

### Phase 3: Real native programs
- Cranelift backend, runtime basics (allocator, panics, strings), linking.
- `quell run` and `quell build` produce real executables on Linux, macOS and Windows.
- Efficiency tracking turned on in CI (§7A): binary size, startup time, peak memory.
- **Exit:** native binaries for all examples; hello world meets the §7A targets (under 100 KB, ~1 MB memory, under 1 ms startup, zero background threads); first benchmarks against C, Rust and Python published honestly.

### Phase 4: The developer experience
- Package manager, `quell test`, `quell doc`, the language server, a VS Code extension.
- **Exit:** writing Quell in VS Code feels as good as writing Rust or TypeScript.

### Phase 5: Standard library and C interop
- `core` and `std`; `import c "header.h"`.
- **Exit:** we can build a real CLI tool and wrap a real C library (e.g., SQLite).

### Phase 6: Maximum speed
- LLVM release backend; copy, reference-count and bounds-check elimination; SIMD.
- **Exit:** within ~1.2× of C on the benchmark suite.

### Phase 7: Concurrency
- Lightweight tasks, `group`/`spawn`, `parallel for`, channels, thread-safety checks.
- **Exit:** a web server handling 100k+ concurrent connections; the race-detection suite passes; the same server uses ~0% CPU when no requests are coming in; programs that never `spawn` still start zero extra threads.

### Phase 8: Ecosystem launch prep
- `comptime` and `@derive`; the package registry with automatic semver and permissions; the playground; the Tour and Book; Python interop; WebAssembly target.
- **Exit:** `quell new --lib` to published package with docs in under 5 minutes.

### Phase 9: Public preview (0.x)
- Open to the world. Collect feedback, run beginner studies, start the RFC process, grow the community.
- **Exit:** real people building real things; the top pain points are fixed.

### Phase 10: Quell 1.0
- The stability promise: code written for 1.0 keeps working. Editions for future evolution.

**Honest timeline:** Rust took about 9 years to reach 1.0 with a full team, and Zig is still pre-1.0 after 10. We can move much faster on code. I can write a lot quickly, and we're standing on the shoulders of those projects. But *design validation, testing with real people and building a community* take real time. Expect a working, runnable Quell (Phases 0 to 3) to be a solid, achievable first goal, and treat 1.0 as a multi-year mission. Each phase delivers something real along the way.

---

## 12. How we measure success

| Area | Measure |
|---|---|
| Learnability | A Python user writes a working Quell program within 10 minutes of first seeing it. |
| Safety | No memory corruption, null crash or data race possible in safe code; fuzzers find no compiler crashes. |
| Speed (release) | Within ~1.2× of C on the benchmark suite. |
| Speed (dev) | Re-running after a small edit feels instant (under ~1 second for typical projects). |
| Efficiency | Hello world: under 100 KB, ~1 MB memory, under 1 ms startup. Idle programs use ~0% CPU. Enforced in CI. |
| Tool efficiency | Language server under 500 MB on a large project; no CPU use while you're not typing. |
| Errors | 100% of errors have an explanation and suggestion; all snapshot-tested. |
| Libraries | Create, test, document and publish a package in under 5 minutes. |
| Reach | The same program builds for Linux, macOS, Windows and WebAssembly with one command. |

---

## 13. Risks and how we handle them

| Risk | Mitigation |
|---|---|
| The memory model is too restrictive for some programs | Validate early in Phase 2 with real programs; provide `Shared`, arenas and `unsafe` as escape hatches. |
| Scope creep ("used for everything" means "never finished") | Strict phases with exit criteria; every new feature must justify itself against the constitution. |
| Type inference produces confusing errors | Types required on `pub` functions; inference stays local; error messages are a tested feature. |
| Indentation syntax causes copy-paste problems | Zero-option formatter, smart editor support, very clear indentation errors. |
| Generics slow down compile times | Incremental compilation, a fast dev backend, sharing identical specialized code. |
| Programs slowly get bigger and heavier over time ("bloat") | Size, memory, startup and idle-CPU budgets enforced in CI; any regression blocks the change. |
| Nobody uses it (the #1 way languages die) | Python interop as the entry point; superb docs; a playground; focus first on use cases where Quell clearly wins. |
| The project feels endless | Every phase ships something usable and exciting. |

---

## 14. Decisions (made and open)

**Decided** (full reasoning in [decision records](decisions/README.md)):
- **The first compiler is written in Rust**, and is later rewritten in Quell itself (self-hosting). See §9.1 and [0001](decisions/0001-compiler-in-rust.md).
- **Completely open source**, under the **MIT + Apache-2.0 dual license**. See §14A and [0002](decisions/0002-license.md).

**Still needed:**
- **A GitHub home** for the project. This coding environment is temporary and gets wiped, so the project must be pushed to GitHub to be kept.

**To settle in Phase 0 (I'll bring proposals):**
- String indexing (`text[0]`: characters vs. bytes, speed vs. simplicity)
- Closure (lambda) syntax
- Operator overloading rules
- Exact `catch` block syntax for multi-line fallbacks
- Keyword bikeshedding: `trait` vs `ability`, `is` vs `implements`, `group` vs `tasks`

**Already decided by this plan:** no null, no exceptions, no inheritance, no macros (`comptime` instead), no garbage collector, no hidden runtime or background threads, indentation syntax, value semantics, structured concurrency, mandatory types on public APIs, CI-enforced efficiency budgets.

---

## 14A. License, name and sustainability

### License: MIT + Apache-2.0 (decided)
Everything is free and open: the compiler, standard library, tools, docs and registry code. Anyone can use Quell for anything, including commercial products, forever, without paying or asking.
- **MIT** is short and simple, and the most widely understood license.
- **Apache-2.0** adds **patent protection**: contributors can't later sue users over patents in their contributions. Big companies' lawyers look for this before allowing a language.
- Users pick whichever they prefer. Rust and much of its ecosystem use this exact combination.
- **Why not a "copyleft" license (like GPL):** it would force companies to open-source their own programs, and many simply wouldn't use Quell. A language only succeeds if everyone can use it freely.
- Programs *written in* Quell belong entirely to their authors. The standard library is compiled into their programs under the same permissive terms, so there are no strings attached.

### The name "Quell" stays protected
The code is open, but the **name and logo are a trademark owned by Noctis Labs**. This isn't about money. It stops someone from shipping a broken or malicious "Quell" and confusing people. Python, Rust and Linux all work this way. A short trademark policy will say that anyone may say "built with Quell", write Quell tutorials, or make Quell libraries; only the official project may call itself "Quell".

### Sustainability (optional, all ethical)
The core stays free forever. If the project grows, these are honest ways to fund full-time work, used by Python, Rust, Zig and others. None of them lock anything away:
- **Donations and sponsorships** (GitHub Sponsors, Open Collective), like the Zig Software Foundation.
- **Company sponsors** of a future Quell Foundation, like the Rust Foundation.
- **Paid extras around the free core:** training courses, consulting and support contracts for companies, hosted build or CI services.
- **Products built with Quell**, Noctis Labs' main plan: proof that Quell works, and a business that doesn't depend on the language itself.

**Rule:** nothing in the language, compiler, standard library or official tools will ever be paywalled.

---

## 15. Next steps

1. Push this repository to GitHub so it's kept.
2. Begin **Phase 0: Quell by Example** in the `examples/` folder, and the first draft of the spec in `spec/`.
