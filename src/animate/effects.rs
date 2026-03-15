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
    if frame % 2 == 0 {
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

fn apply_solid(text: &str, c: Color) -> String {
    text.truecolor(c.r, c.g, c.b).to_string()
}
