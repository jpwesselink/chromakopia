mod color;
mod gradient;
mod terminal;
pub mod animate;
pub mod presets;

pub use color::Color;
pub use gradient::{Gradient, HsvSpin, Interpolation};
pub use terminal::{bg_color, fg_color, set_bg_color, set_fg_color};

/// Create a gradient from a slice of colors.
///
/// Colors can be hex strings like `"#ff0000"`, CSS-style `"rgb(255,0,0)"`,
/// or named colors like `"red"`.
///
/// ```
/// use shimmer::gradient;
/// let text = gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello, world!");
/// ```
pub fn gradient(colors: &[&str]) -> Gradient {
    let stops: Vec<Color> = colors
        .iter()
        .map(|c| c.parse::<Color>().expect("invalid color"))
        .collect();
    Gradient::new(stops)
}
