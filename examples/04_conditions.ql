// Making decisions with `if`, `else`, `and`, `or` and `not`.

let temperature = 23
let raining = false

if temperature > 30 {
    print("It's hot!")
} else if temperature > 15 and not raining {
    print("Nice weather for a walk.")
} else {
    print("Stay inside.")
}

// `if` can also produce a value
let clothes = if temperature > 20 { "t-shirt" } else { "jacket" }
print("Wear a {clothes}.")

let weekend = true
if weekend or temperature > 25 {
    print("Time for the beach.")
}

// Using `=` where you meant `==` is caught by the compiler:
//     if temperature = 23 { ... }
//     error: `=` gives a variable a new value. To compare, use `==`.

// Output:
// Nice weather for a walk.
// Wear a t-shirt.
// Time for the beach.
