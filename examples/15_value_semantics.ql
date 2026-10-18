// Value semantics: in Quell, values never change behind your back.

var a = [1, 2, 3]
var b = a          // b is its own list
b.append(4)
print(a)           // a is unchanged
print(b)

struct Counter {
    count: Int = 0
}

// A normal parameter is read-only: the function can look but not change it
fn show_doubled(counter: Counter) {
    print("doubled: {counter.count * 2}")
}

// A `mut` parameter may be changed by the function
fn bump(counter: mut Counter) {
    counter.count += 1
}

var c = Counter()
show_doubled(c)
bump(mut c)        // `mut` here shows, at a glance, that `c` may change
bump(mut c)
print("count: {c.count}")

// Forgetting `mut` is a compile error, so changes are never hidden:
//     bump(c)
//     error: `bump` changes its argument.
//     help: write `bump(mut c)` to allow it.

// None of this is slow. The compiler only really copies `a` when `b` changes it,
// passes read-only parameters without copying, and moves values instead of
// copying whenever a variable isn't used again.

// Output:
// [1, 2, 3]
// [1, 2, 3, 4]
// doubled: 0
// count: 2
