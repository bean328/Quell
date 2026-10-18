// Strings: text.

let first = "Ada"
let last = "Lovelace"
let full = first + " " + last

print(full)
print("Length: {full.length()}")
print(full.upper())
print(full.contains("Love"))
print(full.replace("Ada", "Augusta"))

// Any expression can go inside { }
print("2 + 2 = {2 + 2}")

// Splitting and joining
let colors = "red,green,blue".split(",")
print(colors)
print(colors.join(" + "))

// Multi-line strings. The indentation before the closing """ is removed.
let poem = """
    Roses are red,
    Quell is fast.
    """
print(poem)

// Raw strings: no { } values and no escapes. Handy for JSON and patterns.
let json = #"{"name": "Ava"}"#
print(json)

// A literal brace in a normal string is written twice
print("Write {{name}} to insert a value")

// A single character uses single quotes
let initial = 'A'
print("{initial} is for Ada")

// Going through the characters
for ch in "hey".chars() {
    print(ch)
}

// Output:
// Ada Lovelace
// Length: 12
// ADA LOVELACE
// true
// Augusta Lovelace
// 2 + 2 = 4
// ["red", "green", "blue"]
// red + green + blue
// Roses are red,
// Quell is fast.
// {"name": "Ava"}
// Write {name} to insert a value
// A is for Ada
// h
// e
// y
