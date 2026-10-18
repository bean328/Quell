# Quell by Example

These programs define what Quell **is**. They're the Phase 0 design: written before the compiler exists, so we can judge the language by reading real code instead of rules.

There is no compiler yet, so none of these run today. Every example ends with an `// Output:` block. In Phases 1 and 2 those blocks become the first test suite: the compiler has to produce exactly that output.

The syntax and where each piece of it comes from: [decision 0003](../docs/decisions/0003-syntax.md), revised after review in [decision 0004](../docs/decisions/0004-syntax-review.md).

## Level 1: Basics

| Example | Teaches |
|---|---|
| [01_hello.ql](01_hello.ql) | The smallest program |
| [02_variables.ql](02_variables.ql) | `let`, `var`, inferred types, string values |
| [03_numbers.ql](03_numbers.ql) | `Int`, `Float`, `/`, `div`, `%`, overflow safety |
| [04_conditions.ql](04_conditions.ql) | `if`, `else`, `and`, `or`, `not`, `if` as a value |
| [05_loops.ql](05_loops.ql) | `for`, ranges `..<` and `..=`, `repeat`, `while`, `break`, `continue` |
| [06_functions.ql](06_functions.ql) | `fn`, `=` for one-expression functions, the last line as the result, `return`, defaults, named arguments |
| [07_lists.ql](07_lists.ql) | Lists, `map`, `filter`, `sum`, `sort`, safe indexing |
| [08_maps.ql](08_maps.ql) | Maps, lookups that might miss |
| [09_strings.ql](09_strings.ql) | Text, interpolation, multi-line and raw strings |
| [10_guess_the_number.ql](10_guess_the_number.ql) | A complete game |

## Level 2: Modeling

| Example | Teaches |
|---|---|
| [11_structs.ql](11_structs.ql) | Structs, methods, `mut fn`, `static fn`, automatic `==` and printing |
| [12_enums_and_match.ql](12_enums_and_match.ql) | Enums with data, `match`, guards, ranges and `or` in patterns |
| [13_optionals.ql](13_optionals.ql) | `T?`, `none`, checking with `if`, `?.`, `??`, leaving early with `?? return` |
| [14_errors.ql](14_errors.ql) | `throws`, `throw`, `try`, `catch`, matching on errors |
| [15_value_semantics.ql](15_value_semantics.ql) | Why values never change behind your back; `mut` at the call site |
| [16_todo_app.ql](16_todo_app.ql) | A complete command-line app that saves to a file |

## Level 3: Organizing

| Example | Teaches |
|---|---|
| [17_traits.ql](17_traits.ql) | Traits, default methods, `any`, `is`, `extend` |
| [18_generics.ql](18_generics.ql) | Generic functions and structs, several requirements with `and` |
| [19_modules/](19_modules/main.ql) | Several files, folders, `import`, `pub` |
| [20_testing.ql](20_testing.ql) | `test` blocks, `assert`, `quell test` |
| [21_library/](21_library/src/lib.ql) | A publishable library: manifest, permissions, docs that are tested |

## Level 4: Power

| Example | Teaches |
|---|---|
| [22_lambdas_and_chaining.ql](22_lambdas_and_chaining.ql) | Lambdas, chaining, function types |
| [23_parallel.ql](23_parallel.ql) | `parallel for`, `group`, `spawn`, channels, no data races |
| [24_sharing.ql](24_sharing.ql) | `Shared`, `Atomic`, `Mutex`, `Arena` |
| [25_web_server.ql](25_web_server.ql) | A JSON web server |
| [26_files_and_cleanup.ql](26_files_and_cleanup.ql) | Files that close themselves, `defer` |

## Level 5: Expert

| Example | Teaches |
|---|---|
| [27_comptime.ql](27_comptime.ql) | Compile-time code, `@derive`, looking at types |
| [28_c_interop/](28_c_interop/main.ql) | Using a C library, `unsafe` |
| [29_embedded_blink/](29_embedded_blink/main.ql) | A microcontroller with no operating system |

## Decisions made while writing these

Writing real programs forced some smaller decisions. They'll go into the spec:

1. **Maps remember insertion order**, so printing and looping give predictable results.
2. **Floats always print with a decimal point** (`8.0`, not `8`), so you can tell them apart from Ints.
3. **Printing an optional** shows the value, or `none`.
4. **Indexing:** `list[i]` stops the program with a clear message if `i` is out of range, and `list.get(i)` gives an optional instead. `map[key]` always gives an optional, because a missing key is normal, not a bug.
5. **Parentheses mean "doing something":** methods always use them, even with no arguments (`length()`). No parentheses means stored data (a field).
6. **Tasks inside `group` and `parallel for` can read** the variables around them. They can only *change* shared data through `Atomic`, `Mutex` or channels.
7. **Calling C always needs `unsafe { }`**, because the compiler can't check C code.
8. **A failed `try` at the top of a script** stops the program and prints the error.
9. **Newlines:** a line that starts with `.` continues the line above. A line that ends with an operator or a comma continues onto the next one. Inside `( )` and `[ ]`, new lines never end a statement.
10. **`exit(code)` never returns**, so after `if command == none { exit(1) }` the compiler knows `command` is there.
11. **Struct fields** on separate lines need no commas. On one line, separate them with commas.
12. **Lambdas that change their input say `mut`**: `log.lock(mut entries => entries.append(x))`.
13. **Comparing, printing and hashing are automatic** for structs and enums whose fields support them. `@derive` is only for extras like `Json`.
14. **Enum options are written by name** when the type is known (`Rainy(mm: 2.5)`), exactly like in `match`. The full name (`Weather.Rainy`) always works too.
