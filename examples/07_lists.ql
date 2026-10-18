// Lists: an ordered collection of values of the same type.

var scores = [72, 95, 88]
scores.append(64)

print(scores)
print(scores[0])
print(scores.length())

// Looping with the position too
for (i, s) in scores.enumerate() {
    print("#{i + 1}: {s}")
}

// Transforming lists with small inline functions (lambdas)
let curved = scores.map(s => s + 5)
let passed = scores.filter(s => s >= 70)
print(curved)
print(passed)
print("Average: {scores.sum() / scores.length()}")

// Sorting changes the list, so `scores` must be a `var`
scores.sort()
print(scores)

// A position that doesn't exist stops the program with a clear message
// instead of reading random memory:
//     scores[10]
//     panic: index 10 is out of range for a list of length 4
// Use `.get()` when a position might not exist. It gives an optional (see 13_optionals.ql):
print(scores.get(10) ?? 0)

// Output:
// [72, 95, 88, 64]
// 72
// 4
// #1: 72
// #2: 95
// #3: 88
// #4: 64
// [77, 100, 93, 69]
// [72, 95, 88]
// Average: 79.75
// [64, 72, 88, 95]
// 0
