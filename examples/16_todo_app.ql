// A small command-line to-do list that saves to a file.
// Brings together structs, enums, match, optionals, errors and files.
//
//     quell run 16_todo_app.ql add "Buy milk"
//     quell run 16_todo_app.ql done 1
//     quell run 16_todo_app.ql list

import std.fs
import std.env

struct Task {
    title: String
    done: Bool = false

    fn to_line() -> String {
        let mark = if self.done { "[x]" } else { "[ ]" }
        "{mark} {self.title}"
    }
}

enum Command {
    Add(title: String)
    Done(number: Int)
    ListAll
}

fn parse_command(args: List<String>) -> Command? = match args {
    ["add", title] => Add(title: title)
    ["done", text] => Done(number: text.to_int() ?? return none)
    ["list"] or [] => ListAll
    _ => none
}

fn load(path: String) -> List<Task> {
    let text = fs.read_text(path) catch ""     // no file yet means no tasks
    var tasks: List<Task> = []
    for line in text.lines() {
        if line.is_empty() { continue }
        tasks.append(Task(title: line.slice(from: 4), done: line.starts_with("[x]")))
    }
    tasks
}

fn save(tasks: List<Task>, path: String) throws {
    let lines = tasks.map(task => task.to_line())
    try fs.write_text(path, lines.join("\n"))
}

let save_file = "todo.txt"
var tasks = load(save_file)

let command = parse_command(env.args())
if command == none {
    print("Usage: todo add <title> | done <number> | list")
    exit(1)
}

match command {
    Add(title) => {
        tasks.append(Task(title: title))
        print("Added: {title}")
    }
    Done(number) => {
        if number < 1 or number > tasks.length() {
            print("There is no task {number}.")
            exit(1)
        }
        tasks[number - 1].done = true
        print("Done: {tasks[number - 1].title}")
    }
    ListAll => {
        for (i, task) in tasks.enumerate() {
            print("{i + 1}. {task.to_line()}")
        }
    }
}

try save(tasks, save_file)

// Output (after `add "Buy milk"`, `add "Walk the dog"`, `done 1`, then `list`):
// 1. [x] Buy milk
// 2. [ ] Walk the dog
