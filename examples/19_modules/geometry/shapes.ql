import geometry.point.{Point}

/// A rectangle, placed at `origin`.
pub struct Rect {
    pub origin: Point
    pub width: Float
    pub height: Float

    /// How much space the rectangle covers.
    pub fn area() -> Float = self.width * self.height
}
