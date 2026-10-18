// Generics: one piece of code that works with many types.

// Works for a list of anything that can be compared
fn largest<T: Compare>(items: List<T>) -> T? {
    var best = items.first() else { return none }
    for item in items {
        if item > best { best = item }
    }
    return best
}

print(largest([3, 9, 4]) ?? 0)
print(largest(["pear", "apple", "zebra"]) ?? "")
print(largest(List<Int>()) ?? -1)

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
// blue plate
// blue plate
// 1
