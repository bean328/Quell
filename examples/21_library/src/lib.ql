// A complete, publishable library.
//
//     quell new colors --lib     creates this layout
//     quell test                 runs the tests and every example in the docs below
//     quell doc --open           shows the documentation in your browser
//     quell publish              checks the version number and publishes
//
// `///` comments become the documentation. Code inside them is run as a test,
// so the docs can never go out of date.

/// A color made of red, green and blue parts, each from 0 to 255.
pub struct Color {
    pub r: UInt8
    pub g: UInt8
    pub b: UInt8

    /// Reads a color written in hex, like `"#ff8800"`.
    /// Gives `none` if the text isn't a valid color.
    ///
    /// ```
    /// assert Color.from_hex("#ff8800") == Color(r: 255, g: 136, b: 0)
    /// assert Color.from_hex("orange") == none
    /// ```
    pub static fn from_hex(text: String) -> Color? {
        let hex = text.strip_prefix("#") ?? return none
        if hex.length() != 6 { return none }
        let r = hex.slice(0..<2).to_int(base: 16) ?? return none
        let g = hex.slice(2..<4).to_int(base: 16) ?? return none
        let b = hex.slice(4..<6).to_int(base: 16) ?? return none
        Color(r: UInt8(r), g: UInt8(g), b: UInt8(b))
    }

    /// Writes the color in hex.
    ///
    /// ```
    /// assert Color(r: 255, g: 136, b: 0).to_hex() == "#ff8800"
    /// ```
    pub fn to_hex() -> String = "#{self.r:02x}{self.g:02x}{self.b:02x}"
}

/// Pure red.
pub let red = Color(r: 255, g: 0, b: 0)

/// Pure blue.
pub let blue = Color(r: 0, g: 0, b: 255)

/// Mixes two colors evenly.
///
/// ```
/// assert mix(red, blue) == Color(r: 127, g: 0, b: 127)
/// ```
pub fn mix(a: Color, b: Color) -> Color = Color(
    r: average(a.r, b.r),
    g: average(a.g, b.g),
    b: average(a.b, b.b),
)

// Private helper. Widens to Int first so 255 + 255 can't overflow a UInt8.
fn average(x: UInt8, y: UInt8) -> UInt8 = UInt8((Int(x) + Int(y)) div 2)

test "mixing a color with itself changes nothing" {
    assert mix(red, red) == red
}

// Once this library reaches 1.0, a later release that removed or changed `mix`
// couldn't be published as a small update:
//     $ quell publish        (version 1.1.0)
//     error: this release breaks the public API, so it must be version 2.0.0, not 1.1.0.
//         removed: pub fn mix(a: Color, b: Color) -> Color
