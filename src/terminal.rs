use std::sync::OnceLock;

use crate::color::Color;

static BG_COLOR: OnceLock<Color> = OnceLock::new();
static FG_COLOR: OnceLock<Color> = OnceLock::new();
static PROBED_COLORS: OnceLock<(Option<Color>, Option<Color>)> = OnceLock::new();
static SYSTEM_THEME: OnceLock<(Color, Color)> = OnceLock::new();

fn probed_osc() -> &'static (Option<Color>, Option<Color>) {
    PROBED_COLORS.get_or_init(probe_osc_colors)
}

/// Probe both terminal foreground and background colors in a single `/dev/tty` session.
///
/// Sends OSC 10 (foreground) and OSC 11 (background) queries back-to-back
/// and parses both responses. Returns `(fg, bg)` where each is `None` if
/// that specific query failed.
#[cfg(unix)]
fn probe_osc_colors() -> (Option<Color>, Option<Color>) {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;
    use std::time::Duration;

    let Some(mut tty) = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
    else {
        return (None, None);
    };

    let fd = tty.as_raw_fd();

    if unsafe { libc::isatty(fd) } != 1 {
        return (None, None);
    }

    let old_termios = unsafe {
        let mut t = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return (None, None);
        }
        t
    };

    struct TermiosGuard {
        fd: i32,
        original: libc::termios,
    }
    impl Drop for TermiosGuard {
        fn drop(&mut self) {
            unsafe {
                libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
            }
        }
    }
    let _guard = TermiosGuard { fd, original: old_termios };

    let mut raw = old_termios;
    unsafe {
        libc::cfmakeraw(&mut raw);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return (None, None);
        }
    }

    // Send both queries back-to-back in one write
    let query = "\x1B]10;?\x07\x1B]11;?\x07";
    if tty.write_all(query.as_bytes()).is_err() {
        return (None, None);
    }
    let _ = tty.flush();

    // Read all response bytes (expecting two OSC responses)
    let mut response = Vec::new();
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(200);
    let mut terminators_seen = 0u8;

    loop {
        if start.elapsed() > timeout || terminators_seen >= 2 {
            break;
        }
        let mut byte = [0u8; 1];
        match tty.read(&mut byte) {
            Ok(1) => {
                response.push(byte[0]);
                // BEL terminator
                if byte[0] == 0x07 && response.len() > 4 {
                    terminators_seen += 1;
                }
                // ST terminator (ESC \)
                if response.len() >= 2
                    && response[response.len() - 2] == 0x1B
                    && response[response.len() - 1] == b'\\'
                {
                    terminators_seen += 1;
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break,
        }
    }

    // Drain leftover bytes
    {
        let mut byte = [0u8; 1];
        loop {
            match std::io::Read::read(&mut tty, &mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    }

    // _guard drops here, restoring old_termios

    // Split response into individual OSC replies and parse each
    let fg = parse_osc_response_for_code(&response, 10);
    let bg = parse_osc_response_for_code(&response, 11);
    (fg, bg)
}

#[cfg(not(unix))]
fn probe_osc_colors() -> (Option<Color>, Option<Color>) {
    (None, None)
}

/// Extract the color for a specific OSC code from a response buffer
/// that may contain multiple concatenated OSC replies.
fn parse_osc_response_for_code(response: &[u8], osc_code: u8) -> Option<Color> {
    let s = String::from_utf8_lossy(response);
    let marker = format!("]{};", osc_code);
    let start = s.find(&marker)?;
    let sub = &s[start..];
    // Find the end of this response (BEL or ST)
    let end = sub.find('\x07')
        .map(|i| i + 1)
        .or_else(|| {
            sub.find("\x1B\\").map(|i| i + 2)
        })?;
    parse_osc_color_response(sub[..end].as_bytes())
}

/// Try to parse the `COLORFGBG` environment variable.
///
/// Format is typically "fg;bg" where fg and bg are ANSI color indices (0-255).
/// Also handles the three-part format "fg;default;bg" used by some terminals
/// (e.g. rxvt). The last segment is always used as the background index.
fn parse_colorfgbg() -> Option<(Color, Color)> {
    let val = std::env::var("COLORFGBG").ok()?;
    let parts: Vec<&str> = val.split(';').collect();
    if parts.len() < 2 {
        return None;
    }
    let fg_idx: u8 = parts[0].parse().ok()?;
    let bg_idx: u8 = parts[parts.len() - 1].parse().ok()?;
    Some((ansi_index_to_color(fg_idx), ansi_index_to_color(bg_idx)))
}

/// Convert an ANSI color index to an approximate RGB color.
///
/// Supports the standard 16 colors (0-15), the 6x6x6 color cube (16-231),
/// and the grayscale ramp (232-255).
fn ansi_index_to_color(idx: u8) -> Color {
    match idx {
        0 => Color::new(0, 0, 0),          // black
        1 => Color::new(170, 0, 0),        // red
        2 => Color::new(0, 170, 0),        // green
        3 => Color::new(170, 170, 0),      // yellow
        4 => Color::new(0, 0, 170),        // blue
        5 => Color::new(170, 0, 170),      // magenta
        6 => Color::new(0, 170, 170),      // cyan
        7 => Color::new(170, 170, 170),    // white
        8 => Color::new(85, 85, 85),       // bright black
        9 => Color::new(255, 85, 85),      // bright red
        10 => Color::new(85, 255, 85),     // bright green
        11 => Color::new(255, 255, 85),    // bright yellow
        12 => Color::new(85, 85, 255),     // bright blue
        13 => Color::new(255, 85, 255),    // bright magenta
        14 => Color::new(85, 255, 255),    // bright cyan
        15 => Color::new(255, 255, 255),   // bright white
        // 6x6x6 color cube
        16..=231 => {
            let i = idx - 16;
            let r = if i / 36 == 0 { 0 } else { 55 + 40 * (i / 36) };
            let g = if (i % 36) / 6 == 0 { 0 } else { 55 + 40 * ((i % 36) / 6) };
            let b = if i % 6 == 0 { 0 } else { 55 + 40 * (i % 6) };
            Color::new(r, g, b)
        }
        // Grayscale ramp
        232..=255 => {
            let gray = 8 + 10 * (idx - 232);
            Color::new(gray, gray, gray)
        }
    }
}

/// Parse an OSC color response: `ESC]{code};rgb:RRRR/GGGG/BBBB BEL`
fn parse_osc_color_response(response: &[u8]) -> Option<Color> {
    let s = String::from_utf8_lossy(response);

    // Find "rgb:" in the response
    let rgb_start = s.find("rgb:")?;
    let rgb_part = &s[rgb_start + 4..];

    // Strip trailing BEL or ST
    let rgb_part = rgb_part
        .trim_end_matches('\x07')
        .trim_end_matches('\\')
        .trim_end_matches('\x1B');

    // Parse R/G/B — values can be 2 or 4 hex digits
    let parts: Vec<&str> = rgb_part.split('/').collect();
    if parts.len() != 3 {
        return None;
    }

    let r = parse_hex_component(parts[0])?;
    let g = parse_hex_component(parts[1])?;
    let b = parse_hex_component(parts[2])?;

    Some(Color::new(r, g, b))
}

fn parse_hex_component(s: &str) -> Option<u8> {
    let val = u16::from_str_radix(s, 16).ok()?;
    Some(match s.len() {
        2 => val as u8,
        4 => (val >> 8) as u8, // scale 16-bit to 8-bit
        _ => return None,
    })
}

/// Guess fg/bg colors from the `TERM_PROGRAM` environment variable.
///
/// Only returns `Some` for terminals with a strong default theme.
/// Returns `None` for terminals that commonly vary (iTerm2, VS Code, etc.).
fn term_program_colors(term_program: &str) -> Option<(Color, Color)> {
    match term_program {
        // Apple Terminal defaults to white background
        "Apple_Terminal" => Some((Color::new(0, 0, 0), Color::new(255, 255, 255))),
        _ => None,
    }
}

/// Detect system theme and return conservative fg/bg color pair.
///
/// On macOS, queries `defaults read -globalDomain AppleInterfaceStyle`.
/// Cached in its own `OnceLock` so the command runs at most once.
fn system_theme_colors() -> (Color, Color) {
    *SYSTEM_THEME.get_or_init(detect_system_theme)
}

#[cfg(target_os = "macos")]
fn detect_system_theme() -> (Color, Color) {
    let is_dark = std::process::Command::new("defaults")
        .args(["read", "-globalDomain", "AppleInterfaceStyle"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            String::from_utf8(o.stdout).ok()
        } else {
            None
        })
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("dark"));

    if is_dark {
        (Color::new(204, 204, 204), Color::new(30, 30, 30))
    } else {
        (Color::new(51, 51, 51), Color::new(255, 255, 255))
    }
}

#[cfg(not(target_os = "macos"))]
fn detect_system_theme() -> (Color, Color) {
    // Non-macOS: assume dark theme (most developer terminals are dark)
    (Color::new(204, 204, 204), Color::new(0, 0, 0))
}

/// Eagerly probe and cache both terminal colors.
///
/// Call this before hiding the cursor or writing any escape sequences,
/// because the OSC probe reads from the terminal and stray escape output
/// could corrupt the response. Safe to call multiple times — only probes
/// on first call.
pub fn probe_colors() {
    let _ = bg_color();
    let _ = fg_color();
}

/// Get the terminal background color.
///
/// Probes the terminal via OSC 11 on first call, falls back to `COLORFGBG`
/// env var, then defaults to black. Caches the result.
pub fn bg_color() -> Color {
    *BG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().1 {
            return c;
        }
        if let Some((_, bg)) = parse_colorfgbg() {
            return bg;
        }
        if let Ok(tp) = std::env::var("TERM_PROGRAM") {
            if let Some((_, bg)) = term_program_colors(&tp) {
                return bg;
            }
        }
        system_theme_colors().1
    })
}

/// Get the terminal foreground color.
///
/// Probes the terminal via OSC 10 on first call, falls back to `COLORFGBG`
/// env var, then defaults to light gray. Caches the result.
pub fn fg_color() -> Color {
    *FG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().0 {
            return c;
        }
        if let Some((fg, _)) = parse_colorfgbg() {
            return fg;
        }
        if let Ok(tp) = std::env::var("TERM_PROGRAM") {
            if let Some((fg, _)) = term_program_colors(&tp) {
                return fg;
            }
        }
        system_theme_colors().0
    })
}

/// Returns `true` if the terminal background appears light (luma > 0.5).
///
/// Uses the same detection chain as [`bg_color()`].
pub fn is_light_theme() -> bool {
    bg_color().luma() > 0.5
}

/// Returns `true` if the terminal background appears dark (luma ≤ 0.5).
///
/// Uses the same detection chain as [`bg_color()`].
pub fn is_dark_theme() -> bool {
    !is_light_theme()
}

/// Manually set the background color (overrides auto-detection).
pub fn set_bg_color(color: Color) {
    let _ = BG_COLOR.set(color);
}

/// Manually set the foreground color (overrides auto-detection).
pub fn set_fg_color(color: Color) {
    let _ = FG_COLOR.set(color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_4digit_response() {
        let resp = b"\x1B]11;rgb:0000/0000/0000\x07";
        assert_eq!(parse_osc_color_response(resp), Some(Color::new(0, 0, 0)));
    }

    #[test]
    fn parse_white_bg() {
        let resp = b"\x1B]11;rgb:ffff/ffff/ffff\x07";
        assert_eq!(parse_osc_color_response(resp), Some(Color::new(255, 255, 255)));
    }

    #[test]
    fn parse_2digit_response() {
        let resp = b"\x1B]11;rgb:1c/1c/1c\x07";
        assert_eq!(parse_osc_color_response(resp), Some(Color::new(0x1c, 0x1c, 0x1c)));
    }

    #[test]
    fn parse_st_terminator() {
        let resp = b"\x1B]11;rgb:2800/2800/2800\x1B\\";
        assert_eq!(parse_osc_color_response(resp), Some(Color::new(0x28, 0x28, 0x28)));
    }

    #[test]
    fn ansi_standard_colors() {
        assert_eq!(ansi_index_to_color(0), Color::new(0, 0, 0));       // black
        assert_eq!(ansi_index_to_color(1), Color::new(170, 0, 0));     // red
        assert_eq!(ansi_index_to_color(4), Color::new(0, 0, 170));     // blue
        assert_eq!(ansi_index_to_color(7), Color::new(170, 170, 170)); // white
        assert_eq!(ansi_index_to_color(8), Color::new(85, 85, 85));    // bright black
        assert_eq!(ansi_index_to_color(12), Color::new(85, 85, 255));  // bright blue
        assert_eq!(ansi_index_to_color(15), Color::new(255, 255, 255)); // bright white
    }

    #[test]
    fn ansi_color_cube() {
        // Index 16 = (0,0,0) in the cube → black
        assert_eq!(ansi_index_to_color(16), Color::new(0, 0, 0));
        // Index 196 = (5,0,0) → pure red end of cube
        assert_eq!(ansi_index_to_color(196), Color::new(255, 0, 0));
        // Index 21 = (0,0,5) → pure blue end of cube
        assert_eq!(ansi_index_to_color(21), Color::new(0, 0, 255));
    }

    #[test]
    fn ansi_grayscale_ramp() {
        assert_eq!(ansi_index_to_color(232), Color::new(8, 8, 8));
        assert_eq!(ansi_index_to_color(255), Color::new(238, 238, 238));
    }

    #[test]
    fn parse_colorfgbg_two_parts() {
        // Test the parsing logic directly by extracting the core logic
        fn parse_val(val: &str) -> Option<(Color, Color)> {
            let parts: Vec<&str> = val.split(';').collect();
            if parts.len() < 2 {
                return None;
            }
            let fg_idx: u8 = parts[0].parse().ok()?;
            let bg_idx: u8 = parts[parts.len() - 1].parse().ok()?;
            Some((ansi_index_to_color(fg_idx), ansi_index_to_color(bg_idx)))
        }

        // Standard "fg;bg" format
        let (fg, bg) = parse_val("15;0").unwrap();
        assert_eq!(fg, Color::new(255, 255, 255));
        assert_eq!(bg, Color::new(0, 0, 0));

        // Three-part format "fg;default;bg" (rxvt)
        let (fg, bg) = parse_val("15;default;0").unwrap();
        assert_eq!(fg, Color::new(255, 255, 255));
        assert_eq!(bg, Color::new(0, 0, 0));

        // Single value → None
        assert!(parse_val("15").is_none());

        // Non-numeric → None
        assert!(parse_val("default;default").is_none());

        // Empty → None
        assert!(parse_val("").is_none());
    }

    #[test]
    fn parse_osc_response_for_code_extracts_fg() {
        // Concatenated fg + bg responses
        let combined = b"\x1B]10;rgb:cccc/cccc/cccc\x07\x1B]11;rgb:1c1c/1c1c/1c1c\x07";
        assert_eq!(
            parse_osc_response_for_code(combined, 10),
            Some(Color::new(0xcc, 0xcc, 0xcc))
        );
        assert_eq!(
            parse_osc_response_for_code(combined, 11),
            Some(Color::new(0x1c, 0x1c, 0x1c))
        );
    }

    #[test]
    fn macos_theme_fg_lighter_than_bg_in_dark_mode_or_vice_versa() {
        let (fg, bg) = system_theme_colors();
        // fg and bg should have distinct brightness — one light, one dark
        let fg_luma = fg.luma();
        let bg_luma = bg.luma();
        assert!((fg_luma - bg_luma).abs() > 0.3,
            "fg luma ({fg_luma}) and bg luma ({bg_luma}) should differ significantly");
    }

    #[test]
    fn term_program_heuristics() {
        assert_eq!(term_program_colors("Apple_Terminal"), Some((Color::new(0, 0, 0), Color::new(255, 255, 255))));
        assert_eq!(term_program_colors("iTerm.app"), None);
        assert_eq!(term_program_colors("unknown_terminal"), None);
    }

    #[test]
    fn theme_detection_is_consistent() {
        let light = is_light_theme();
        let dark = is_dark_theme();
        assert!(light != dark);
    }

    #[test]
    fn parse_osc_response_for_code_missing() {
        let only_bg = b"\x1B]11;rgb:0000/0000/0000\x07";
        assert_eq!(parse_osc_response_for_code(only_bg, 10), None);
        assert_eq!(
            parse_osc_response_for_code(only_bg, 11),
            Some(Color::new(0, 0, 0))
        );
    }
}
