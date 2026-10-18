// A small web server, using the official http and json packages:
//     quell add http json
//     quell run 25_web_server.ql
//
// Each request runs as its own lightweight task. While nobody is visiting,
// the server sleeps and uses no CPU at all.

import http.{Server, Request, Response}
import json

@derive(Json)
struct Greeting {
    message: String
    visitor: Int
}

fn handle(request: Request, visits: Atomic<Int>) -> Response = match (request.method, request.path) {
    (Get, "/") => Response.text("Welcome to Quell!")
    (Get, "/hello") => {
        let name = request.query("name") ?? "stranger"
        let number = visits.add(1)
        Response.json(Greeting(message: "Hello, {name}!", visitor: number))
    }
    _ => Response.not_found()
}

let visits = Atomic(0)
let server = Server(port: 8080)
print("Listening on http://localhost:8080")

try server.serve(request => handle(request, visits))

// Output:
// Listening on http://localhost:8080
//
// $ curl "localhost:8080/hello?name=Ava"
// {"message": "Hello, Ava!", "visitor": 1}
