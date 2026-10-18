// Maps: look up a value by its key, like words in a dictionary.
// Maps remember the order you added things.

var ages = ["Ava": 12, "Sam": 14]
ages["Kai"] = 11

// Looking up a key gives an optional, because the key might not be there
let sam = ages["Sam"] ?? 0
print("Sam is {sam}")

// After checking with `if`, Quell knows the value is there
let zoe = ages["Zoe"]
if zoe != none {
    print("Zoe is {zoe}")
} else {
    print("No age for Zoe")
}

for (name, age) in ages {
    print("{name}: {age}")
}

ages.remove("Ava")
print("{ages.length()} people left")

// An empty map needs its types written once
var stock: Map<String, Int> = [:]
stock["pens"] = 20
print(stock)

// Output:
// Sam is 14
// No age for Zoe
// Ava: 12
// Sam: 14
// Kai: 11
// 2 people left
// ["pens": 20]
