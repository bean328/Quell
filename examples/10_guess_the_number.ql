// A complete little game: guess the secret number.
// Uses loops, `match`, and checking optionals (see 13_optionals.ql).

import std.random

let secret = random.int(1..=100)
var guesses = 0

print("I'm thinking of a number between 1 and 100.")

repeat {
    // read_line gives `none` when there's no more input
    let line = read_line("Your guess: ")
    if line == none {
        print("Bye!")
        break
    }

    let guess = line.trim().to_int()
    if guess == none {
        print("Please type a whole number.")
        continue
    }
    guesses += 1

    match guess.compare(secret) {
        Less => print("Too low!")
        Greater => print("Too high!")
        Equal => {
            print("You got it in {guesses} guesses!")
            break
        }
    }
}

// Output (interactive, one possible game):
// I'm thinking of a number between 1 and 100.
// Your guess: 50
// Too high!
// Your guess: 25
// Too low!
// Your guess: 37
// You got it in 3 guesses!
