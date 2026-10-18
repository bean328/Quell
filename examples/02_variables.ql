// Variables: `let` for values that never change, `var` for values that do.
// Quell works out the types for you.

let name = "Ava"          // a String
let age = 12              // an Int
let height = 1.52         // a Float
let likes_coding = true   // a Bool

var score = 0
score = score + 10
score += 5

print("{name} is {age} years old and {height} m tall.")
print("Likes coding: {likes_coding}")
print("Score: {score}")

// You can write the type yourself when you want to:
let lives: Int = 3
print("Lives: {lives}")

// Changing a `let` is a compile error, with a fix:
//     name = "Sam"
//     error: `name` can't be changed because it was declared with `let`.
//     help: declare it with `var name = "Ava"` if it should change.

// Output:
// Ava is 12 years old and 1.52 m tall.
// Likes coding: true
// Score: 15
// Lives: 3
