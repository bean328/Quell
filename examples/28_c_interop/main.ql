// Using a C library directly. Quell reads the C header itself,
// so every C library in the world works with Quell on day one.

import c "sqlite3.h"

// The `unsafe` part stays small and wrapped in a safe function,
// so the rest of the program never touches C directly.
fn sqlite_version() -> String {
    unsafe {
        return String.from_c(c.sqlite3_libversion())
    }
}

print("SQLite version: {sqlite_version()}")

// Using C without `unsafe` is a compile error:
//     c.sqlite3_libversion()
//     error: calling C code needs an `unsafe { }` block, because the compiler
//     can't check what C code does.

// Output (the version depends on your system):
// SQLite version: 3.46.1
