//! Keyboard input for interactive animations.

use std::sync::Arc;

/// A key press, as delivered to `Scene::on_key` handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
}

/// Shared key callback, called from the input thread.
pub(crate) type KeyHandler = Arc<dyn Fn(Key) + Send + Sync>;

/// Decode raw terminal bytes into keys. Arrows arrive as `ESC [ A–D`
/// (or `ESC O A–D` in application mode); any other escape sequence is
/// dropped whole. Control bytes and non-ASCII bytes are skipped.
pub fn parse_keys(bytes: &[u8]) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1B {
            if i + 2 < bytes.len() && matches!(bytes[i + 1], b'[' | b'O') {
                let arrow = match bytes[i + 2] {
                    b'A' => Some(Key::Up),
                    b'B' => Some(Key::Down),
                    b'C' => Some(Key::Right),
                    b'D' => Some(Key::Left),
                    _ => None,
                };
                if let Some(key) = arrow {
                    keys.push(key);
                    i += 3;
                } else {
                    // Unknown sequence: skip through its final byte (0x40–0x7E).
                    let mut j = i + 2;
                    while j < bytes.len() && !(0x40..=0x7E).contains(&bytes[j]) {
                        j += 1;
                    }
                    i = j + 1;
                }
            } else {
                i += 1; // lone ESC
            }
            continue;
        }
        if (0x20..0x7F).contains(&b) {
            keys.push(Key::Char(b as char));
        }
        i += 1;
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_chars() {
        assert_eq!(parse_keys(b"n 1q"), vec![Key::Char('n'), Key::Char(' '), Key::Char('1'), Key::Char('q')]);
    }

    #[test]
    fn arrows_csi_and_ss3() {
        assert_eq!(
            parse_keys(b"\x1B[A\x1B[B\x1B[C\x1B[D\x1BOC"),
            vec![Key::Up, Key::Down, Key::Right, Key::Left, Key::Right]
        );
    }

    #[test]
    fn mixed_input() {
        assert_eq!(parse_keys(b"a\x1B[Cb"), vec![Key::Char('a'), Key::Right, Key::Char('b')]);
    }

    #[test]
    fn unknown_escapes_are_dropped_whole() {
        // Delete key, Ctrl-Right, a lone ESC.
        assert_eq!(parse_keys(b"\x1B[3~x\x1B[1;5Cy\x1B"), vec![Key::Char('x'), Key::Char('y')]);
    }

    #[test]
    fn control_and_non_ascii_bytes_are_skipped() {
        assert_eq!(parse_keys(&[0x03, 0xC3, 0xA9, b'z']), vec![Key::Char('z')]);
    }
}
