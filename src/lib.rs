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

/// Center a text block within the terminal width.
///
/// Uses the widest line to compute a single left-pad, then applies
/// the same offset to every line so multiline text (like figlet banners)
/// stays aligned as a block.
pub fn center(text: &str) -> String {
    let w = terminal::terminal_width();
    let max_line_width = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let left = if max_line_width < w { (w - max_line_width) / 2 } else { 0 };
    let pad_left = " ".repeat(left);
    text.lines()
        .map(|line| {
            let len = line.chars().count();
            let right = w.saturating_sub(left + len);
            format!("{}{}{}", pad_left, line, " ".repeat(right))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
