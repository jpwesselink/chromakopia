use crate::color::Color;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A single cell in the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub color: Color,
}

impl Cell {
    pub fn new(ch: char, color: Color) -> Self {
        Self { ch, color }
    }

    pub fn space() -> Self {
        Self { ch: ' ', color: Color::new(0, 0, 0) }
    }
}

/// A 2D grid of cells representing one frame.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    cells: Vec<Cell>,
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::space(); width * height],
        }
    }

    /// Create a framebuffer from text, using a default color.
    pub fn from_text(text: &str, color: Color) -> Self {
        let lines: Vec<&str> = text.split('\n').collect();
        let height = lines.len();
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let mut buf = Self::new(width, height);
        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                buf.set(x, y, Cell::new(ch, color));
            }
        }
        buf
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Cell {
        self.cells[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, cell: Cell) {
        self.cells[y * self.width + x] = cell;
    }

    /// Set just the color at (x, y), keeping the character.
    #[inline]
    pub fn set_color(&mut self, x: usize, y: usize, color: Color) {
        self.cells[y * self.width + x].color = color;
    }

    /// Set just the character at (x, y), keeping the color.
    #[inline]
    pub fn set_char(&mut self, x: usize, y: usize, ch: char) {
        self.cells[y * self.width + x].ch = ch;
    }

    /// Write a line of text at row y with a color.
    pub fn write_line(&mut self, y: usize, text: &str, color: Color) {
        for (x, ch) in text.chars().enumerate() {
            if x < self.width {
                self.set(x, y, Cell::new(ch, color));
            }
        }
    }

    /// Clear all cells to spaces.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::space());
    }
}

/// Shared mailbox between animation and renderer.
type Mailbox = Arc<Mutex<Option<FrameBuffer>>>;

/// Try to get the current cursor row using ANSI DSR (Device Status Report).
/// Returns 1-based row number, or None if detection fails.
#[cfg(unix)]
fn get_cursor_row() -> Option<usize> {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;

    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;

    let fd = tty.as_raw_fd();
    let old = unsafe {
        let mut t = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 { return None; }
        t
    };

    let mut raw = old;
    unsafe {
        libc::cfmakeraw(&mut raw);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1;
        libc::tcsetattr(fd, libc::TCSANOW, &raw);
    }

    // Send DSR: ESC[6n — terminal responds with ESC[row;colR
    let _ = tty.write_all(b"\x1B[6n");
    let _ = tty.flush();

    let mut buf = [0u8; 32];
    let mut len = 0;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(100) {
        match tty.read(&mut buf[len..]) {
            Ok(0) => break,
            Ok(n) => {
                len += n;
                if buf[..len].contains(&b'R') { break; }
            }
            Err(_) => break,
        }
    }

    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &old); }

    let s = std::str::from_utf8(&buf[..len]).ok()?;
    // Parse ESC[row;colR
    let inner = s.strip_prefix("\x1B[")?.strip_suffix('R')?;
    let row_str = inner.split(';').next()?;
    row_str.parse().ok()
}

#[cfg(not(unix))]
fn get_cursor_row() -> Option<usize> {
    None
}

/// Diff two framebuffers and produce minimal ANSI output.
///
/// `start_row` is the 1-based terminal row where the framebuffer starts.
/// Uses absolute cursor positioning — no newlines, no scrolling.
fn diff_render(prev: Option<&FrameBuffer>, curr: &FrameBuffer, start_row: usize) -> String {
    let mut out = String::with_capacity(curr.width * curr.height * 4);
    let mut last_color: Option<Color> = None;

    for y in 0..curr.height {
        let mut need_position = true;

        for x in 0..curr.width {
            let cell = curr.get(x, y);
            let changed = prev.map_or(true, |p| {
                y < p.height && x < p.width && p.get(x, y) != cell
            }) || prev.is_none();

            if changed {
                if need_position {
                    // Absolute cursor position: ESC[row;colH (1-based)
                    out.push_str(&format!("\x1B[{};{}H", start_row + y, x + 1));
                }
                need_position = false;

                if last_color != Some(cell.color) {
                    out.push_str(&format!("\x1B[38;2;{};{};{}m", cell.color.r, cell.color.g, cell.color.b));
                    last_color = Some(cell.color);
                }
                out.push(cell.ch);
            } else {
                need_position = true;
            }
        }
    }

    if last_color.is_some() {
        out.push_str("\x1B[0m");
    }
    out
}

/// A framebuffer-based effect: writes directly to a grid instead of returning ANSI strings.
pub trait Effect: Send + 'static {
    /// Render frame into the buffer. Called by the animation loop.
    fn render(&self, buf: &mut FrameBuffer, frame: usize);
}

/// Adapt an old-style `fn(&str, usize) -> String` effect to the framebuffer system.
/// This is a bridge so existing effects work without rewriting them.
pub struct LegacyEffect<F: Fn(&str, usize) -> String + Send + 'static> {
    text: String,
    func: F,
}

impl<F: Fn(&str, usize) -> String + Send + 'static> LegacyEffect<F> {
    pub fn new(text: &str, func: F) -> Self {
        Self {
            text: text.to_string(),
            func,
        }
    }
}

impl<F: Fn(&str, usize) -> String + Send + 'static> Effect for LegacyEffect<F> {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let rendered = (self.func)(&self.text, frame);
        // Parse ANSI output back into the buffer
        let mut x = 0;
        let mut y = 0;
        let mut color = Color::new(204, 204, 204);
        let bytes = rendered.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'\n' {
                // Fill rest of line with spaces
                while x < buf.width {
                    buf.set(x, y, Cell::new(' ', Color::new(0, 0, 0)));
                    x += 1;
                }
                y += 1;
                x = 0;
                i += 1;
                if y >= buf.height {
                    break;
                }
            } else if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                // Parse ANSI escape
                i += 2;
                let seq_start = i;
                while i < bytes.len() && bytes[i] != b'm' {
                    i += 1;
                }
                if i < bytes.len() {
                    let seq = &rendered[seq_start..i];
                    if let Some(rgb) = seq.strip_prefix("38;2;") {
                        let parts: Vec<&str> = rgb.split(';').collect();
                        if parts.len() == 3 {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                parts[0].parse(),
                                parts[1].parse(),
                                parts[2].parse(),
                            ) {
                                color = Color::new(r, g, b);
                            }
                        }
                    } else if seq == "0" {
                        color = Color::new(204, 204, 204);
                    }
                    i += 1;
                }
            } else {
                // Visible character
                let byte = bytes[i];
                let char_len = if byte < 0x80 { 1 }
                    else if byte < 0xE0 { 2 }
                    else if byte < 0xF0 { 3 }
                    else { 4 };
                let end = (i + char_len).min(bytes.len());
                if let Ok(ch_str) = std::str::from_utf8(&bytes[i..end]) {
                    if let Some(ch) = ch_str.chars().next() {
                        if x < buf.width && y < buf.height {
                            buf.set(x, y, Cell::new(ch, color));
                        }
                        x += 1;
                    }
                }
                i = end;
            }
        }
    }
}

/// Run an effect with the two-loop framebuffer renderer.
///
/// - Animation loop: runs the effect, posts frames to the mailbox
/// - Render loop: 25fps fixed tick, diffs and flushes to stderr
pub async fn run_effect(
    effect: impl Effect,
    width: usize,
    height: usize,
    duration: Duration,
    speed: f64,
) {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mailbox: Mailbox = Arc::new(Mutex::new(None));
    let running = Arc::new(AtomicBool::new(true));

    // Hide cursor and reserve space by printing blank lines
    {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[?25l");
        // Print empty lines to reserve space, then save cursor row
        for _ in 0..height {
            let _ = write!(stderr, "\n");
        }
        // Move back up to where we started
        let _ = write!(stderr, "\x1B[{}A", height);
        let _ = stderr.flush();
    }

    // Query current cursor row via DSR — or just use a simple approach:
    // save the cursor position and use it as our start row
    // Since we can't reliably query, we'll use \x1B[s/\x1B[u (save/restore)
    // and track position via the row count we moved.
    //
    // Simpler: use \x1B7 to save position, render with absolute positioning
    // relative to saved position. Actually simplest: just get cursor row.
    //
    // For now, get cursor position via ANSI DSR or fall back to row 1.
    let start_row = get_cursor_row().unwrap_or(1);

    let fps = 25u64;
    let frame_duration = Duration::from_millis(1000 / fps);

    // Animation task
    let m = mailbox.clone();
    let r = running.clone();
    let anim_handle = tokio::spawn(async move {
        let mut frame: usize = 0;
        let delay = Duration::from_millis((33.0 / speed) as u64);
        while r.load(Ordering::Relaxed) {
            let mut buf = FrameBuffer::new(width, height);
            effect.render(&mut buf, frame);
            *m.lock().unwrap() = Some(buf);
            frame += 1;
            tokio::time::sleep(delay).await;
        }
    });

    // Render task
    let m = mailbox.clone();
    let r = running.clone();
    let term_width = crate::terminal::terminal_width();
    let render_handle = tokio::spawn(async move {
        let mut prev: Option<FrameBuffer> = None;
        let mut interval = tokio::time::interval(frame_duration);
        let mut last_fps_time = std::time::Instant::now();
        let mut render_count: u32 = 0;
        let mut displayed_fps: u32 = 0;

        while r.load(Ordering::Relaxed) {
            interval.tick().await;

            let new_frame = m.lock().unwrap().take();
            if let Some(buf) = new_frame {
                let output = diff_render(prev.as_ref(), &buf, start_row);
                render_count += 1;

                // Update FPS counter every second
                let now = std::time::Instant::now();
                if now.duration_since(last_fps_time) >= Duration::from_secs(1) {
                    displayed_fps = render_count;
                    render_count = 0;
                    last_fps_time = now;
                }

                let mut stderr = std::io::stderr().lock();
                if !output.is_empty() {
                    let _ = write!(stderr, "{}", output);
                }
                // FPS overlay in top-right corner
                let fps_str = format!(" {}fps ", displayed_fps);
                let fps_col = term_width.saturating_sub(fps_str.len());
                let _ = write!(stderr, "\x1B[{};{}H\x1B[90m{}\x1B[0m", start_row, fps_col + 1, fps_str);
                let _ = stderr.flush();

                prev = Some(buf);
            }
        }
    });

    // Wait for duration
    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    let _ = anim_handle.await;
    let _ = render_handle.await;

    // Move cursor below rendered area and show it
    {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", start_row + height);
        let _ = stderr.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_from_text() {
        let buf = FrameBuffer::from_text("ab\ncd", Color::new(255, 255, 255));
        assert_eq!(buf.width, 2);
        assert_eq!(buf.height, 2);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(1, 1).ch, 'd');
    }

    #[test]
    fn framebuffer_clear() {
        let mut buf = FrameBuffer::from_text("hello", Color::new(255, 0, 0));
        buf.clear();
        assert_eq!(buf.get(0, 0).ch, ' ');
        assert_eq!(buf.get(4, 0).ch, ' ');
    }

    #[test]
    fn diff_render_first_frame() {
        let buf = FrameBuffer::from_text("hi", Color::new(255, 255, 255));
        let output = diff_render(None, &buf, 1);
        assert!(output.contains('h'));
        assert!(output.contains('i'));
    }

    #[test]
    fn diff_render_no_change() {
        let buf = FrameBuffer::from_text("hi", Color::new(255, 255, 255));
        let output = diff_render(Some(&buf), &buf, 1);
        // No visible chars should be emitted — nothing changed
        assert!(!output.contains('h'));
    }

    #[test]
    fn diff_render_partial_change() {
        let buf1 = FrameBuffer::from_text("hi", Color::new(255, 255, 255));
        let mut buf2 = buf1.clone();
        buf2.set(1, 0, Cell::new('o', Color::new(255, 0, 0)));
        let output = diff_render(Some(&buf1), &buf2, 1);
        assert!(!output.contains('h')); // unchanged
        assert!(output.contains('o'));  // changed
    }
}
