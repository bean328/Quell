// Enums: a value that is exactly one of several options.
// `match`: do something different for each option.

enum Weather {
    Sunny
    Cloudy
    Rainy(mm: Float)
    Snowy(cm: Float)
}

// `match` must handle every option. Forget one and it won't compile.
fn advice(weather: Weather) -> String = match weather {
    Sunny => "Sunglasses!"
    Cloudy => "Maybe a jacket."
    Rainy(mm) if mm > 10.0 => "Stay inside, it's pouring."
    Rainy(_) => "Take an umbrella."
    Snowy(cm) => "Build a snowman with {cm} cm of snow!"
}

// When the type is already known, `Rainy` is enough. `Weather.Rainy` also works.
let week: List<Weather> = [Sunny, Rainy(mm: 2.5), Rainy(mm: 25.0), Snowy(cm: 8.0), Cloudy]

for day in week {
    print(advice(day))
}

// Matching numbers, ranges, and several options at once
fn day_type(day: Int) -> String = match day {
    1..=5 => "weekday"
    6 or 7 => "weekend"
    _ => "not a day"
}

print(day_type(3))
print(day_type(7))
print(day_type(9))

// Leaving out an option is a compile error:
//     match weather {
//         Sunny => ...
//         Cloudy => ...
//     }
//     error: this `match` doesn't handle `Rainy` and `Snowy`.
//     help: add the missing options, or `_ => ...` to handle everything else.

// Output:
// Sunglasses!
// Take an umbrella.
// Stay inside, it's pouring.
// Build a snowman with 8.0 cm of snow!
// Maybe a jacket.
// weekday
// weekend
// not a day
