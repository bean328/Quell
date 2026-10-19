// Functions: reusable pieces of code with inputs and an output.

// A function whose whole body is one expression uses `=`
fn square(x: Int) -> Int = x * x

// A longer function uses { }. Its last line is the result.
fn describe_number(n: Int) -> String {
    let kind = if n % 2 == 0 { "even" } else { "odd" }
    "{n} is {kind}"
}

// `return` leaves a function early
fn safe_divide(a: Int, b: Int) -> Float {
    if b == 0 { return 0.0 }
    a / b
}

// Default values, and arguments you can name when you call
fn greet(name: String, greeting: String = "Hello") = print("{greeting}, {name}!")

print(square(4))
print(describe_number(7))
print(safe_divide(7, 2))
print(safe_divide(1, 0))
greet("Ava")
greet("Sam", greeting: "Welcome")
greet(name: "Kai", greeting: "Hey")

// A function can give back several values at once
fn min_max(numbers: List<Int>) -> (Int, Int) {
    var low = numbers[0]
    var high = numbers[0]
    for n in numbers {
        if n < low { low = n }
        if n > high { high = n }
    }
    (low, high)
}

let (low, high) = min_max([4, 9, 1, 7])
print("lowest {low}, highest {high}")

// Output:
// 16
// 7 is odd
// 3.5
// 0.0
// Hello, Ava!
// Welcome, Sam!
// Hey, Kai!
// lowest 1, highest 9
