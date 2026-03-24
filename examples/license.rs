use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, ScrollDirection, TimeRange};

const LICENSE: &str = "\
MIT License

Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.";

#[tokio::main]
async fn main() {
    let full_text = chromakopia::pad(LICENSE);
    let line_count = full_text.lines().count();

    let fps = 30;
    let frames_per_line = 90;
    let stagger = 1;
    let scroll_secs = ((line_count - 1) * stagger + frames_per_line) as f64 / fps as f64;
    let total = scroll_secs + 5.0;

    let scroll_fn = animate::scroll_staggered_effect(
        ScrollDirection::Left,
        Easing::Elastic(0.25),
        presets::storm(),
        frames_per_line,
        stagger,
    );
    let plasma_fn = animate::plasma_gradient_effect(presets::storm());

    // Composite: plasma colors the text, scroll moves it
    // Scroll renders position + gradient, plasma re-colors the visible characters
    let composite = move |text: &str, frame: usize| -> String {
        let scrolled = scroll_fn(text, frame);
        // Apply plasma coloring to the scrolled output's visible characters
        // by running plasma on the original text and borrowing its colors
        let plasma_colored = plasma_fn(text, frame);

        // Merge: take character positions from scroll, colors from plasma
        let scroll_lines: Vec<&str> = scrolled.split('\n').collect();
        let plasma_lines: Vec<&str> = plasma_colored.split('\n').collect();

        scroll_lines.iter().enumerate().map(|(y, scroll_line)| {
            let plasma_line = plasma_lines.get(y).copied().unwrap_or("");
            merge_position_and_color(scroll_line, plasma_line)
        }).collect::<Vec<_>>().join("\n")
    };

    animate::Sequence::new(&full_text)
        .effect(TimeRange::new(0.0, total), fps as u64, composite)
        .fade(
            TimeRange::new(total - 2.0, total),
            FadeKind::FadeTo(FadeTarget::Foreground),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}

/// Take character visibility/position from `positioned` but color from `colored`.
/// Both are ANSI-encoded strings. Where positioned shows a space, output a space.
/// Where positioned shows a visible char, use the color from the corresponding
/// position in `colored`.
fn merge_position_and_color(positioned: &str, colored: &str) -> String {
    let pos_chars = extract_chars(positioned);
    let col_colors = extract_colors(colored);
    let mut result = String::new();

    for (i, (ch, _)) in pos_chars.iter().enumerate() {
        if ch.is_whitespace() {
            result.push(*ch);
        } else if let Some(color) = col_colors.get(i) {
            result.push_str(&format!("\x1B[38;2;{};{};{}m{}\x1B[0m", color.0, color.1, color.2, ch));
        } else {
            result.push(*ch);
        }
    }
    result
}

/// Extract (char, index) pairs from an ANSI string, skipping escape sequences.
fn extract_chars(s: &str) -> Vec<(char, usize)> {
    let mut chars = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut pos = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'm' { i += 1; }
            if i < bytes.len() { i += 1; }
        } else {
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 } else if byte < 0xE0 { 2 } else if byte < 0xF0 { 3 } else { 4 };
            let end = (i + char_len).min(bytes.len());
            if let Ok(ch_str) = std::str::from_utf8(&bytes[i..end]) {
                if let Some(ch) = ch_str.chars().next() {
                    chars.push((ch, pos));
                }
            }
            pos += 1;
            i = end;
        }
    }
    chars
}

/// Extract colors at each visible character position from an ANSI string.
fn extract_colors(s: &str) -> Vec<(u8, u8, u8)> {
    let mut colors = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut current_color = (255u8, 255u8, 255u8);
    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            let seq_start = i;
            while i < bytes.len() && bytes[i] != b'm' { i += 1; }
            if i < bytes.len() {
                let seq = &s[seq_start..i];
                if let Some(rgb) = seq.strip_prefix("38;2;") {
                    let parts: Vec<&str> = rgb.split(';').collect();
                    if parts.len() == 3 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[0].parse(), parts[1].parse(), parts[2].parse()
                        ) {
                            current_color = (r, g, b);
                        }
                    }
                }
                i += 1;
            }
        } else {
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 } else if byte < 0xE0 { 2 } else if byte < 0xF0 { 3 } else { 4 };
            colors.push(current_color);
            i = (i + char_len).min(bytes.len());
        }
    }
    colors
}
