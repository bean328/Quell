// Structs: group related data together, with functions (methods) that belong to it.

struct Player {
    name: String
    health: Int = 100
    level: Int = 1

    fn is_alive() -> Bool = self.health > 0

    fn describe() -> String {
        return "{self.name} (level {self.level}, {self.health} HP)"
    }

    // `mut fn` methods are allowed to change the struct
    mut fn take_damage(amount: Int) {
        self.health = max(0, self.health - amount)
    }

    mut fn level_up() {
        self.level += 1
        self.health = 100
    }

    // `static fn` belongs to the type itself, not to one player
    static fn guest() -> Player = Player(name: "Guest")
}

var hero = Player(name: "Ava")
hero.take_damage(30)
print(hero.describe())

hero.level_up()
print(hero.describe())

let visitor = Player.guest()
print(visitor.describe())
print(hero.is_alive())

// Calling a `mut fn` on a `let` value is a compile error:
//     visitor.take_damage(10)
//     error: `take_damage` changes `visitor`, but `visitor` was declared with `let`.
//     help: declare it with `var visitor = ...` if it should change.

// Output:
// Ava (level 1, 70 HP)
// Ava (level 2, 100 HP)
// Guest (level 1, 100 HP)
// true
