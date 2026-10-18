// Lambdas (small inline functions) and chaining list operations.

struct Movie {
    title: String
    year: Int
    rating: Float
}

let movies = [
    Movie(title: "Spirited Away", year: 2001, rating: 8.6),
    Movie(title: "Up", year: 2009, rating: 8.3),
    Movie(title: "Coco", year: 2017, rating: 8.4),
    Movie(title: "Cars 2", year: 2011, rating: 6.2),
]

// Chains read top to bottom, like a recipe.
// A line starting with `.` continues the line above.
let top_titles = movies
    .filter(m => m.rating >= 8.0)
    .sort_by(m => m.rating, descending: true)
    .map(m => "{m.title} ({m.year})")

for title in top_titles {
    print(title)
}

let from_2010s = movies.count(m => m.year >= 2010 and m.year < 2020)
print("From the 2010s: {from_2010s}")

let best = movies.max_by(m => m.rating)
if best != none {
    print("Best: {best.title}")
}

// Lambdas can read variables from around them
let min_year = 2005
let recent = movies.filter(m => m.year >= min_year).map(m => m.title)
print(recent)

// A lambda stored in a variable
let shout = (text: String) => text.upper() + "!"
print(shout("quell"))

// A lambda with a block: the last line is the result
let describe = (m: Movie) => {
    let age = 2026 - m.year
    "{m.title} is {age} years old"
}
print(describe(movies[0]))

// Functions can take functions. `(Int) -> Int` means "takes an Int, gives an Int".
fn apply_twice(x: Int, f: (Int) -> Int) -> Int = f(f(x))
print(apply_twice(3, n => n * 10))

// Output:
// Spirited Away (2001)
// Coco (2017)
// Up (2009)
// From the 2010s: 2
// Best: Spirited Away
// ["Up", "Coco", "Cars 2"]
// QUELL!
// Spirited Away is 25 years old
// 300
