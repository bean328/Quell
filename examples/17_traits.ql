// Traits: describe an ability that many different types can have.

import std.math.{pi}

trait Shape {
    fn area() -> Float
    fn name() -> String

    // A default method: every Shape gets this for free
    fn summary() -> String = "{self.name()} with area {self.area():.1}"
}

struct Circle: Shape {
    radius: Float

    fn area() -> Float = pi * self.radius * self.radius
    fn name() -> String = "circle"
}

struct Square: Shape {
    side: Float

    fn area() -> Float = self.side * self.side
    fn name() -> String = "square"
}

// A trait as a parameter type works with any Shape. It's just as fast as
// writing a separate function for each type, because the compiler does that for you.
fn print_summary(shape: Shape) {
    print(shape.summary())
}

print_summary(Circle(radius: 1.0))
print_summary(Square(side: 2.0))

// A list that mixes different types uses `any Shape`
let shapes: List<any Shape> = [Circle(radius: 2.0), Square(side: 3.0)]
let total = shapes.map(s => s.area()).sum()
print("Total area: {total:.1}")

// `extend` adds methods to types you didn't write, even built-in ones
extend Int {
    fn is_even() -> Bool = self % 2 == 0
}

let n = 4
print(n.is_even())

// Leaving out a required method is a compile error:
//     struct Triangle: Shape { base: Float, height: Float }
//     error: `Triangle` says it is a `Shape`, but it's missing `area()` and `name()`.

// Output:
// circle with area 3.1
// square with area 4.0
// Total area: 21.6
// true
