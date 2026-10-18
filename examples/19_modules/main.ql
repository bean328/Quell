// A program split across several files. Each file is a module, and
// folders group modules: `geometry/point.ql` is imported as `geometry.point`.
//
//     19_modules/
//     ├── main.ql              this file, where the program starts
//     └── geometry/
//         ├── point.ql
//         └── shapes.ql
//
// Run it with:  quell run 19_modules/main.ql

import geometry.point.{Point, distance}
import geometry.shapes.{Rect}

let a = Point(x: 0.0, y: 0.0)
let b = Point(x: 3.0, y: 4.0)
print(distance(a, b))

let box = Rect(origin: a, width: 2.0, height: 5.0)
print(box.area())

// Using something that isn't `pub` is a compile error:
//     geometry.point.square(2.0)
//     error: `square` is private to geometry/point.ql.
//     help: mark it `pub fn` in that file if other files should use it.

// Output:
// 5.0
// 10.0
