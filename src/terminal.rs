use std::sync::OnceLock;

use crate::color::Color;

static BG_COLOR: OnceLock<Color> = OnceLock::new();
static FG_COLOR: OnceLock<Color> = OnceLock::new();

/// Probe a terminal color using an OSC query.
///
/// `osc_code` is 10 for foreground, 11 for background.
/// Sends `ESC]{osc_code};?BEL` and parses the `rgb:` response.
#[cfg(unix)]
fn probe_osc_color(osc_code: u8) -> Option<Color> {
    use std::io::{Read, Write};
    use std::time::Duration;

    let fd = libc_fd();
    let old_termios = unsafe {
        let mut t = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return None;
        }
        t
    };

    let mut raw = old_termios;
    unsafe {
        libc::cfmakeraw(&mut raw);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return None;
        }
    }

    let result = (|| -> Option<Color> {
        let mut stderr = std::io::stderr();
        let query = format!("\x1B]{};?\x07", osc_code);
        stderr.write_all(query.as_bytes()).ok()?;
        stderr.flush().ok()?;

        let mut stdin = std::io::stdin();
        let mut response = Vec::new();
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(100);

        loop {
            if start.elapsed() > timeout {
                break;
            }
            let mut byte = [0u8; 1];
            match stdin.read(&mut byte) {
                Ok(1) => {
                    response.push(byte[0]);
                    if byte[0] == 0x07 {
                        break;
                    }
                    if response.len() >= 2
                        && response[response.len() - 2] == 0x1B
                        && response[response.len() - 1] == b'\\'
                    {
                        break;
                    }
                }
                Ok(_) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }

        parse_osc_color_response(&response)
    })();

    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &old_termios);
    }

    result
}

#[cfg(not(unix))]
fn probe_osc_color(_osc_code: u8) -> Option<Color> {
    None
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

#[cfg(unix)]
fn libc_fd() -> i32 {
    // Use stdin fd for tcgetattr
    0
}

/// Get the terminal background color.
///
/// Probes the terminal on first call, caches the result.
/// Returns black if detection fails.
pub fn bg_color() -> Color {
    *BG_COLOR.get_or_init(|| probe_osc_color(11).unwrap_or(Color::new(0, 0, 0)))
}

/// Get the terminal foreground color.
///
/// Probes the terminal on first call, caches the result.
/// Returns white if detection fails.
pub fn fg_color() -> Color {
    *FG_COLOR.get_or_init(|| probe_osc_color(10).unwrap_or(Color::new(204, 204, 204)))
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
}
