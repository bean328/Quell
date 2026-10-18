// Files, and cleanup that happens by itself.

import std.fs

// Files close themselves when they're no longer used. There's no `close()` to forget.
fn write_report(path: String, lines: List<String>) throws {
    var file = try fs.create(path)
    for line in lines {
        try file.write_line(line)
    }
}   // the file is closed here, even if an error happened above

fn count_words(path: String) -> Int throws {
    let text = try fs.read_text(path)
    text.split_whitespace().length()
}

// `defer` runs something when the current block ends, however it ends
fn make_report() throws {
    print("starting")
    defer { print("finished (always printed)") }

    try write_report("report.txt", ["Quell is fast", "Quell is safe"])
    let words = try count_words("report.txt")
    print("words: {words}")
}

try make_report()
try fs.remove("report.txt")

// Output:
// starting
// words: 6
// finished (always printed)
