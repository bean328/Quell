import std.math.{sqrt}

/// A position in 2D space.
pub struct Point {
    pub x: Float
    pub y: Float
}

/// The straight-line distance between two points.
pub fn distance(a: Point, b: Point) -> Float {
    let dx = b.x - a.x
    let dy = b.y - a.y
    sqrt(square(dx) + square(dy))
}

// No `pub`, so only this file can use it
fn square(x: Float) -> Float = x * x
