// Optionals: a value that might be missing has a `?` type.
// There is no null in Quell, so "missing" can never surprise you.

struct User {
    name: String
    email: String?
}

let users = [
    User(name: "Ava", email: "ava@example.com"),
    User(name: "Sam", email: none),
]

fn find_user(users: List<User>, name: String) -> User? {
    for user in users {
        if user.name == name { return user }
    }
    return none
}

// 1. `if let`: run code only when the value is there
if let user = find_user(users, "Ava") {
    print("Found {user.name}")
}

// 2. `??`: use a default when it's missing
let email = find_user(users, "Sam")?.email ?? "no email"
print("Sam's email: {email}")

// 3. `?.`: if anything along the way is missing, the result is `none`
let zoe_email = find_user(users, "Zoe")?.email
print(zoe_email)

// 4. `let ... else`: unwrap it, or leave early
fn email_domain(user: User) -> String? {
    let email = user.email else { return none }
    let at = email.find("@") else { return none }
    return email.slice(from: at + 1)
}

print(email_domain(users[0]) ?? "unknown")
print(email_domain(users[1]) ?? "unknown")

// Using an optional as if it were definitely there is a compile error:
//     print(users[1].email.length())
//     error: `email` might be missing (its type is `String?`).
//     help: use `if let`, `??` or `?.` to handle the missing case.

// Output:
// Found Ava
// Sam's email: no email
// none
// example.com
// unknown
