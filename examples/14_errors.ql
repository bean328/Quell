// Errors: functions that can fail say so with `throws`.
// Errors are normal values. Nothing jumps out of your code unexpectedly.

enum AgeError: Error {
    Empty
    NotANumber(text: String)
    OutOfRange(age: Int)
}

fn parse_age(text: String) -> Int throws AgeError {
    let trimmed = text.trim()
    if trimmed.is_empty() { throw Empty }
    let age = trimmed.to_int() ?? throw NotANumber(text: trimmed)
    if age < 0 or age > 150 { throw OutOfRange(age: age) }
    age
}

// `try` passes the error up to whoever called this function
fn parse_ages(texts: List<String>) -> List<Int> throws AgeError {
    var ages: List<Int> = []
    for text in texts {
        ages.append(try parse_age(text))
    }
    ages
}

// `catch` gives a fallback value
let age = parse_age("abc") catch 0
print("Age: {age}")

// `catch err { }` lets you look at the error first. The last line is the value.
let age2 = parse_age("200") catch err {
    print("Problem: {err}")
    18
}
print("Age: {age2}")

// `match` handles each case separately
for input in ["42", "", "-5", "ten"] {
    match parse_age(input) {
        Ok(age) => print("{input} -> {age}")
        Err(Empty) => print("(empty) -> please type something")
        Err(NotANumber(text)) => print("{text} -> not a number")
        Err(OutOfRange(age)) => print("{age} -> out of range")
    }
}

print(parse_ages(["1", "2", "3"]) catch [])

// Forgetting to handle an error is a compile error:
//     let n = parse_age("7")
//     error: `parse_age` can fail, but the error isn't handled.
//     help: add `try` to pass it up, `catch` to give a fallback, or `match` on it.
//
// In a script, a `try` at the top level stops the program and prints the error.

// Output:
// Age: 0
// Problem: OutOfRange(age: 200)
// Age: 18
// 42 -> 42
// (empty) -> please type something
// -5 -> out of range
// ten -> not a number
// [1, 2, 3]
