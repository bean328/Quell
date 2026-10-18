// Tests live right next to the code they check.
// `quell run` runs the program. `quell test` runs the tests.

fn fizzbuzz(n: Int) -> String {
    if n % 15 == 0 { return "FizzBuzz" }
    if n % 3 == 0 { return "Fizz" }
    if n % 5 == 0 { return "Buzz" }
    return "{n}"
}

for n in 1..=15 {
    print(fizzbuzz(n))
}

test "multiples of three say Fizz" {
    assert fizzbuzz(3) == "Fizz"
    assert fizzbuzz(9) == "Fizz"
}

test "multiples of three and five say FizzBuzz" {
    assert fizzbuzz(15) == "FizzBuzz"
    assert fizzbuzz(30) == "FizzBuzz"
}

test "other numbers stay as they are" {
    assert fizzbuzz(7) == "7"
}

// When a test fails, Quell shows both sides:
//     FAILED  multiples of three say Fizz   (20_testing.ql:17)
//         assert fizzbuzz(9) == "Fizz"
//         left:  "9"
//         right: "Fizz"

// Output (quell run):
// 1
// 2
// Fizz
// 4
// Buzz
// Fizz
// 7
// 8
// Fizz
// Buzz
// 11
// Fizz
// 13
// 14
// FizzBuzz
//
// Output (quell test):
// running 3 tests
//   ok  multiples of three say Fizz
//   ok  multiples of three and five say FizzBuzz
//   ok  other numbers stay as they are
// 3 passed, 0 failed
