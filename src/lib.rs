mod color;
mod gradient;
mod terminal;
pub mod animate;
pub mod presets;

pub use color::Color;
pub use gradient::{Gradient, HsvSpin, Interpolation};
pub use terminal::{bg_color, fg_color, is_dark_theme, is_light_theme, probe_colors, set_bg_color, set_fg_color, terminal_width};

/// Create a gradient from a slice of colors.
///
/// Colors can be hex strings like `"#ff0000"`, CSS-style `"rgb(255,0,0)"`,
/// or named colors like `"red"`.
///
/// ```
/// use chromakopia::gradient;
/// let text = gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello, world!");
/// ```
pub fn gradient(colors: &[&str]) -> Gradient {
    let stops: Vec<Color> = colors
        .iter()
        .map(|c| c.parse::<Color>().expect("invalid color"))
        .collect();
    Gradient::new(stops)
}

/// Pad each line with spaces on the right to fill the terminal width.
pub fn pad(text: &str) -> String {
    let w = terminal::terminal_width();
    text.lines()
        .map(|line| {
            let len = line.chars().count();
            if len < w {
                format!("{}{}", line, " ".repeat(w - len))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Center each line within the terminal width by padding left and right.
pub fn center(text: &str) -> String {
    let w = terminal::terminal_width();
    text.lines()
        .map(|line| {
            let len = line.chars().count();
            if len < w {
                let left = (w - len) / 2;
                let right = w - len - left;
                format!("{}{}{}", " ".repeat(left), line, " ".repeat(right))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
