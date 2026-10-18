// Sharing data. Most Quell code never needs this file's tools.
// They're for when several parts of a program really need the same data.

// 1. Shared: many owners, read-only, never copied
struct Config {
    app_name: String
    max_users: Int
}

let config = Shared(Config(app_name: "Chat", max_users: 100))
let server_config = config      // no copy: both point to the same data
print(server_config.app_name)

// 2. Atomic: a number that many tasks can change safely
let visits = Atomic(0)
group {
    repeat 1000 {
        spawn { visits.add(1) }
    }
}
print("visits: {visits.get()}")

// 3. Mutex: shared data bigger than a number. Only one task can use it at a time.
// `lock` waits its turn, then hands the lambda the data.
// `mut entries` shows that this lambda changes it.
let log = Mutex(List<String>())
group {
    for name in ["Ava", "Sam", "Kai"] {
        spawn {
            log.lock(mut entries => entries.append("{name} joined"))
        }
    }
}
print("log has {log.lock(entries => entries.length())} entries")

// 4. Arena: the easy, fast way to build things that point at each other,
// like maps, graphs, or game worlds. Handles are safe: they can't dangle.
struct Room {
    name: String
    exits: List<Handle<Room>> = []
}

var world = Arena<Room>()
let hall = world.add(Room(name: "Hall"))
let kitchen = world.add(Room(name: "Kitchen"))
let garden = world.add(Room(name: "Garden"))

world[hall].exits.append(kitchen)
world[kitchen].exits.append(garden)
world[garden].exits.append(hall)       // a loop back to the start is fine

for exit in world[hall].exits {
    print("From the hall you can go to the {world[exit].name}.")
}

// Output:
// Chat
// visits: 1000
// log has 3 entries
// From the hall you can go to the Kitchen.
