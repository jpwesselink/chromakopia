use colored::Colorize;

use crate::color::Color;

/// How to interpolate between color stops.
#[derive(Debug, Clone, Copy, Default)]
pub enum Interpolation {
    #[default]
    Rgb,
    Hsv,
}

/// Direction around the hue wheel when using HSV interpolation.
#[derive(Debug, Clone, Copy, Default)]
pub enum HsvSpin {
    #[default]
    Short,
    Long,
}

/// A gradient that can be applied to strings.
#[derive(Debug, Clone)]
pub struct Gradient {
    stops: Vec<Color>,
    positions: Option<Vec<f64>>,
    interpolation: Interpolation,
    hsv_spin: HsvSpin,
}

impl Gradient {
    /// Create a new gradient from a list of color stops.
    /// At least 2 colors are required.
    pub fn new(stops: Vec<Color>) -> Self {
        assert!(stops.len() >= 2, "gradient needs at least 2 colors");
        Self {
            stops,
            positions: None,
            interpolation: Interpolation::Rgb,
            hsv_spin: HsvSpin::Short,
        }
    }

    /// Create a gradient with explicit positions for each color stop.
    /// Positions should be in the range 0.0..=1.0.
    pub fn new_with_positions(stops: Vec<(Color, f64)>) -> Self {
        assert!(stops.len() >= 2, "gradient needs at least 2 colors");
        let (colors, positions): (Vec<_>, Vec<_>) = stops.into_iter().unzip();
        Self {
            stops: colors,
            positions: Some(positions),
            interpolation: Interpolation::Rgb,
            hsv_spin: HsvSpin::Short,
        }
    }

    /// Use HSV interpolation (short arc by default).
    pub fn hsv(mut self) -> Self {
        self.interpolation = Interpolation::Hsv;
        self
    }

    /// Use RGB interpolation (the default).
    pub fn rgb(mut self) -> Self {
        self.interpolation = Interpolation::Rgb;
        self
    }

    /// Use the long arc around the hue wheel (implies HSV).
    pub fn long(mut self) -> Self {
        self.interpolation = Interpolation::Hsv;
        self.hsv_spin = HsvSpin::Long;
        self
    }

    /// Use the short arc around the hue wheel.
    pub fn short(mut self) -> Self {
        self.hsv_spin = HsvSpin::Short;
        self
    }

    /// Generate a palette of `n` colors distributed across the gradient.
    pub fn palette(&self, n: usize) -> Vec<Color> {
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![self.stops[0]];
        }

        let long = matches!(self.hsv_spin, HsvSpin::Long);
        let mut colors = Vec::with_capacity(n);

        // Build position array (either custom or evenly spaced)
        let positions: Vec<f64> = match &self.positions {
            Some(p) => p.clone(),
            None => {
                let segments = self.stops.len() - 1;
                (0..self.stops.len())
                    .map(|i| i as f64 / segments as f64)
                    .collect()
            }
        };

        for i in 0..n {
            let t = i as f64 / (n - 1) as f64;

            // Find which segment this t falls in
            let mut seg = 0;
            for (j, &pos) in positions[..positions.len() - 1].iter().enumerate() {
                if t >= pos {
                    seg = j;
                }
            }
            let seg = seg.min(self.stops.len() - 2);

            let seg_start = positions[seg];
            let seg_end = positions[seg + 1];
            let seg_len = seg_end - seg_start;
            let local_t = if seg_len > 0.0 {
                ((t - seg_start) / seg_len).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let a = self.stops[seg];
            let b = self.stops[seg + 1];

            let c = match self.interpolation {
                Interpolation::Rgb => Color::lerp_rgb(a, b, local_t),
                Interpolation::Hsv => Color::lerp_hsv(a, b, local_t, long),
            };
            colors.push(c);
        }

        colors
    }

    /// Apply the gradient to a single-line string.
    ///
    /// Whitespace characters are preserved without consuming a color,
    /// so the gradient flows smoothly across visible characters only.
    pub fn apply(&self, text: &str) -> String {
        let visible: usize = text.chars().filter(|c| !c.is_whitespace()).count();
        let n = visible.max(self.stops.len());
        let palette = self.palette(n);

        let mut result = String::new();
        let mut color_idx = 0;

        for ch in text.chars() {
            if ch.is_whitespace() {
                result.push(ch);
            } else {
                let c = palette[color_idx];
                let colored = ch.to_string().truecolor(c.r, c.g, c.b);
                result.push_str(&colored.to_string());
                color_idx += 1;
            }
        }

        result
    }

    /// Apply the gradient to a multiline string.
    ///
    /// Colors are assigned by column position, so the gradient stays
    /// vertically aligned across lines — ideal for ASCII art.
    pub fn multiline(&self, text: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let max_len = lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0)
            .max(self.stops.len());
        let palette = self.palette(max_len);

        lines
            .iter()
            .map(|line| {
                line.chars()
                    .enumerate()
                    .map(|(i, ch)| {
                        let c = palette[i];
                        ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_gradient() {
        colored::control::set_override(true);
        let g = Gradient::new(vec![Color::new(255, 0, 0), Color::new(0, 0, 255)]);
        let result = g.apply("Hello");
        // Should contain ANSI escape codes
        assert!(result.contains("\x1b["));
        // Should contain all original characters
        for ch in "Hello".chars() {
            assert!(result.contains(ch));
        }
    }

    #[test]
    fn whitespace_preserved() {
        let g = Gradient::new(vec![Color::new(255, 0, 0), Color::new(0, 0, 255)]);
        let result = g.apply("a b c");
        // Spaces should be present without ANSI codes around them
        assert!(result.contains(' '));
    }

    #[test]
    fn multiline_works() {
        let g = Gradient::new(vec![Color::new(255, 0, 0), Color::new(0, 0, 255)]);
        let result = g.multiline("ab\ncd");
        let lines: Vec<&str> = result.split('\n').collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn palette_endpoints() {
        let g = Gradient::new(vec![Color::new(0, 0, 0), Color::new(255, 255, 255)]);
        let p = g.palette(5);
        assert_eq!(p.len(), 5);
        assert_eq!(p[0], Color::new(0, 0, 0));
        assert_eq!(p[4], Color::new(255, 255, 255));
    }

    #[test]
    fn three_stop_palette() {
        let g = Gradient::new(vec![
            Color::new(255, 0, 0),
            Color::new(0, 255, 0),
            Color::new(0, 0, 255),
        ]);
        let p = g.palette(5);
        assert_eq!(p[0], Color::new(255, 0, 0));
        assert_eq!(p[2], Color::new(0, 255, 0));
        assert_eq!(p[4], Color::new(0, 0, 255));
    }
}
