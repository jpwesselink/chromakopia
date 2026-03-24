use colored::Colorize;
use rand::Rng;

use crate::color::Color;
use crate::gradient::Gradient;

/// Render a rainbow frame: full-spectrum HSV gradient that shifts each frame.
pub fn rainbow(text: &str, frame: usize) -> String {
    let hue = (frame * 5 % 360) as f64;
    let left = Color::from_hsv(hue, 1.0, 1.0);
    let right = Color::from_hsv((hue + 1.0) % 360.0, 1.0, 1.0);
    Gradient::new(vec![left, right]).long().multiline(text)
}

/// Render a pulse frame: red highlight expands from center then contracts.
pub fn pulse(text: &str, frame: usize) -> String {
    let cycle = (frame % 120) + 1;
    let transition = 6;
    let duration = 10;

    let on = Color::new(0xff, 0x10, 0x10);
    let off = Color::new(0xe6, 0xe6, 0xe6);

    if cycle <= transition {
        // Expanding phase
        let progress = cycle as f64 / transition as f64;
        let half = progress / 2.0;
        let gradient = if progress <= 0.5 {
            Gradient::new_with_positions(vec![
                (off, 0.0),
                (off, 0.5 - half),
                (on, 0.5),
                (off, 0.5 + half),
                (off, 1.0),
            ])
        } else {
            Gradient::new_with_positions(vec![
                (on, 0.0),
                (off, 0.5 - half),
                (on, 0.5),
                (off, 0.5 + half),
                (on, 1.0),
            ])
        };
        gradient.multiline(text)
    } else if cycle <= transition + duration {
        // Solid red phase
        apply_solid(text, on)
    } else if cycle <= transition * 2 + duration {
        // Contracting phase (reverse of expanding)
        let progress = (transition * 2 + duration - cycle + 1) as f64 / transition as f64;
        let half = progress / 2.0;
        let gradient = if progress <= 0.5 {
            Gradient::new_with_positions(vec![
                (off, 0.0),
                (off, 0.5 - half),
                (on, 0.5),
                (off, 0.5 + half),
                (off, 1.0),
            ])
        } else {
            Gradient::new_with_positions(vec![
                (on, 0.0),
                (off, 0.5 - half),
                (on, 0.5),
                (off, 0.5 + half),
                (on, 1.0),
            ])
        };
        gradient.multiline(text)
    } else {
        // Off phase
        apply_solid(text, off)
    }
}

/// Render a glitch frame: random character corruption and blanking.
pub fn glitch(text: &str, frame: usize) -> String {
    let blackout =
        (frame % 2) + (frame % 3) + (frame % 11) + (frame % 29) + (frame % 37) > 52;
    if blackout {
        return " ".repeat(text.len());
    }

    let mut rng = rand::rng();
    let glitch_chars: &[u8] = b"x*0987654321[]0-~@#(____!!!!\\|?????....0000\t";
    let chunk_size = (text.len() / 50).max(3);
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Random chance to skip a chunk
        if rng.random_range(0..100) < 2 && i + chunk_size < chars.len() {
            result.extend(std::iter::repeat_n(' ', chunk_size));
            i += chunk_size;
            continue;
        }

        // Random chance to insert a glitch character
        if rng.random_range(0..1000) < 5 {
            let gc = glitch_chars[rng.random_range(0..glitch_chars.len())] as char;
            result.push(gc);
            i += 1;
            continue;
        }

        // Random chance to drop a character
        if rng.random_range(0..1000) < 5 {
            i += 1;
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    // Random case change
    if rng.random_range(0..100) < 1 {
        result = result.to_uppercase();
    } else if rng.random_range(0..100) < 1 {
        result = result.to_lowercase();
    }

    result
}

/// Render a radar frame: a spotlight sweeps across the text.
pub fn radar(text: &str, frame: usize) -> String {
    let len = text.chars().count();
    if len == 0 {
        return String::new();
    }
    let depth = (len as f64 * 0.2).floor() as usize;
    let depth = depth.max(1).min(len);
    let step = 255 / depth;
    let global_pos = frame % (len + depth);

    text.chars()
        .enumerate()
        .map(|(i, ch)| {
            let dist_from_head = global_pos as isize - i as isize;
            if dist_from_head >= 0 && (dist_from_head as usize) < depth {
                let brightness = 255 - (dist_from_head as usize * step);
                let b = brightness as u8;
                ch.to_string().truecolor(b, b, b).to_string()
            } else {
                " ".to_string()
            }
        })
        .collect()
}

/// Render a neon frame: flickering between dim and bright.
pub fn neon(text: &str, frame: usize) -> String {
    if frame.is_multiple_of(2) {
        // Dim
        apply_solid(text, Color::new(88, 80, 85))
    } else {
        // Bright bold
        let colored = text.truecolor(213, 70, 242).bold();
        colored.to_string()
    }
}

/// Render a karaoke frame: progressive highlight from left to right.
pub fn karaoke(text: &str, frame: usize) -> String {
    let len = text.chars().count();
    let cursor = (frame % (len + 20)) as isize - 10;

    if cursor < 0 {
        return apply_solid(text, Color::new(255, 255, 255));
    }

    let cursor = cursor as usize;
    let mut result = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i < cursor {
            result.push_str(&ch.to_string().truecolor(255, 187, 0).bold().to_string());
        } else {
            result.push_str(&ch.to_string().truecolor(255, 255, 255).to_string());
        }
    }
    result
}

/// Split-flap departure board: all words flip simultaneously,
/// characters within each word stagger left to right.
pub fn flap(text: &str, frame: usize, settled: Color, flipping: Color) -> String {
    const FLAP_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-.:";
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    // Find the longest word to determine cycle length
    let max_word_len = text.split_whitespace().map(|w| w.len()).max().unwrap_or(1);
    let flips_per_char = 4;
    let stagger = 2;
    let total_frames = max_word_len * stagger + flips_per_char;
    let cycle_frame = frame.min(total_frames); // don't loop, hold when settled

    // Track position within the current word
    let mut rng = rand::rng();
    let mut result = String::new();
    let mut pos_in_word: usize = 0;

    for &target in &chars {
        if target.is_whitespace() {
            result.push(target);
            pos_in_word = 0;
            continue;
        }

        let char_start = pos_in_word * stagger;
        let frames_in = cycle_frame as isize - char_start as isize;

        if frames_in < 0 || (frames_in as usize) < flips_per_char {
            let rc = FLAP_CHARS[rng.random_range(0..FLAP_CHARS.len())] as char;
            result.push_str(&rc.to_string().truecolor(flipping.r, flipping.g, flipping.b).to_string());
        } else {
            result.push_str(&target.to_string().truecolor(settled.r, settled.g, settled.b).to_string());
        }

        pos_in_word += 1;
    }

    result
}

/// Fade in: text goes from invisible to full color over `total_frames` frames.
pub fn fade_in(text: &str, frame: usize, total_frames: usize, target: Color) -> String {
    let t = (frame as f64 / total_frames.max(1) as f64).min(1.0);
    let c = Color::lerp_rgb(Color::new(0, 0, 0), target, t);

    text.split('\n').map(|line| {
        line.chars().map(|ch| {
            ch.to_string().truecolor(c.r, c.g, c.b).to_string()
        }).collect::<String>()
    }).collect::<Vec<_>>().join("\n")
}

/// Fade out: text goes from full color to invisible over `total_frames` frames.
pub fn fade_out(text: &str, frame: usize, total_frames: usize, target: Color) -> String {
    let t = (frame as f64 / total_frames.max(1) as f64).min(1.0);
    let c = Color::lerp_rgb(target, Color::new(0, 0, 0), t);

    text.split('\n').map(|line| {
        line.chars().map(|ch| {
            ch.to_string().truecolor(c.r, c.g, c.b).to_string()
        }).collect::<String>()
    }).collect::<Vec<_>>().join("\n")
}

/// Demoscene plasma: overlapping sine waves create a flowing 2D color field.
/// Each character's color is computed from its (x, y) position and time.
/// Uses a gradient palette for colors. Pass `None` for rainbow HSV.
pub fn plasma(text: &str, frame: usize, palette: Option<&[Color]>) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let t = frame as f64 * 0.08;

    lines
        .iter()
        .enumerate()
        .map(|(y, line)| {
            let yf = y as f64;
            line.chars()
                .enumerate()
                .map(|(x, ch)| {
                    let xf = x as f64;

                    // Four overlapping sine planes — the classic plasma recipe
                    let v1 = (xf * 0.08 + t).sin();
                    let v2 = (yf * 0.12 + t * 0.6).sin();
                    let v3 = ((xf * 0.06 + yf * 0.08 + t * 0.4).sin()
                        + (xf * 0.04 - yf * 0.06 + t * 0.7).cos())
                        * 0.5;
                    // Radial ripple from center
                    let cx = xf - 30.0;
                    let cy = (yf - 5.0) * 2.5; // exaggerate y for an elongated ripple
                    let v4 = ((cx * cx + cy * cy).sqrt() * 0.12 - t * 1.2).sin();

                    let v = (v1 + v2 + v3 + v4) * 0.25; // -1..1

                    let c = if let Some(pal) = palette {
                        if pal.is_empty() {
                            Color::new(0, 0, 0)
                        } else if pal.len() == 1 {
                            pal[0]
                        } else {
                            // Map -1..1 → 0..1, then cycle through palette
                            let idx = ((v + 1.0) * 0.5 + t * 0.05).rem_euclid(1.0);
                            let fi = idx * (pal.len() - 1) as f64;
                            let lo = (fi.floor() as usize).min(pal.len() - 1);
                            let hi = (lo + 1).min(pal.len() - 1);
                            let frac = fi - lo as f64;
                            Color::lerp_rgb(pal[lo], pal[hi], frac)
                        }
                    } else {
                        // Rainbow HSV
                        let hue = ((v + 1.0) * 180.0 + t * 20.0) % 360.0;
                        let sat = 0.8 + 0.2 * (v * std::f64::consts::PI).cos();
                        let val = 0.7 + 0.3 * (v * 2.0 * std::f64::consts::PI + t).sin();
                        Color::from_hsv(hue, sat, val)
                    };

                    ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// PETSCII-style character cycling animation.
///
/// Each non-space character is replaced by a symbol from a cycling sequence,
/// offset by its `(x, y)` position so the whole screen appears to animate
/// like a C64 demo. A gradient is applied on top.
///
/// `pattern` selects the character set:
/// - `"blocks"` — `░▒▓█▓▒░` density wave
/// - `"circles"` — `·∘○◎●◎○∘` growing circles
/// - `"dots"` — braille rotating dot
/// - `"diamonds"` — `◇◆◈◆◇` diamond pulse
/// - any other string is used as the cycle characters directly
pub fn petscii(text: &str, frame: usize, pattern: &str, gradient: Option<&Gradient>) -> String {
    let cycle: Vec<char> = match pattern {
        "blocks" => "░▒▓█▓▒░".chars().collect(),
        "circles" => "·∘○◎●◎○∘".chars().collect(),
        "dots" => "⠁⠂⠄⡀⢀⠠⠐⠈".chars().collect(),
        "diamonds" => "◇◆◈◆◇".chars().collect(),
        custom => custom.chars().collect(),
    };
    let cycle_len = cycle.len().max(1);

    let lines: Vec<&str> = text.split('\n').collect();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1).max(1);
    let palette = gradient.map(|g| g.palette(max_width.max(2)));

    lines
        .iter()
        .enumerate()
        .map(|(y, line)| {
            line.chars()
                .enumerate()
                .map(|(x, ch)| {
                    let out_ch = if ch.is_whitespace() {
                        // Whitespace gets the first cycle char (background)
                        cycle[0]
                    } else {
                        // Phase offset by position creates the kaleidoscope
                        let phase = (frame + x * 3 + y * 7) % cycle_len;
                        cycle[phase]
                    };

                    let c = if let Some(ref pal) = palette {
                        let color_phase = (x + y + frame) % pal.len();
                        pal[color_phase]
                    } else {
                        let hue = ((x + y) as f64 / (max_width + lines.len()) as f64 * 360.0
                            + frame as f64 * 8.0) % 360.0;
                        Color::from_hsv(hue, 0.9, 1.0)
                    };

                    out_ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Slide-in from the left with bounce easing.
///
/// Text starts off-screen and slides into its final position with a
/// bounce at the end. `total_frames` controls the animation duration;
/// after that the text stays in place. A rainbow gradient is applied.
pub fn scroll(text: &str, frame: usize, total_frames: usize) -> String {
    scroll_inner(text, frame, total_frames, None)
}

/// Slide-in from the left with bounce easing and a custom gradient.
pub fn scroll_with(text: &str, frame: usize, total_frames: usize, gradient: &Gradient) -> String {
    scroll_inner(text, frame, total_frames, Some(gradient))
}

fn scroll_inner(text: &str, frame: usize, total_frames: usize, gradient: Option<&Gradient>) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    if max_width == 0 {
        return text.to_string();
    }

    let t = if total_frames == 0 {
        1.0
    } else {
        (frame as f64 / total_frames as f64).min(1.0)
    };
    let eased = bounce_out(t);

    // offset = how many chars the text is still shifted off-screen to the left
    // Starts at max_width (fully hidden), ends at 0 (fully visible).
    // Bounce can overshoot past 0 (negative = text shifted right, gap on left).
    let offset = ((1.0 - eased) * max_width as f64).round() as i32;

    let palette = gradient.map(|g| g.palette(max_width.max(2)));

    lines
        .iter()
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let width = chars.len();
            let padded: Vec<char> = chars
                .iter()
                .copied()
                .chain(std::iter::repeat(' ').take(max_width - width))
                .collect();

            (0..max_width)
                .map(|i| {
                    let src = i as i32 + offset;
                    let ch = if src >= 0 && (src as usize) < max_width {
                        padded[src as usize]
                    } else {
                        ' '
                    };

                    let c = if let Some(ref pal) = palette {
                        pal[i % pal.len()]
                    } else {
                        let hue = (i as f64 / max_width as f64) * 360.0;
                        Color::from_hsv(hue, 0.9, 1.0)
                    };

                    ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Bounce-out easing: overshoots then settles.
fn bounce_out(t: f64) -> f64 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984375
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_color<F: FnOnce()>(f: F) {
        colored::control::set_override(true);
        f();
    }

    #[test]
    fn plasma_preserves_line_structure() {
        with_color(|| {
            let output = plasma("ab\ncd", 0, None);
            let lines: Vec<&str> = output.split('\n').collect();
            assert_eq!(lines.len(), 2);
        });
    }

    #[test]
    fn plasma_empty_input() {
        let output = plasma("", 0, None);
        assert_eq!(output, "");
    }

    #[test]
    fn plasma_single_char() {
        with_color(|| {
            let output = plasma("x", 0, None);
            assert!(!output.is_empty());
            assert!(output.contains('x'));
        });
    }

    #[test]
    fn plasma_deterministic() {
        with_color(|| {
            let a = plasma("hello\nworld", 10, None);
            let b = plasma("hello\nworld", 10, None);
            assert_eq!(a, b);
        });
    }

    #[test]
    fn plasma_different_frames_differ() {
        with_color(|| {
            let a = plasma("hello", 0, None);
            let b = plasma("hello", 50, None);
            assert_ne!(a, b);
        });
    }

    #[test]
    fn plasma_empty_palette_no_panic() {
        let output = plasma("test", 0, Some(&[]));
        assert!(!output.is_empty());
    }

    #[test]
    fn plasma_single_color_palette() {
        let pal = [Color::new(255, 0, 0)];
        let output = plasma("test", 0, Some(&pal));
        assert!(!output.is_empty());
    }

    #[test]
    fn plasma_with_palette() {
        with_color(|| {
            let pal = [Color::new(255, 0, 0), Color::new(0, 0, 255)];
            let output = plasma("ab\ncd", 0, Some(&pal));
            let lines: Vec<&str> = output.split('\n').collect();
            assert_eq!(lines.len(), 2);
        });
    }

    #[test]
    fn scroll_preserves_line_count() {
        with_color(|| {
            let output = scroll("abc\ndef\nghi", 30, 60);
            assert_eq!(output.split('\n').count(), 3);
        });
    }

    #[test]
    fn scroll_empty_input() {
        let output = scroll("", 0, 60);
        assert_eq!(output, "");
    }

    #[test]
    fn scroll_frame_zero_is_blank() {
        with_color(|| {
            // At frame 0 the text is fully off-screen
            let output = scroll("hello", 0, 60);
            // Should contain only spaces (plus ANSI codes)
            let stripped: String = output.chars().filter(|c| !c.is_ascii_control() && *c != '[' && *c != 'm' && !c.is_ascii_digit() && *c != ';').collect();
            assert!(stripped.trim().is_empty(), "frame 0 should be blank, got: {stripped}");
        });
    }

    #[test]
    fn scroll_final_frame_shows_text() {
        with_color(|| {
            let total = 60;
            let output = scroll("hello", total, total);
            // After the animation completes, original text should be visible
            assert!(output.contains('h'));
            assert!(output.contains('o'));
        });
    }

    #[test]
    fn scroll_past_end_holds_position() {
        with_color(|| {
            let total = 60;
            let a = scroll("hello", total, total);
            let b = scroll("hello", total + 100, total);
            assert_eq!(a, b);
        });
    }

    #[test]
    fn scroll_multiline_same_line_count() {
        with_color(|| {
            let output = scroll("ab\ncdef", 30, 60);
            assert_eq!(output.split('\n').count(), 2);
        });
    }

    #[test]
    fn petscii_preserves_line_count() {
        with_color(|| {
            let output = petscii("abc\ndef", 0, "blocks", None);
            assert_eq!(output.split('\n').count(), 2);
        });
    }

    #[test]
    fn petscii_spaces_become_background() {
        with_color(|| {
            let output = petscii("a b", 0, "blocks", None);
            // The space becomes the first cycle char (░ for blocks)
            assert!(output.contains('░'));
        });
    }

    #[test]
    fn petscii_different_frames_differ() {
        with_color(|| {
            let a = petscii("hello", 0, "blocks", None);
            let b = petscii("hello", 3, "blocks", None);
            assert_ne!(a, b);
        });
    }

    #[test]
    fn petscii_custom_pattern() {
        with_color(|| {
            let output = petscii("x", 0, "AB", None);
            // Should contain either A or B, not x
            assert!(output.contains('A') || output.contains('B'));
        });
    }
}

fn apply_solid(text: &str, c: Color) -> String {
    text.truecolor(c.r, c.g, c.b).to_string()
}
