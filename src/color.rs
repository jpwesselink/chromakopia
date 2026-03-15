use std::fmt;
use std::str::FromStr;

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to HSV (h: 0..360, s: 0..1, v: 0..1).
    pub fn to_hsv(self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }

    /// Create from HSV (h: 0..360, s: 0..1, v: 0..1).
    pub fn from_hsv(h: f64, s: f64, v: f64) -> Self {
        let h = ((h % 360.0) + 360.0) % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self {
            r: ((r + m) * 255.0).round() as u8,
            g: ((g + m) * 255.0).round() as u8,
            b: ((b + m) * 255.0).round() as u8,
        }
    }

    /// Linearly interpolate in RGB space.
    pub fn lerp_rgb(a: Color, b: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (a.r as f64 + (b.r as f64 - a.r as f64) * t).round() as u8,
            g: (a.g as f64 + (b.g as f64 - a.g as f64) * t).round() as u8,
            b: (a.b as f64 + (b.b as f64 - a.b as f64) * t).round() as u8,
        }
    }

    /// Linearly interpolate in HSV space.
    /// `long` controls whether to take the long arc around the hue wheel.
    pub fn lerp_hsv(a: Color, b: Color, t: f64, long: bool) -> Color {
        let t = t.clamp(0.0, 1.0);
        let (h1, s1, v1) = a.to_hsv();
        let (h2, s2, v2) = b.to_hsv();

        let mut dh = h2 - h1;

        if long {
            // Take the long way around the hue wheel
            if dh > 0.0 && dh < 180.0 {
                dh -= 360.0;
            } else if dh > -180.0 && dh <= 0.0 {
                dh += 360.0;
            }
        } else {
            // Take the short way around the hue wheel
            if dh > 180.0 {
                dh -= 360.0;
            } else if dh < -180.0 {
                dh += 360.0;
            }
        }

        let h = h1 + dh * t;
        let s = s1 + (s2 - s1) * t;
        let v = v1 + (v2 - v1) * t;

        Color::from_hsv(h, s, v)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Hex: #RGB, #RRGGBB
        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex);
        }

        // Also accept without #
        if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            return parse_hex(s);
        }

        // rgb(r, g, b)
        if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 3 {
                let r = parts[0].trim().parse::<u8>().map_err(|e| e.to_string())?;
                let g = parts[1].trim().parse::<u8>().map_err(|e| e.to_string())?;
                let b = parts[2].trim().parse::<u8>().map_err(|e| e.to_string())?;
                return Ok(Color::new(r, g, b));
            }
        }

        // Named colors
        match s.to_lowercase().as_str() {
            "red" => Ok(Color::new(255, 0, 0)),
            "green" => Ok(Color::new(0, 128, 0)),
            "blue" => Ok(Color::new(0, 0, 255)),
            "cyan" => Ok(Color::new(0, 255, 255)),
            "magenta" | "fuchsia" => Ok(Color::new(255, 0, 255)),
            "yellow" => Ok(Color::new(255, 255, 0)),
            "white" => Ok(Color::new(255, 255, 255)),
            "black" => Ok(Color::new(0, 0, 0)),
            "orange" => Ok(Color::new(255, 165, 0)),
            "pink" => Ok(Color::new(255, 192, 203)),
            "purple" => Ok(Color::new(128, 0, 128)),
            "gold" => Ok(Color::new(255, 215, 0)),
            "coral" => Ok(Color::new(255, 127, 80)),
            "lime" => Ok(Color::new(0, 255, 0)),
            "navy" => Ok(Color::new(0, 0, 128)),
            "teal" => Ok(Color::new(0, 128, 128)),
            "indigo" => Ok(Color::new(75, 0, 130)),
            "violet" => Ok(Color::new(238, 130, 238)),
            _ => Err(format!("unknown color: {s}")),
        }
    }
}

fn parse_hex(hex: &str) -> Result<Color, String> {
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).map_err(|e| e.to_string())? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).map_err(|e| e.to_string())? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).map_err(|e| e.to_string())? * 17;
            Ok(Color::new(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
            Ok(Color::new(r, g, b))
        }
        _ => Err(format!("invalid hex color: #{hex}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_colors() {
        assert_eq!("#ff0000".parse::<Color>().unwrap(), Color::new(255, 0, 0));
        assert_eq!("#0f0".parse::<Color>().unwrap(), Color::new(0, 255, 0));
        assert_eq!("4bc0c8".parse::<Color>().unwrap(), Color::new(75, 192, 200));
    }

    #[test]
    fn parse_named() {
        assert_eq!("red".parse::<Color>().unwrap(), Color::new(255, 0, 0));
        assert_eq!("Cyan".parse::<Color>().unwrap(), Color::new(0, 255, 255));
    }

    #[test]
    fn parse_rgb_func() {
        assert_eq!(
            "rgb(10, 20, 30)".parse::<Color>().unwrap(),
            Color::new(10, 20, 30)
        );
    }

    #[test]
    fn lerp_midpoint() {
        let c = Color::lerp_rgb(Color::new(0, 0, 0), Color::new(255, 255, 255), 0.5);
        assert_eq!(c, Color::new(128, 128, 128));
    }

    #[test]
    fn hsv_roundtrip() {
        let original = Color::new(200, 100, 50);
        let (h, s, v) = original.to_hsv();
        let restored = Color::from_hsv(h, s, v);
        assert!((original.r as i16 - restored.r as i16).abs() <= 1);
        assert!((original.g as i16 - restored.g as i16).abs() <= 1);
        assert!((original.b as i16 - restored.b as i16).abs() <= 1);
    }
}
