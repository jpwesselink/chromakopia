//! Framebuffer-based terminal renderer.
//!
//! Two tasks at the same fixed framerate:
//! - Animation task: runs effects, writes to a grid, posts to mailbox
//! - Render task: diffs grid against previous frame, flushes changed cells

use crate::color::Color;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Default text color used when effects haven't colored a cell yet.
pub const DEFAULT_TEXT_COLOR: Color = Color { r: 204, g: 204, b: 204 };

/// Animation framerate. All seconds→frames conversions use this.
pub const FPS: f64 = 30.0;

/// Convert seconds to frames.
pub fn secs_to_frames(seconds: f64) -> usize {
    (seconds * FPS).round() as usize
}

/// A single cell in the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub color: Color,
    /// Optional background color. When set, emits `\x1B[48;2;r;g;bm`.
    pub bg: Option<Color>,
}

impl Cell {
    pub fn new(ch: char, color: Color) -> Self {
        Self { ch, color, bg: None }
    }

    pub fn with_bg(ch: char, color: Color, bg: Color) -> Self {
        Self { ch, color, bg: Some(bg) }
    }

    pub fn space() -> Self {
        Self { ch: ' ', color: Color::new(0, 0, 0), bg: None }
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

    /// Width of actual content (rightmost non-space column + 1).
    /// Color effects use this instead of buf.width so hue/position
    /// calculations scale to the text, not the buffer.
    pub fn content_width(&self) -> usize {
        let mut max = 0;
        for y in 0..self.height {
            for x in (0..self.width).rev() {
                if self.cells[y * self.width + x].ch != ' ' {
                    max = max.max(x + 1);
                    break;
                }
            }
        }
        max.max(1)
    }

    /// Render the framebuffer to an ANSI string — no cursor positioning, no
    /// terminal takeover. Use this for inline animations (progress bars,
    /// single-line effects, etc.).
    pub fn to_ansi_string(&self) -> String {
        let mut out = String::with_capacity(self.width * self.height * 4);
        let mut last_fg: Option<Color> = None;
        let mut last_bg: Option<Option<Color>> = None;

        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            for x in 0..self.width {
                let cell = self.get(x, y);

                if last_bg != Some(cell.bg) {
                    match cell.bg {
                        Some(bg) => out.push_str(&format!("\x1B[48;2;{};{};{}m", bg.r, bg.g, bg.b)),
                        None => {
                            if last_bg.is_some() && last_bg != Some(None) {
                                out.push_str("\x1B[49m");
                            }
                        }
                    }
                    last_bg = Some(cell.bg);
                }

                if last_fg != Some(cell.color) {
                    out.push_str(&format!("\x1B[38;2;{};{};{}m", cell.color.r, cell.color.g, cell.color.b));
                    last_fg = Some(cell.color);
                }
                out.push(cell.ch);
            }
        }

        if last_fg.is_some() || last_bg.is_some() {
            out.push_str("\x1B[0m");
        }
        out
    }
}

/// A framebuffer-based effect: writes directly to a grid.
pub trait Effect: Send + 'static {
    fn render(&self, buf: &mut FrameBuffer, frame: usize);

    /// Inherent size of this effect's content (width, height).
    /// Used by Scene for layout. Returns (0, 0) for pure color transforms.
    fn size(&self) -> (usize, usize) { (0, 0) }
}

/// Wraps any effect with text. Writes text into the buffer before the
/// effect runs, and reports size for Scene layout.
pub struct On<E> {
    lines: Vec<Vec<char>>,
    w: usize,
    h: usize,
    pub(crate) effect: E,
}

impl<E: Effect> On<E> {
    /// Spawn in a terminal area sized to the text. Runs until `.stop()`.
    pub fn spawn(self) -> AnimationHandle {
        let (w, h) = self.size();
        spawn_effect(self, w.max(1), h.max(1), 1.0)
    }

    /// Run in a terminal area sized to the text for `seconds`, then stop.
    pub async fn run(self, seconds: f64) {
        let (w, h) = self.size();
        run_effect(self, w.max(1), h.max(1), Duration::from_secs_f64(seconds), 1.0).await;
    }

    /// Render a single frame to an ANSI string. For inline use.
    pub fn frame(&self, frame: usize) -> String {
        let mut buf = FrameBuffer::new(self.w.max(1), self.h.max(1));
        <Self as Effect>::render(self, &mut buf, frame);
        buf.to_ansi_string()
    }
}

impl<E: Effect> Effect for On<E> {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        for (y, line) in self.lines.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x < buf.width && y < buf.height {
                    buf.set(x, y, Cell::new(ch, DEFAULT_TEXT_COLOR));
                }
            }
        }
        self.effect.render(buf, frame);
    }

    fn size(&self) -> (usize, usize) { (self.w, self.h) }
}

/// Extension trait: `.on(text)` wraps any effect with text.
pub trait EffectExt: Effect + Sized {
    fn on(self, text: &str) -> On<Self> {
        let lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
        let h = lines.len();
        let w = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        On { lines, w, h, effect: self }
    }
}

impl<E: Effect> EffectExt for E {}

/// Shared mailbox between animation and renderer.
type Mailbox = Arc<Mutex<Option<FrameBuffer>>>;

/// Commands sent from AnimationHandle to the animation task.
#[allow(dead_code)]
enum Command {
    FadeOut {
        frames: usize,
        color: Color,
        easing: super::easing::Easing,
    },
    TransitionTo {
        effect: Box<dyn Effect>,
        frames: usize,
        easing: super::easing::Easing,
    },
    Stop,
}

type CommandSlot = Arc<Mutex<Option<Command>>>;

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
    let mut last_fg: Option<Color> = None;
    let mut last_bg: Option<Option<Color>> = None;

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

                // Background color
                if last_bg != Some(cell.bg) {
                    match cell.bg {
                        Some(bg) => out.push_str(&format!("\x1B[48;2;{};{};{}m", bg.r, bg.g, bg.b)),
                        None => out.push_str("\x1B[49m"),
                    }
                    last_bg = Some(cell.bg);
                }

                // Foreground color
                if last_fg != Some(cell.color) {
                    out.push_str(&format!("\x1B[38;2;{};{};{}m", cell.color.r, cell.color.g, cell.color.b));
                    last_fg = Some(cell.color);
                }
                out.push(cell.ch);
            } else {
                need_position = true;
            }
        }
    }

    if last_fg.is_some() || last_bg.is_some() {
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

/// Handle for a running animation.
pub struct AnimationHandle {
    running: Arc<std::sync::atomic::AtomicBool>,
    command: CommandSlot,
    height: usize,
    start_row: usize,
}

impl AnimationHandle {
    /// Hard stop — kills the animation immediately.
    pub fn stop(&self) {
        use std::io::Write;
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(50));
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", self.start_row + self.height);
        let _ = stderr.flush();
    }

    /// Fade out over `seconds` to background color (EaseOut), then stop.
    pub fn fade_out(&self, seconds: f64) {
        self.fade_out_with(crate::terminal::bg_color(), seconds, super::easing::Easing::EaseOut);
    }

    /// Fade out over `seconds` to a specific color (EaseOut), then stop.
    pub fn fade_out_to(&self, color: Color, seconds: f64) {
        self.fade_out_with(color, seconds, super::easing::Easing::EaseOut);
    }

    /// Fade out with full control over color, duration, and easing.
    pub fn fade_out_with(&self, color: Color, seconds: f64, easing: super::easing::Easing) {
        let frames = (seconds * 30.0).round() as usize;
        *self.command.lock().unwrap() = Some(Command::FadeOut { frames, color, easing });
    }

    /// Crossfade to a new effect over `seconds` (EaseInOut).
    pub fn transition_to(&self, effect: impl Effect, seconds: f64) {
        self.transition_to_with(effect, seconds, super::easing::Easing::EaseInOut);
    }

    /// Crossfade to a new effect with custom easing.
    pub fn transition_to_with(&self, effect: impl Effect, seconds: f64, easing: super::easing::Easing) {
        let frames = (seconds * 30.0).round() as usize;
        *self.command.lock().unwrap() = Some(Command::TransitionTo {
            effect: Box::new(effect),
            frames,
            easing,
        });
    }

    /// Wait for the animation to finish (after fade_out or transition completes).
    pub async fn wait(&self) {
        while self.running.load(std::sync::atomic::Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
    }
}

/// Spawn an effect that runs until stopped. Returns a handle.
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
    let command: CommandSlot = Arc::new(Mutex::new(None));

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
    let cmd = command.clone();
    tokio::spawn(async move {
        let mut frame: usize = 0;
        let mut effect: Box<dyn Effect> = Box::new(effect);
        let mut buf = FrameBuffer::new(width, height);
        let mut interval = tokio::time::interval(frame_duration);

        // Fade state: (color, easing, total_frames, elapsed)
        let mut fade: Option<(Color, super::easing::Easing, usize, usize)> = None;
        // Transition state: (old_effect, easing, total_frames, elapsed, new_frame_offset)
        let mut transition: Option<(Box<dyn Effect>, super::easing::Easing, usize, usize, usize)> = None;

        while r.load(Ordering::Relaxed) {
            interval.tick().await;

            // Check for commands
            if let Some(cmd) = cmd.lock().unwrap().take() {
                match cmd {
                    Command::FadeOut { frames, color, easing } => {
                        fade = Some((color, easing, frames, 0));
                    }
                    Command::TransitionTo { effect: new_effect, frames, easing } => {
                        let old = std::mem::replace(&mut effect, new_effect);
                        transition = Some((old, easing, frames, 0, frame));
                    }
                    Command::Stop => {
                        r.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }

            // Render current effect
            let effect_frame = if let Some((_, _, _, _, offset)) = &transition {
                frame - offset
            } else {
                frame
            };
            effect.render(&mut buf, effect_frame);

            // Crossfade with old effect during transition
            if let Some((ref old_effect, ref easing, total, ref mut elapsed, _)) = transition {
                let mut old_buf = FrameBuffer::new(width, height);
                old_effect.render(&mut old_buf, frame);

                *elapsed += 1;
                let t = easing.apply((*elapsed as f64 / total.max(1) as f64).min(1.0));

                for y in 0..buf.height {
                    for x in 0..buf.width {
                        let old_c = old_buf.get(x, y).color;
                        let new_c = buf.get(x, y).color;
                        buf.set_color(x, y, Color::lerp_rgb(old_c, new_c, t));
                    }
                }

                if *elapsed >= total {
                    transition = None;
                }
            }

            // Apply fade on top
            if let Some((color, ref easing, total, ref mut elapsed)) = fade {
                *elapsed += 1;
                let t = easing.apply((*elapsed as f64 / total.max(1) as f64).min(1.0));
                for y in 0..buf.height {
                    for x in 0..buf.width {
                        let cell = buf.get(x, y);
                        buf.set_color(x, y, Color::lerp_rgb(cell.color, color, t));
                    }
                }
                if *elapsed >= total {
                    *m.lock().unwrap() = Some(buf.clone());
                    r.store(false, Ordering::Relaxed);
                    break;
                }
            }

            *m.lock().unwrap() = Some(buf.clone());
            frame += 1;
        }

        // Cleanup: show cursor, move below
        std::thread::sleep(Duration::from_millis(50));
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", start_row + height);
        let _ = stderr.flush();
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

    AnimationHandle { running, command, height, start_row }
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
    fn to_ansi_string_single_line() {
        let mut buf = FrameBuffer::new(3, 1);
        let red = Color::new(255, 0, 0);
        buf.set(0, 0, Cell::new('a', red));
        buf.set(1, 0, Cell::new('b', red));
        buf.set(2, 0, Cell::new('c', red));
        let s = buf.to_ansi_string();
        // One color code, then all three chars, then reset
        assert!(s.contains("abc"));
        assert!(s.ends_with("\x1B[0m"));
        // No cursor positioning
        assert!(!s.contains("H"));
    }

    #[test]
    fn to_ansi_string_multiline() {
        let buf = FrameBuffer::from_text("ab\ncd", Color::new(255, 255, 255));
        let s = buf.to_ansi_string();
        assert!(s.contains("ab\n"));
        assert!(s.contains("cd"));
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
