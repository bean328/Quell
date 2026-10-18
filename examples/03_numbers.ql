// Numbers: `Int` for whole numbers, `Float` for decimals.

let apples = 7
let friends = 2

print(apples / friends)      // `/` always gives the exact answer
print(apples div friends)    // `div` gives whole-number division
print(apples % friends)      // `%` gives the remainder

let price = 4.99
let total = price * 3        // the number 3 adapts to a Float by itself
print("Total: {total:.2}")   // `:.2` shows two decimal places

// Mixing an Int variable with a Float is written out, so nothing is lost by accident
let items = 3
let cost = price * Float(items)
print("Cost: {cost:.2}")

let big = 1_000_000          // underscores make big numbers easy to read
print(big)

// Going past the biggest Int stops the program with a clear message
// instead of silently wrapping around to a wrong number:
//     let too_big = Int.max + 1
//     panic: overflow in `Int.max + 1` (03_numbers.ql)

// Output:
// 3.5
// 3
// 1
// Total: 14.97
// Cost: 14.97
// 1000000
