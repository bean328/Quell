// Generics: one piece of code that works with many types.

// Works for a list of anything that can be compared
fn largest<T: Compare>(items: List<T>) -> T? {
    var best = items.first() ?? return none
    for item in items {
        if item > best { best = item }
    }
    best
}

print(largest([3, 9, 4]) ?? 0)
print(largest(["pear", "apple", "zebra"]) ?? "")
print(largest(List<Int>()) ?? -1)

// Several requirements at once are joined with `and`
fn show_largest<T: Compare and Show>(items: List<T>) {
    let best = largest(items)
    if best != none {
        print("Largest: {best}")
    }
}

show_largest([2.5, 7.5, 1.0])

// A struct that can hold any type
struct Stack<T> {
    items: List<T> = []

    mut fn push(item: T) {
        self.items.append(item)
    }

    mut fn pop() -> T? = self.items.remove_last()

    fn peek() -> T? = self.items.last()

    fn size() -> Int = self.items.length()
}

var plates = Stack<String>()
plates.push("red plate")
plates.push("blue plate")
print(plates.peek() ?? "empty")
print(plates.pop() ?? "empty")
print(plates.size())

// Each use gets its own specialized version of the code,
// so generics cost nothing when the program runs.

// Output:
// 9
// zebra
// -1
// Largest: 7.5
// blue plate
// blue plate
// 1
