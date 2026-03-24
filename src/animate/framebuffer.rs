//! Framebuffer-based terminal renderer.
//!
//! Two tasks at the same fixed framerate:
//! - Animation task: runs effects, writes to a grid, posts to mailbox
//! - Render task: diffs grid against previous frame, flushes changed cells

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

    #[inline]
    pub fn set_color(&mut self, x: usize, y: usize, color: Color) {
        self.cells[y * self.width + x].color = color;
    }

    #[inline]
    pub fn set_char(&mut self, x: usize, y: usize, ch: char) {
        self.cells[y * self.width + x].ch = ch;
    }

    pub fn write_line(&mut self, y: usize, text: &str, color: Color) {
        for (x, ch) in text.chars().enumerate() {
            if x < self.width {
                self.set(x, y, Cell::new(ch, color));
            }
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::space());
    }
}

/// A framebuffer-based effect: writes directly to a grid.
pub trait Effect: Send + 'static {
    fn render(&self, buf: &mut FrameBuffer, frame: usize);
}

/// Shared mailbox between animation and renderer.
type Mailbox = Arc<Mutex<Option<FrameBuffer>>>;

/// Get current cursor row via ANSI DSR (Device Status Report).
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
/// Uses absolute cursor positioning — no newlines, no scrolling.
fn diff_render(prev: Option<&FrameBuffer>, curr: &FrameBuffer, start_row: usize) -> String {
    let mut out = String::with_capacity(curr.width * curr.height * 4);
    let mut last_color: Option<Color> = None;

    for y in 0..curr.height {
        let mut need_position = true;

        for x in 0..curr.width {
            let cell = curr.get(x, y);
            let changed = match prev {
                Some(p) if y < p.height && x < p.width => p.get(x, y) != cell,
                _ => true,
            };

            if changed {
                if need_position {
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

/// Run an effect with the two-loop framebuffer renderer.
///
/// Both animation and render run at the same fixed framerate (60fps).
/// Animation produces a frame, render consumes it — no overproduction.
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

    // Hide cursor and reserve space
    {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[?25l");
        for _ in 0..height {
            let _ = write!(stderr, "\n");
        }
        let _ = write!(stderr, "\x1B[{}A", height);
        let _ = stderr.flush();
    }

    let start_row = get_cursor_row().unwrap_or(1);

    let fps = 30u64;
    let frame_ms = (1000.0 / fps as f64 / speed) as u64;
    let frame_duration = Duration::from_millis(frame_ms.max(1));

    // Animation task — same framerate as renderer
    let m = mailbox.clone();
    let r = running.clone();
    let anim_handle = tokio::spawn(async move {
        let mut frame: usize = 0;
        let mut buf = FrameBuffer::new(width, height);
        let mut interval = tokio::time::interval(frame_duration);
        while r.load(Ordering::Relaxed) {
            interval.tick().await;
            effect.render(&mut buf, frame);
            *m.lock().unwrap() = Some(buf.clone());
            frame += 1;
        }
    });

    // Render task — same framerate
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
                let mut output = diff_render(prev.as_ref(), &buf, start_row);
                render_count += 1;

                let now = std::time::Instant::now();
                if now.duration_since(last_fps_time) >= Duration::from_secs(1) {
                    displayed_fps = render_count;
                    render_count = 0;
                    last_fps_time = now;
                }

                let fps_str = format!(" {}fps ", displayed_fps);
                let fps_col = term_width.saturating_sub(fps_str.len());
                output.push_str(&format!("\x1B[{};{}H\x1B[90m{}\x1B[0m", start_row, fps_col + 1, fps_str));

                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(output.as_bytes());
                let _ = stderr.flush();

                prev = Some(buf);
            }
        }
    });

    tokio::time::sleep(duration).await;
    running.store(false, Ordering::Relaxed);

    let _ = anim_handle.await;
    let _ = render_handle.await;

    // Show cursor, move below rendered area
    {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", start_row + height);
        let _ = stderr.flush();
    }
}

/// Handle for a running animation. Call `.stop()` to end it.
pub struct AnimationHandle {
    running: Arc<std::sync::atomic::AtomicBool>,
    height: usize,
    start_row: usize,
}

impl AnimationHandle {
    /// Stop the animation and clean up the terminal.
    pub fn stop(&self) {
        use std::io::Write;
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        // Give tasks a moment to finish, then restore cursor
        std::thread::sleep(Duration::from_millis(50));
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", self.start_row + self.height);
        let _ = stderr.flush();
    }
}

/// Spawn an effect that runs until stopped. Returns a handle.
///
/// Use this for the standalone `animate::rainbow(text, speed)` style API.
pub fn spawn_effect(
    effect: impl Effect,
    width: usize,
    height: usize,
    speed: f64,
) -> AnimationHandle {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mailbox: Mailbox = Arc::new(Mutex::new(None));
    let running = Arc::new(AtomicBool::new(true));

    // Hide cursor and reserve space
    {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[?25l");
        for _ in 0..height {
            let _ = write!(stderr, "\n");
        }
        let _ = write!(stderr, "\x1B[{}A", height);
        let _ = stderr.flush();
    }

    let start_row = get_cursor_row().unwrap_or(1);

    let fps = 30u64;
    let frame_ms = (1000.0 / fps as f64 / speed) as u64;
    let frame_duration = Duration::from_millis(frame_ms.max(1));

    // Animation task
    let m = mailbox.clone();
    let r = running.clone();
    tokio::spawn(async move {
        let mut frame: usize = 0;
        let mut buf = FrameBuffer::new(width, height);
        let mut interval = tokio::time::interval(frame_duration);
        while r.load(Ordering::Relaxed) {
            interval.tick().await;
            effect.render(&mut buf, frame);
            *m.lock().unwrap() = Some(buf.clone());
            frame += 1;
        }
    });

    // Render task
    let m = mailbox.clone();
    let r = running.clone();
    let term_width = crate::terminal::terminal_width();
    tokio::spawn(async move {
        let mut prev: Option<FrameBuffer> = None;
        let mut interval = tokio::time::interval(frame_duration);
        let mut last_fps_time = std::time::Instant::now();
        let mut render_count: u32 = 0;
        let mut displayed_fps: u32 = 0;

        while r.load(Ordering::Relaxed) {
            interval.tick().await;

            let new_frame = m.lock().unwrap().take();
            if let Some(buf) = new_frame {
                let mut output = diff_render(prev.as_ref(), &buf, start_row);
                render_count += 1;

                let now = std::time::Instant::now();
                if now.duration_since(last_fps_time) >= Duration::from_secs(1) {
                    displayed_fps = render_count;
                    render_count = 0;
                    last_fps_time = now;
                }

                let fps_str = format!(" {}fps ", displayed_fps);
                let fps_col = term_width.saturating_sub(fps_str.len());
                output.push_str(&format!("\x1B[{};{}H\x1B[90m{}\x1B[0m", start_row, fps_col + 1, fps_str));

                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(output.as_bytes());
                let _ = stderr.flush();

                prev = Some(buf);
            }
        }
    });

    AnimationHandle { running, height, start_row }
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
        assert!(!output.contains('h'));
    }

    #[test]
    fn diff_render_partial_change() {
        let buf1 = FrameBuffer::from_text("hi", Color::new(255, 255, 255));
        let mut buf2 = buf1.clone();
        buf2.set(1, 0, Cell::new('o', Color::new(255, 0, 0)));
        let output = diff_render(Some(&buf1), &buf2, 1);
        assert!(!output.contains('h'));
        assert!(output.contains('o'));
    }
}
