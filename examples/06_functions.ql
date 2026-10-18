// Functions: reusable pieces of code with inputs and an output.

fn add(a: Int, b: Int) -> Int {
    return a + b
}

// One-line functions use `=`
fn square(x: Int) -> Int = x * x

// Default values, and arguments you can name when you call
fn greet(name: String, greeting: String = "Hello") {
    print("{greeting}, {name}!")
}

print(add(2, 3))
print(square(4))
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
    return (low, high)
}

let (low, high) = min_max([4, 9, 1, 7])
print("lowest {low}, highest {high}")

// Output:
// 5
// 16
// Hello, Ava!
// Welcome, Sam!
// Hey, Kai!
// lowest 1, highest 9
