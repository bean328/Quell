// Doing many things at once, safely.

import std.time

fn slow_square(n: Int) -> Int {
    time.sleep(ms: 100)     // pretend this is heavy work
    n * n
}

// `parallel for` spreads the work over all CPU cores
// and gives back the results in the original order.
let numbers = [1, 2, 3, 4, 5, 6, 7, 8]
let squares = parallel for n in numbers {
    slow_square(n)
}
print(squares)

// `group` runs tasks at the same time. The group only ends when every
// task inside it has finished, so no task is ever forgotten.
fn download(name: String) -> String {
    time.sleep(ms: 200)
    "{name} downloaded"
}

group {
    let photos = spawn download("photos.zip")
    let music = spawn download("music.zip")
    print(photos.wait())
    print(music.wait())
}

// Channels let tasks send each other messages
let (sender, receiver) = channel<String>()

group {
    spawn {
        for i in 1..=3 {
            sender.send("message {i}")
        }
        sender.close()
    }
    for message in receiver {
        print("got {message}")
    }
}

// Tasks can read variables from around them, but can't change them.
// That's why data races are impossible. This does not compile:
//     var total = 0
//     parallel for n in numbers { total += n }
//     error: tasks running at the same time can't change `total`.
//     help: return values instead (`parallel for` collects them), or use `Atomic`.
let total = squares.sum()
print("total: {total}")

// Output:
// [1, 4, 9, 16, 25, 36, 49, 64]
// photos.zip downloaded
// music.zip downloaded
// got message 1
// got message 2
// got message 3
// total: 204
