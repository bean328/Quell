// Loops: `for`, `repeat` and `while`.

// for: go through each item
let fruits = ["apple", "banana", "cherry"]
for fruit in fruits {
    print("I like {fruit}")
}

// Ranges: `..<` stops before the end, `..=` includes the end
for i in 0..<3 {
    print("i = {i}")
}
for n in 1..=3 {
    print("n = {n}")
}

// repeat: do something a number of times
repeat 2 {
    print("Hip hip hooray!")
}

// while: keep going while something is true
var countdown = 3
while countdown > 0 {
    print("{countdown}...")
    countdown -= 1
}
print("Liftoff!")

// continue skips to the next round, break stops the loop
for n in 1..=10 {
    if n % 2 == 0 { continue }
    if n > 5 { break }
    print("odd: {n}")
}

// repeat with no number goes forever, until `break`
var tries = 0
repeat {
    tries += 1
    if tries == 3 { break }
}
print("Stopped after {tries} tries")

// Output:
// I like apple
// I like banana
// I like cherry
// i = 0
// i = 1
// i = 2
// n = 1
// n = 2
// n = 3
// Hip hip hooray!
// Hip hip hooray!
// 3...
// 2...
// 1...
// Liftoff!
// odd: 1
// odd: 3
// odd: 5
// Stopped after 3 tries
