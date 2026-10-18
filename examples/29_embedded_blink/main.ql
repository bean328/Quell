// Blinks the LED on a Raspberry Pi Pico. The same language, on a tiny chip.
//
//     quell build --release    the whole program is a few kilobytes

import rp2040.{Pin, delay}

// Without an operating system there are no scripts, so the start is marked
// with @entry. `Never` means this function never returns.
@entry
fn main() -> Never {
    var led = Pin.output(25)
    repeat {
        led.toggle()
        delay(ms: 500)
    }
}

// Output:
// (the LED turns on and off every half second)
