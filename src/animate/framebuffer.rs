//! Framebuffer-based terminal renderer.
//!
//! Two tasks at the same fixed framerate:
//! - Animation task: runs effects, writes to a grid, posts to mailbox
//! - Render task: diffs grid against previous frame, flushes changed cells

use crate::color::Color;

/// Default text color used when effects haven't colored a cell yet.
pub const DEFAULT_TEXT_COLOR: Color = Color {
    r: 204,
    g: 204,
    b: 204,
};

/// Animation framerate. All seconds→frames conversions use this.
pub const FPS: f64 = 60.0;

/// Convert seconds to frames.
pub fn secs_to_frames(seconds: f64) -> usize {
    (seconds * FPS).round() as usize
}

/// Frame count in "tempo units" — one unit = one frame at the original
/// 30fps tuning. Phase-accumulating effects multiply their speed knobs
/// by this instead of the raw frame count, so tuned constants keep the
/// same on-screen speed at any FPS.
pub fn tempo(frame: usize) -> f64 {
    frame as f64 * 30.0 / FPS
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
        Self {
            ch,
            color,
            bg: None,
        }
    }

    pub fn with_bg(ch: char, color: Color, bg: Color) -> Self {
        Self {
            ch,
            color,
            bg: Some(bg),
        }
    }

    pub fn space() -> Self {
        Self {
            ch: ' ',
            color: Color::new(0, 0, 0),
            bg: None,
        }
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

    /// Grow the buffer by appending blank rows at the bottom.
    pub fn grow(&mut self, rows: usize) {
        self.cells
            .extend(std::iter::repeat(Cell::space()).take(self.width * rows));
        self.height += rows;
    }

    /// Scroll content up by `n` rows. Top rows are discarded, bottom fills with spaces.
    pub fn scroll_up(&mut self, n: usize) {
        let shift = n.min(self.height) * self.width;
        self.cells.drain(..shift);
        self.cells
            .extend(std::iter::repeat(Cell::space()).take(shift));
    }

    /// Render to plain text — no ANSI codes. Just characters and newlines.
    /// Use when `color_enabled()` is false (CI, piped output, NO_COLOR).
    pub fn to_plain_string(&self) -> String {
        let mut out = String::with_capacity(self.width * self.height + self.height);
        for y in 0..self.height {
            if y > 0 {
                out.push('\n');
            }
            // Trim trailing spaces per row
            let mut row_end = self.width;
            while row_end > 0 && self.cells[y * self.width + row_end - 1].ch == ' ' {
                row_end -= 1;
            }
            for x in 0..row_end {
                out.push(self.cells[y * self.width + x].ch);
            }
        }
        out
    }

    /// Auto-detect: returns ANSI string if colors enabled, plain text otherwise.
    pub fn to_string_auto(&self) -> String {
        if crate::color_enabled() {
            self.to_ansi_string()
        } else {
            self.to_plain_string()
        }
    }

    /// Render rows `start..start+count` to an ANSI string, then release:
    /// returns the string and marks those rows as "done" (they won't be
    /// diffed or rewritten by the renderer).
    pub fn freeze_rows(&self, start: usize, count: usize) -> String {
        let end = (start + count).min(self.height);
        let mut out = String::new();
        let mut last_fg: Option<Color> = None;
        for y in start..end {
            if y > start {
                out.push('\n');
            }
            for x in 0..self.width {
                let cell = self.get(x, y);
                if last_fg != Some(cell.color) {
                    out.push_str(&format!(
                        "\x1B[38;2;{};{};{}m",
                        cell.color.r, cell.color.g, cell.color.b
                    ));
                    last_fg = Some(cell.color);
                }
                out.push(cell.ch);
            }
        }
        if last_fg.is_some() {
            out.push_str("\x1B[0m");
        }
        out
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
                    out.push_str(&format!(
                        "\x1B[38;2;{};{};{}m",
                        cell.color.r, cell.color.g, cell.color.b
                    ));
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
    fn size(&self) -> (usize, usize) {
        (0, 0)
    }
}

/// Wraps any effect with text. Writes text into the buffer before the
/// effect runs, and reports size for Scene layout.
pub struct On<E> {
    lines: Vec<Vec<char>>,
    w: usize,
    h: usize,
    pub(crate) effect: E,
    pub(crate) quit_key: Option<char>,
}

impl<E: Effect> On<E> {
    /// Render a single frame. Returns ANSI string if colors enabled, plain text in CI.
    pub fn frame(&self, frame: usize) -> String {
        let mut buf = FrameBuffer::new(self.w.max(1), self.h.max(1));
        <Self as Effect>::render(self, &mut buf, frame);
        buf.to_string_auto()
    }

    /// Stop the animation early when `key` is pressed (case-insensitive).
    pub fn quit_key(mut self, key: char) -> Self {
        self.quit_key = Some(key);
        self
    }

    /// Spawn in a terminal area sized to the text. Runs until `.stop()`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn(self) -> AnimationHandle {
        let (w, h) = self.size();
        let quit_key = self.quit_key;
        spawn_effect(self, w.max(1), h.max(1), 1.0, quit_key)
    }

    /// Run in a terminal area sized to the text for `seconds`, then stop.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn run(self, seconds: f64) {
        use std::time::Duration;
        let (w, h) = self.size();
        let quit_key = self.quit_key;
        run_effect(
            self,
            w.max(1),
            h.max(1),
            Duration::from_secs_f64(seconds),
            1.0,
            quit_key,
        )
        .await;
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

    fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }
}

/// A bare effect paired with a quit key — created by `.quit_key()` on
/// effects that carry their own text (Glitch, Flap, Scroll, Spread, Dycp).
#[cfg(not(target_arch = "wasm32"))]
pub struct QuitKey<E> {
    effect: E,
    key: char,
}

#[cfg(not(target_arch = "wasm32"))]
impl<E: Effect> QuitKey<E> {
    pub fn new(effect: E, key: char) -> Self {
        Self { effect, key }
    }

    /// Spawn in a terminal area sized to the effect. Runs until `.stop()` or the key.
    pub fn spawn(self) -> AnimationHandle {
        let (w, h) = self.effect.size();
        spawn_effect(self.effect, w.max(1), h.max(1), 1.0, Some(self.key))
    }

    /// Run for `seconds`, or until the key is pressed.
    pub async fn run(self, seconds: f64) {
        let (w, h) = self.effect.size();
        run_effect(
            self.effect,
            w.max(1),
            h.max(1),
            Duration::from_secs_f64(seconds),
            1.0,
            Some(self.key),
        )
        .await;
    }
}

/// Extension trait: `.on(text)` wraps any effect with text.
pub trait EffectExt: Effect + Sized {
    fn on(self, text: &str) -> On<Self> {
        let lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
        let h = lines.len();
        let w = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        On {
            lines,
            w,
            h,
            effect: self,
            quit_key: None,
        }
    }

    /// Repaint this effect's glyphs through `palette`, by brightness.
    fn recolor(self, palette: Vec<Color>) -> super::wrappers::Recolor {
        super::wrappers::Recolor::new(self, palette)
    }

    /// Repaint this effect's glyphs through a cyclic `palette`, by hue.
    fn recolor_hue(self, palette: Vec<Color>) -> super::wrappers::Recolor {
        super::wrappers::Recolor::hue(self, palette)
    }

    /// Give this effect empty room: top, right, bottom, left (CSS order).
    fn pad(self, top: usize, right: usize, bottom: usize, left: usize) -> super::wrappers::Padded {
        super::wrappers::Padded::new(self, top, right, bottom, left)
    }

    /// Play this effect `k` times as fast (0.5 = half speed).
    fn time_scale(self, k: f64) -> super::wrappers::TimeScale {
        super::wrappers::TimeScale::new(self, k)
    }
}

impl<E: Effect> EffectExt for E {}

// ── Animation runtime (terminal only, not available on WASM) ──────────────────

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use super::keys::{parse_keys, Key, KeyHandler};

/// Commands sent from AnimationHandle to the animation task.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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
        if libc::tcgetattr(fd, &mut t) != 0 {
            return None;
        }
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
                if buf[..len].contains(&b'R') {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &old);
    }

    let s = std::str::from_utf8(&buf[..len]).ok()?;
    let inner = s.strip_prefix("\x1B[")?.strip_suffix('R')?;
    let row_str = inner.split(';').next()?;
    row_str.parse().ok()
}

#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn get_cursor_row() -> Option<usize> {
    None
}

/// Watch `/dev/tty` for keys. `quit` (case-insensitive) flips `running` to
/// false; every other key goes to `on_key`. Clears only ICANON|ECHO — ISIG
/// stays on so Ctrl-C still works. Restores the original termios when the
/// thread exits. Returns a flag that turns true once quit was pressed.
#[cfg(unix)]
fn spawn_key_listener(
    quit: Option<char>,
    on_key: Option<KeyHandler>,
    running: Arc<std::sync::atomic::AtomicBool>,
) -> Arc<std::sync::atomic::AtomicBool> {
    use std::io::Read;
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::{AtomicBool, Ordering};

    let pressed = Arc::new(AtomicBool::new(false));
    let flag = pressed.clone();
    std::thread::spawn(move || {
        let Ok(mut tty) = std::fs::OpenOptions::new().read(true).open("/dev/tty") else {
            return;
        };
        let fd = tty.as_raw_fd();

        let old = unsafe {
            let mut t = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut t) != 0 {
                return;
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
        let _guard = TermiosGuard { fd, original: old };

        let mut raw = old;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1; // read() returns after ≤100ms — lets us poll `running`
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return;
        }

        let mut bytes = [0u8; 32];
        'read: while running.load(Ordering::Relaxed) {
            match tty.read(&mut bytes) {
                Ok(0) => {}
                Ok(n) => {
                    for key in parse_keys(&bytes[..n]) {
                        if let (Key::Char(c), Some(q)) = (key, quit) {
                            if c.eq_ignore_ascii_case(&q) {
                                flag.store(true, Ordering::Relaxed);
                                running.store(false, Ordering::Relaxed);
                                break 'read;
                            }
                        }
                        if let Some(handler) = &on_key {
                            handler(key);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    pressed
}

#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn spawn_key_listener(
    _quit: Option<char>,
    _on_key: Option<KeyHandler>,
    _running: Arc<std::sync::atomic::AtomicBool>,
) -> Arc<std::sync::atomic::AtomicBool> {
    Arc::new(std::sync::atomic::AtomicBool::new(false))
}

/// Leave the terminal after an animation. Quit by key: wipe the whole
/// animation area and park the cursor where it began, as if it never
/// ran. Otherwise the last frame stays and the cursor goes below it.
#[cfg(not(target_arch = "wasm32"))]
fn finish(stderr: &mut impl std::io::Write, quit: bool, start_row: usize, height: usize) {
    if quit {
        let _ = write!(stderr, "\x1B[0m\x1B[{};1H\x1B[J\x1B[?25h", start_row);
    } else {
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", start_row + height);
    }
    let _ = stderr.flush();
}

#[cfg(not(target_arch = "wasm32"))]
fn synced(frame: String) -> String {
    // Synchronized output: the terminal shows the frame once it's all in,
    // never half drawn. Terminals without it ignore the codes.
    format!("\x1B[?2026h{frame}\x1B[?2026l")
}

/// A block cell that is one colour top to bottom goes out as a space on
/// that background: a filled rectangle every terminal draws edge to edge,
/// where a font's block glyph can leave seams or bleed into its
/// neighbours (Terminal.app) — and with no foreground to send.
fn solid(cell: Cell) -> Cell {
    let one = match cell.ch {
        '\u{2588}' => Some(cell.color),
        '\u{2580}' | '\u{2584}' if cell.bg == Some(cell.color) => Some(cell.color),
        _ => None,
    };
    one.map_or(cell, |c| Cell::with_bg(' ', c, c))
}

/// Diff two framebuffers and produce minimal ANSI output.
///
/// Uses absolute cursor positioning — no newlines, no scrolling.
#[cfg(not(target_arch = "wasm32"))]
fn diff_render(prev: Option<&FrameBuffer>, curr: &FrameBuffer, start_row: usize) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(curr.width * curr.height * 4);
    let mut last_fg: Option<Color> = None;
    let mut last_bg: Option<Option<Color>> = None;

    for y in 0..curr.height {
        // Unchanged cells since the cursor was last placed on this row
        // (None: it isn't on this row yet).
        let mut gap: Option<usize> = None;

        for x in 0..curr.width {
            let cell = curr.get(x, y);
            let changed = match prev {
                Some(p) if y < p.height && x < p.width => p.get(x, y) != cell,
                _ => true,
            };
            let cell = solid(cell);
            if !changed {
                gap = gap.map(|g| g + 1);
                continue;
            }
            match gap {
                None => {
                    let _ = write!(out, "\x1B[{};{}H", start_row + y, x + 1);
                }
                Some(0) => {}
                Some(n) => {
                    let _ = write!(out, "\x1B[{n}C");
                }
            }
            gap = Some(0);

            // Only the colours that show: a space is all background, a
            // full block all foreground. Both go in one code.
            let fg = (cell.ch != ' ' && last_fg != Some(cell.color)).then_some(cell.color);
            let bg = (cell.ch != '\u{2588}' && last_bg != Some(cell.bg)).then_some(cell.bg);
            if fg.is_some() || bg.is_some() {
                out.push_str("\x1B[");
                if let Some(c) = fg {
                    let _ = write!(out, "38;2;{};{};{}", c.r, c.g, c.b);
                    last_fg = Some(c);
                }
                if let Some(b) = bg {
                    if fg.is_some() {
                        out.push(';');
                    }
                    match b {
                        Some(c) => {
                            let _ = write!(out, "48;2;{};{};{}", c.r, c.g, c.b);
                        }
                        None => out.push_str("49"),
                    }
                    last_bg = Some(b);
                }
                out.push('m');
            }
            out.push(cell.ch);
        }
    }

    if last_fg.is_some() || last_bg.is_some() {
        out.push_str("\x1B[0m");
    }
    out
}

/// Run an effect with the framebuffer renderer.
///
/// One loop per frame: animate, diff against the previous frame, write.
/// Frame production and display are 1:1 — no overproduction, no mailbox.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_effect(
    effect: impl Effect,
    width: usize,
    height: usize,
    duration: Duration,
    speed: f64,
    quit_key: Option<char>,
) {
    run_effect_in(effect, width, height, duration, speed, quit_key, None, false).await
}

/// Restores the main screen if the process is killed by Ctrl-C while an
/// alternate-screen animation runs. Only async-signal-safe calls.
#[cfg(unix)]
extern "C" fn leave_alt_screen_and_exit(_sig: libc::c_int) {
    const RESTORE: &[u8] = b"\x1B[0m\x1B[?25h\x1B[?1049l";
    unsafe {
        libc::write(2, RESTORE.as_ptr().cast(), RESTORE.len());
        libc::_exit(130);
    }
}

/// `run_effect`, optionally on the terminal's alternate screen: drawn from
/// the top-left of a blank screen, and on exit (quit key, Ctrl-C or the
/// end) the terminal returns to exactly what was there before.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn run_effect_in(
    effect: impl Effect,
    width: usize,
    height: usize,
    duration: Duration,
    speed: f64,
    quit_key: Option<char>,
    on_key: Option<KeyHandler>,
    alt_screen: bool,
) {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    // CI / no-terminal: render one static frame to stderr and sleep
    if !crate::color_enabled() {
        let mut buf = FrameBuffer::new(width, height);
        effect.render(&mut buf, 0);
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{}", buf.to_plain_string());
        let _ = stderr.flush();
        tokio::time::sleep(duration).await;
        return;
    }

    let running = Arc::new(AtomicBool::new(true));

    let start_row = if alt_screen {
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGINT, leave_alt_screen_and_exit as *const () as libc::sighandler_t);
        }
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[?1049h\x1B[?25l\x1B[2J");
        let _ = stderr.flush();
        1
    } else {
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
        get_cursor_row().unwrap_or(1)
    };

    let quit_pressed = (quit_key.is_some() || on_key.is_some()).then(|| {
        // Force the OSC color probe now, while we still own the tty.
        crate::terminal::probe_colors();
        spawn_key_listener(quit_key, on_key, running.clone())
    });

    let frame_duration = Duration::from_secs_f64(1.0 / FPS / speed.max(0.001));
    let term_width = crate::terminal::terminal_width();
    let deadline = tokio::time::Instant::now() + duration;
    let mut interval = tokio::time::interval(frame_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut frame: usize = 0;
    let mut buf = FrameBuffer::new(width, height);
    let mut prev: Option<FrameBuffer> = None;
    let mut last_fps_time = std::time::Instant::now();
    let mut render_count: u32 = 0;
    let mut displayed_fps: u32 = 0;

    while running.load(Ordering::Relaxed) && tokio::time::Instant::now() < deadline {
        interval.tick().await;
        effect.render(&mut buf, frame);

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
        output.push_str(&format!(
            "\x1B[{};{}H\x1B[90m{}\x1B[0m",
            start_row,
            fps_col + 1,
            fps_str
        ));

        {
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(synced(output).as_bytes());
            let _ = stderr.flush();
        }
        prev = Some(buf.clone());
        frame += 1;
    }
    running.store(false, Ordering::Relaxed);

    if alt_screen {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[0m\x1B[?25h\x1B[?1049l");
        let _ = stderr.flush();
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
        }
        return;
    }
    let quit = quit_pressed.is_some_and(|p| p.load(Ordering::Relaxed));
    finish(&mut std::io::stderr().lock(), quit, start_row, height);
}

/// Handle for a running animation.
#[cfg(not(target_arch = "wasm32"))]
pub struct AnimationHandle {
    running: Arc<std::sync::atomic::AtomicBool>,
    command: CommandSlot,
    height: usize,
    start_row: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl AnimationHandle {
    /// Hard stop — kills the animation immediately.
    pub fn stop(&self) {
        use std::io::Write;
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(50));
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\x1B[{};1H\x1B[?25h", self.start_row + self.height);
        let _ = stderr.flush();
    }

    /// Fade out over `seconds` to background color (EaseOut), then stop.
    pub fn fade_out(&self, seconds: f64) {
        self.fade_out_with(
            crate::terminal::bg_color(),
            seconds,
            super::easing::Easing::EaseOut,
        );
    }

    /// Fade out over `seconds` to a specific color (EaseOut), then stop.
    pub fn fade_out_to(&self, color: Color, seconds: f64) {
        self.fade_out_with(color, seconds, super::easing::Easing::EaseOut);
    }

    /// Fade out with full control over color, duration, and easing.
    pub fn fade_out_with(&self, color: Color, seconds: f64, easing: super::easing::Easing) {
        let frames = secs_to_frames(seconds);
        *self.command.lock().unwrap() = Some(Command::FadeOut {
            frames,
            color,
            easing,
        });
    }

    /// Crossfade to a new effect over `seconds` (EaseInOut).
    pub fn transition_to(&self, effect: impl Effect, seconds: f64) {
        self.transition_to_with(effect, seconds, super::easing::Easing::EaseInOut);
    }

    /// Crossfade to a new effect with custom easing.
    pub fn transition_to_with(
        &self,
        effect: impl Effect,
        seconds: f64,
        easing: super::easing::Easing,
    ) {
        let frames = secs_to_frames(seconds);
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
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_effect(
    effect: impl Effect,
    width: usize,
    height: usize,
    speed: f64,
    quit_key: Option<char>,
) -> AnimationHandle {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    // CI / no-terminal: render one static frame, return a no-op handle
    if !crate::color_enabled() {
        let mut buf = FrameBuffer::new(width, height);
        effect.render(&mut buf, 0);
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{}", buf.to_plain_string());
        let _ = stderr.flush();
        let running = Arc::new(AtomicBool::new(false));
        let command: CommandSlot = Arc::new(Mutex::new(None));
        return AnimationHandle {
            running,
            command,
            height,
            start_row: 0,
        };
    }

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

    let quit_pressed = quit_key.map(|key| {
        // Force the OSC color probe now, while we still own the tty.
        crate::terminal::probe_colors();
        spawn_key_listener(Some(key), None, running.clone())
    });

    let frame_duration = Duration::from_secs_f64(1.0 / FPS / speed.max(0.001));
    let term_width = crate::terminal::terminal_width();

    // Single loop: animate, diff, write — frame production and display 1:1.
    let r = running.clone();
    let cmd = command.clone();
    tokio::spawn(async move {
        let mut frame: usize = 0;
        let mut effect: Box<dyn Effect> = Box::new(effect);
        let mut buf = FrameBuffer::new(width, height);
        let mut prev: Option<FrameBuffer> = None;
        let mut interval = tokio::time::interval(frame_duration);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_fps_time = std::time::Instant::now();
        let mut render_count: u32 = 0;
        let mut displayed_fps: u32 = 0;

        // Fade state: (color, easing, total_frames, elapsed)
        let mut fade: Option<(Color, super::easing::Easing, usize, usize)> = None;
        // Transition state: (old_effect, easing, total_frames, elapsed, new_frame_offset)
        let mut transition: Option<(Box<dyn Effect>, super::easing::Easing, usize, usize, usize)> =
            None;

        while r.load(Ordering::Relaxed) {
            interval.tick().await;

            // Check for commands
            if let Some(cmd) = cmd.lock().unwrap().take() {
                match cmd {
                    Command::FadeOut {
                        frames,
                        color,
                        easing,
                    } => {
                        fade = Some((color, easing, frames, 0));
                    }
                    Command::TransitionTo {
                        effect: new_effect,
                        frames,
                        easing,
                    } => {
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
                    // Draw this final faded frame below, then stop.
                    r.store(false, Ordering::Relaxed);
                }
            }

            // Diff + fps counter + write
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
            output.push_str(&format!(
                "\x1B[{};{}H\x1B[90m{}\x1B[0m",
                start_row,
                fps_col + 1,
                fps_str
            ));
            {
                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(synced(output).as_bytes());
                let _ = stderr.flush();
            }
            prev = Some(buf.clone());
            frame += 1;
        }

        let quit = quit_pressed.is_some_and(|p| p.load(Ordering::Relaxed));
        finish(&mut std::io::stderr().lock(), quit, start_row, height);
    });

    AnimationHandle {
        running,
        command,
        height,
        start_row,
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
    fn on_quit_key_builder_stores_key() {
        struct Noop;
        impl Effect for Noop {
            fn render(&self, _buf: &mut FrameBuffer, _frame: usize) {}
        }
        let on = Noop.on("hi").quit_key('x');
        assert_eq!(on.quit_key, Some('x'));
        assert_eq!(Noop.on("hi").quit_key, None);
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
    fn diff_render_sets_both_colours_in_one_code() {
        let mut buf = FrameBuffer::new(1, 1);
        buf.set(0, 0, Cell::with_bg('\u{2580}', Color::new(1, 2, 3), Color::new(4, 5, 6)));
        let output = diff_render(None, &buf, 1);
        assert!(output.contains("\x1B[38;2;1;2;3;48;2;4;5;6m\u{2580}"), "{output:?}");
    }

    #[test]
    fn diff_render_skips_colours_nobody_sees() {
        // A space shows only its background, a full block only its foreground.
        let mut buf = FrameBuffer::new(4, 1);
        let bg = Color::new(9, 9, 9);
        buf.set(0, 0, Cell::with_bg(' ', Color::new(1, 1, 1), bg));
        buf.set(1, 0, Cell::with_bg(' ', Color::new(2, 2, 2), bg));
        buf.set(2, 0, Cell::with_bg('\u{2588}', Color::new(3, 3, 3), Color::new(7, 7, 7)));
        buf.set(3, 0, Cell::with_bg('\u{2588}', Color::new(3, 3, 3), Color::new(8, 8, 8)));
        let output = diff_render(None, &buf, 1);
        assert_eq!(output.matches("\x1B[").count(), 4, "{output:?}"); // move, bg, bg, reset
        assert!(output.contains("  \x1B[48;2;3;3;3m  "), "{output:?}"); // full blocks go out as filled spaces
    }

    #[test]
    fn diff_render_steps_over_short_gaps() {
        let buf1 = FrameBuffer::from_text("abcdefgh", Color::new(255, 255, 255));
        let mut buf2 = buf1.clone();
        buf2.set(0, 0, Cell::new('x', Color::new(255, 255, 255)));
        buf2.set(4, 0, Cell::new('y', Color::new(255, 255, 255)));
        let output = diff_render(Some(&buf1), &buf2, 1);
        assert_eq!(output.matches('H').count(), 1, "{output:?}");
        assert!(output.contains("x\x1B[3Cy"), "{output:?}");
    }

    /// A tiny terminal: plays `diff_render`'s output (cursor moves, short
    /// hops, SGR colours, characters) onto a grid of (char, fg, bg).
    struct Term {
        w: usize,
        cells: Vec<(char, Option<Color>, Option<Color>)>,
        x: usize,
        y: usize,
        fg: Option<Color>,
        bg: Option<Color>,
    }

    impl Term {
        fn new(w: usize, h: usize) -> Self {
            Term { w, cells: vec![(' ', None, None); w * h], x: 0, y: 0, fg: None, bg: None }
        }

        fn play(&mut self, s: &str) {
            let mut chars = s.chars().peekable();
            while let Some(c) = chars.next() {
                if c != '\x1B' {
                    self.cells[self.y * self.w + self.x] = (c, self.fg, self.bg);
                    self.x += 1;
                    continue;
                }
                assert_eq!(chars.next(), Some('['));
                let mut params = String::new();
                let end = loop {
                    let c = chars.next().unwrap();
                    if c.is_ascii_alphabetic() {
                        break c;
                    }
                    params.push(c);
                };
                let nums: Vec<u8> = params.split(';').map(|p| p.parse::<u16>().unwrap_or(0) as u8).collect();
                match end {
                    'H' => {
                        self.y = nums[0] as usize - 1;
                        self.x = nums[1] as usize - 1;
                    }
                    'C' => self.x += nums[0] as usize,
                    'm' => {
                        let mut i = 0;
                        while i < nums.len() {
                            match nums[i] {
                                0 => (self.fg, self.bg) = (None, None),
                                38 => {
                                    self.fg = Some(Color::new(nums[i + 2], nums[i + 3], nums[i + 4]));
                                    i += 4;
                                }
                                48 => {
                                    self.bg = Some(Color::new(nums[i + 2], nums[i + 3], nums[i + 4]));
                                    i += 4;
                                }
                                49 => self.bg = None,
                                n => panic!("unexpected SGR {n}"),
                            }
                            i += 1;
                        }
                    }
                    e => panic!("unexpected CSI {e}"),
                }
            }
        }

        /// Whether the screen shows `buf`: block cells and spaces by the
        /// colours of their top and bottom halves, anything else by its
        /// character and both colours.
        fn shows(&self, buf: &FrameBuffer) -> Result<(), String> {
            fn halves(ch: char, fg: Option<Color>, bg: Option<Color>) -> Option<[Option<Color>; 2]> {
                match ch {
                    ' ' => Some([bg, bg]),
                    '\u{2588}' => Some([fg, fg]),
                    '\u{2580}' => Some([fg, bg]),
                    '\u{2584}' => Some([bg, fg]),
                    _ => None,
                }
            }
            for y in 0..buf.height {
                for x in 0..buf.width {
                    let want = buf.get(x, y);
                    let (ch, fg, bg) = self.cells[y * self.w + x];
                    let ok = match (halves(want.ch, Some(want.color), want.bg), halves(ch, fg, bg)) {
                        (Some(a), Some(b)) => a == b,
                        _ => ch == want.ch && fg == Some(want.color) && bg == want.bg,
                    };
                    if !ok {
                        return Err(format!("({x},{y}): screen {ch:?} {fg:?} {bg:?}, frame {want:?}"));
                    }
                }
            }
            Ok(())
        }
    }

    #[test]
    fn played_on_a_terminal_every_frame_shows_exactly() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let (w, h) = (40, 12);
        let palette = [Color::new(10, 20, 30), Color::new(200, 100, 50), Color::new(0, 0, 0), Color::new(255, 255, 255)];
        let glyphs = [' ', '\u{2580}', '\u{2584}', '\u{2588}', 'a'];
        let mut term = Term::new(w, h);
        let mut prev: Option<FrameBuffer> = None;
        let mut buf = FrameBuffer::new(w, h);
        for frame in 0..300 {
            // Change a few cells, sometimes a lot, the way scenes do.
            let changes = if frame % 50 == 0 { w * h } else { rng.random_range(0..40) };
            for _ in 0..changes {
                let (x, y) = (rng.random_range(0..w), rng.random_range(0..h));
                let ch = glyphs[rng.random_range(0..glyphs.len())];
                let fg = palette[rng.random_range(0..palette.len())];
                let bg = if rng.random_bool(0.2) { None } else { Some(palette[rng.random_range(0..palette.len())]) };
                buf.set(x, y, Cell { ch, color: fg, bg });
            }
            term.play(&diff_render(prev.as_ref(), &buf, 1));
            if let Err(e) = term.shows(&buf) {
                panic!("frame {frame}: {e}");
            }
            prev = Some(buf.clone());
        }
    }

    #[test]
    fn frames_are_drawn_in_one_go() {
        let out = synced("abc".to_string());
        assert_eq!(out, "\x1B[?2026habc\x1B[?2026l");
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
