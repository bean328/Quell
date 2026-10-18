// `comptime`: Quell code that runs while compiling, so the finished program
// doesn't have to. This replaces the macros other languages use.

import json

// A lookup table built by the compiler. The program just contains the answers.
comptime let squares = (0..<16).map(n => n * n)
print(squares[12])

// Checks that run at compile time: if one fails, the program doesn't build
comptime {
    assert squares.length() == 16, "the table has the wrong size"
}

// Comparing, printing and hashing come automatically.
// @derive adds extra abilities, using comptime under the hood.
@derive(Json)
struct Point {
    x: Int
    y: Int
}

let p = Point(x: 1, y: 2)
print(p)                          // automatic: a readable form
print(p == Point(x: 1, y: 2))     // automatic: comparing
print(json.encode(p))             // from @derive(Json)

// Library authors write their own the same way: plain Quell that can look
// at types. This is how @derive itself is built.
comptime fn field_names(T: type) -> List<String> = T.fields.map(f => f.name)

print(field_names(Point))

// Output:
// 144
// Point(x: 1, y: 2)
// true
// {"x": 1, "y": 2}
// ["x", "y"]
