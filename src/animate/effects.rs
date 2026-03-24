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
    plasma_full(text, frame, palette, 0.0, 0.0)
}

/// Plasma with a vertical offset.
pub fn plasma_offset(text: &str, frame: usize, palette: Option<&[Color]>, y_offset: f64) -> String {
    plasma_full(text, frame, palette, y_offset, 0.0)
}

/// Plasma with vertical offset and deterministic seed.
///
/// The seed shifts the phase of all sine waves, producing a completely
/// different pattern. Same seed always produces the same result.
pub fn plasma_full(text: &str, frame: usize, palette: Option<&[Color]>, y_offset: f64, seed: f64) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let t = frame as f64 * 0.08;

    lines
        .iter()
        .enumerate()
        .map(|(y, line)| {
            let yf = y as f64 + y_offset;
            line.chars()
                .enumerate()
                .map(|(x, ch)| {
                    let xf = x as f64;

                    // Four overlapping sine planes — the classic plasma recipe
                    let v1 = (xf * 0.08 + t + seed).sin();
                    let v2 = (yf * 0.12 + t * 0.6 + seed * 1.3).sin();
                    let v3 = ((xf * 0.06 + yf * 0.08 + t * 0.4 + seed * 0.7).sin()
                        + (xf * 0.04 - yf * 0.06 + t * 0.7 + seed * 1.9).cos())
                        * 0.5;
                    // Radial ripple from center
                    let cx = xf - 30.0;
                    let cy = (yf - 5.0) * 2.5; // exaggerate y for an elongated ripple
                    let v4 = ((cx * cx + cy * cy).sqrt() * 0.12 - t * 1.2 + seed * 0.5).sin();

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

/// Direction from which text slides in.
#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Left,
    Right,
    Top,
    Bottom,
}

/// Starfield warp: stars radiate outward from a central vanishing point.
///
/// Characters near the center are dim and slow. Characters near the edges
/// are bright and streak fast — like flying through a star tunnel.
pub fn sparkle(text: &str, frame: usize, palette: Option<&[Color]>) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let line_count = lines.len();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1).max(1);
    let cx = max_width as f64 / 2.0;
    let cy = line_count as f64 / 2.0;
    let max_dist = (cx * cx + (cy * 2.5) * (cy * 2.5)).sqrt();
    let t = frame as f64;

    lines
        .iter()
        .enumerate()
        .map(|(y, line)| {
            line.chars()
                .enumerate()
                .map(|(x, ch)| {
                    if ch.is_whitespace() {
                        return ch.to_string();
                    }

                    let dx = x as f64 - cx;
                    let dy = (y as f64 - cy) * 2.5; // stretch y (chars are taller)
                    let dist = (dx * dx + dy * dy).sqrt() / max_dist; // 0=center, 1=edge

                    // Each star has a unique phase based on position
                    let phase = ((x * 3571 + y * 2719) % 997) as f64;

                    // Stars move outward over time — the "warp" cycle
                    // Near center: slow cycle. Near edge: fast cycle.
                    let speed = 0.2 + dist * 0.8;
                    let cycle = ((t * speed * 0.15 + phase) % 40.0) / 40.0;

                    // Brightness: edges are brighter, center is dimmer
                    // Plus a pulsing sparkle based on the cycle
                    let pulse = (cycle * std::f64::consts::TAU).sin() * 0.5 + 0.5;
                    let brightness = dist * (0.3 + 0.7 * pulse);

                    let c = if let Some(pal) = palette {
                        if pal.is_empty() {
                            Color::new(0, 0, 0)
                        } else {
                            // Pick color based on distance — center is deep, edges are hot
                            let color_t = (dist + cycle * 0.3).rem_euclid(1.0);
                            let fi = color_t * (pal.len() - 1) as f64;
                            let lo = (fi.floor() as usize).min(pal.len() - 1);
                            let hi = (lo + 1).min(pal.len() - 1);
                            let frac = fi - lo as f64;
                            let base = Color::lerp_rgb(pal[lo], pal[hi], frac);
                            Color::new(
                                (base.r as f64 * brightness) as u8,
                                (base.g as f64 * brightness) as u8,
                                (base.b as f64 * brightness) as u8,
                            )
                        }
                    } else {
                        let r = (200.0 * brightness + 55.0 * dist) as u8;
                        let g = (220.0 * brightness + 35.0 * dist) as u8;
                        let b = (255.0 * brightness) as u8;
                        Color::new(r, g, b)
                    };

                    ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Slide-in with bounce easing and rainbow gradient.
pub fn scroll(text: &str, frame: usize, total_frames: usize, direction: ScrollDirection) -> String {
    scroll_inner(text, frame, total_frames, direction, None, crate::animate::Easing::BounceOut)
}

/// Slide-in with bounce easing and a custom gradient.
pub fn scroll_with(text: &str, frame: usize, total_frames: usize, direction: ScrollDirection, gradient: &Gradient) -> String {
    scroll_inner(text, frame, total_frames, direction, Some(gradient), crate::animate::Easing::BounceOut)
}

/// Slide-in with a specific easing and rainbow gradient.
#[allow(dead_code)]
pub fn scroll_eased(text: &str, frame: usize, total_frames: usize, direction: ScrollDirection, easing: crate::animate::Easing) -> String {
    scroll_inner(text, frame, total_frames, direction, None, easing)
}

/// Slide-in with a specific easing and a custom gradient.
pub fn scroll_eased_with(text: &str, frame: usize, total_frames: usize, direction: ScrollDirection, easing: crate::animate::Easing, gradient: &Gradient) -> String {
    scroll_inner(text, frame, total_frames, direction, Some(gradient), easing)
}

fn scroll_inner(
    text: &str,
    frame: usize,
    total_frames: usize,
    direction: ScrollDirection,
    gradient: Option<&Gradient>,
    easing: crate::animate::Easing,
) -> String {
    scroll_staggered(text, frame, total_frames, direction, gradient, easing, 0)
}

/// Core scroll renderer with per-line stagger.
///
/// `line_delay` is the number of frames between each line's start.
/// 0 means all lines animate together.
pub(crate) fn scroll_staggered(
    text: &str,
    frame: usize,
    total_frames: usize,
    direction: ScrollDirection,
    gradient: Option<&Gradient>,
    easing: crate::animate::Easing,
    line_delay: usize,
) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let line_count = lines.len();
    let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    if max_width == 0 {
        return text.to_string();
    }

    let term_width = crate::terminal::terminal_width();
    let render_width = term_width.max(max_width);
    let palette = gradient.map(|g| g.palette(render_width.max(2)));

    // Pad all lines to max_width
    let padded_lines: Vec<Vec<char>> = lines
        .iter()
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let width = chars.len();
            chars
                .into_iter()
                .chain(std::iter::repeat(' ').take(max_width - width))
                .collect()
        })
        .collect();

    (0..line_count)
        .map(|y| {
            // Each line starts `line_delay * y` frames later
            let line_frame = frame.saturating_sub(y * line_delay);
            let t = if total_frames == 0 {
                1.0
            } else if line_frame == 0 && frame < y * line_delay {
                // Line hasn't started yet
                0.0
            } else {
                (line_frame as f64 / total_frames as f64).min(1.0)
            };
            let eased = easing.apply(t);

            let h_offset = match direction {
                ScrollDirection::Left | ScrollDirection::Right => {
                    let sign = if matches!(direction, ScrollDirection::Left) { 1.0 } else { -1.0 };
                    if eased <= 1.0 {
                        (sign * (1.0 - eased) * max_width as f64).round() as i32
                    } else {
                        (sign * (1.0 - eased) * term_width as f64).round() as i32
                    }
                }
                _ => 0,
            };

            let v_offset = match direction {
                ScrollDirection::Top | ScrollDirection::Bottom => {
                    let sign = if matches!(direction, ScrollDirection::Top) { 1.0 } else { -1.0 };
                    if eased <= 1.0 {
                        (sign * (1.0 - eased) * line_count as f64).round() as i32
                    } else {
                        (sign * (1.0 - eased) * line_count as f64).round() as i32
                    }
                }
                _ => 0,
            };

            let src_y = y as i32 + v_offset;
            let line_visible = src_y >= 0 && (src_y as usize) < line_count;

            (0..render_width)
                .map(|x| {
                    let src_x = x as i32 + h_offset;
                    let ch = if line_visible
                        && src_x >= 0
                        && (src_x as usize) < max_width
                    {
                        padded_lines[src_y as usize][src_x as usize]
                    } else {
                        ' '
                    };

                    let c = if let Some(ref pal) = palette {
                        pal[x % pal.len()]
                    } else {
                        let hue = (x as f64 / render_width as f64) * 360.0;
                        Color::from_hsv(hue, 0.9, 1.0)
                    };

                    ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            let output = scroll("abc\ndef\nghi", 30, 60, ScrollDirection::Left);
            assert_eq!(output.split('\n').count(), 3);
        });
    }

    #[test]
    fn scroll_empty_input() {
        let output = scroll("", 0, 60, ScrollDirection::Left);
        assert_eq!(output, "");
    }

    #[test]
    fn scroll_frame_zero_is_blank() {
        with_color(|| {
            let output = scroll("hello", 0, 60, ScrollDirection::Left);
            let stripped: String = output.chars().filter(|c| !c.is_ascii_control() && *c != '[' && *c != 'm' && !c.is_ascii_digit() && *c != ';').collect();
            assert!(stripped.trim().is_empty(), "frame 0 should be blank, got: {stripped}");
        });
    }

    #[test]
    fn scroll_final_frame_shows_text() {
        with_color(|| {
            let total = 60;
            let output = scroll("hello", total, total, ScrollDirection::Left);
            assert!(output.contains('h'));
            assert!(output.contains('o'));
        });
    }

    #[test]
    fn scroll_past_end_holds_position() {
        with_color(|| {
            let total = 60;
            let a = scroll("hello", total, total, ScrollDirection::Left);
            let b = scroll("hello", total + 100, total, ScrollDirection::Left);
            assert_eq!(a, b);
        });
    }

    #[test]
    fn scroll_multiline_same_line_count() {
        with_color(|| {
            let output = scroll("ab\ncdef", 30, 60, ScrollDirection::Left);
            assert_eq!(output.split('\n').count(), 2);
        });
    }

    #[test]
    #[test]
    fn scroll_right_final_shows_text() {
        with_color(|| {
            let total = 60;
            let output = scroll("hello", total, total, ScrollDirection::Right);
            assert!(output.contains('h'));
        });
    }

    #[test]
    fn scroll_top_final_shows_text() {
        with_color(|| {
            let total = 60;
            let output = scroll("hi\nlo", total, total, ScrollDirection::Top);
            assert!(output.contains('h'));
            assert!(output.contains('l'));
        });
    }

    #[test]
    fn scroll_bottom_final_shows_text() {
        with_color(|| {
            let total = 60;
            let output = scroll("hi\nlo", total, total, ScrollDirection::Bottom);
            assert!(output.contains('h'));
            assert!(output.contains('l'));
        });
    }

}

fn apply_solid(text: &str, c: Color) -> String {
    text.truecolor(c.r, c.g, c.b).to_string()
}
