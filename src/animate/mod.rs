//! Animated terminal effects — text coloring, transitions, and demoscene-style animations.
//!
//! ```no_run
//! # async fn example() {
//! let anim = chromakopia::animate::rainbow("Loading...", 1.0);
//! // ... do async work ...
//! anim.stop();
//! # }
//! ```

mod effects;
mod easing;

pub use easing::Easing;
pub use effects::ScrollDirection;

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::gradient::Gradient;

/// A handle to a running animation. Drop it or call `.stop()` to halt.
pub struct Animation {
    running: Arc<AtomicBool>,
    text: Arc<Mutex<String>>,
    handle: Option<JoinHandle<()>>,
    frame: Arc<AtomicUsize>,
    last_rendered: Arc<Mutex<String>>,
    lines_printed: Arc<AtomicUsize>,
    clear_on_stop: Arc<AtomicBool>,
}

impl Animation {
    /// Stop the animation and clear it from the terminal.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Replace the animated text mid-animation.
    pub fn replace(&self, new_text: &str) {
        let mut text = self.text.lock().unwrap();
        *text = new_text.to_string();
    }

    /// Resume a stopped animation.
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Get the current frame counter.
    pub fn frame(&self) -> usize {
        self.frame.load(Ordering::SeqCst)
    }

    /// Wait for the animation task to finish (only finishes when stopped).
    pub async fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.await;
        }
    }

    /// Stop the animation and fade to the terminal's foreground color.
    /// The text stays on screen as solid, readable text.
    pub async fn fade_to_foreground(self, duration: Duration) {
        self.fade_out_to(FadeTarget::Foreground, duration, true).await;
    }

    /// Stop the animation and fade to a static gradient.
    /// The text stays on screen with the gradient applied.
    pub async fn fade_to_gradient(self, gradient: Gradient, duration: Duration) {
        self.fade_out_to(FadeTarget::Gradient(gradient), duration, true).await;
    }

    /// Stop the animation and fade to background (disappear).
    pub async fn fade_to_background(self, duration: Duration) {
        self.fade_out_to(FadeTarget::Background, duration, false).await;
    }

    async fn fade_out_to(mut self, target: FadeTarget, duration: Duration, settle: bool) {
        // Tell the spawned task not to clear on stop
        self.clear_on_stop.store(false, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);

        // Wait for the animation task to finish
        if let Some(h) = self.handle.take() {
            let _ = h.await;
        }

        let last = self.last_rendered.lock().unwrap().clone();
        let text = self.text.lock().unwrap().clone();
        let mut lines_printed = self.lines_printed.load(Ordering::SeqCst);

        let delay = Duration::from_millis(30);
        let total_frames = (duration.as_millis() / 30).max(1) as usize;
        let easing = Easing::EaseOut;

        for frame in 0..=total_frames {
            let raw_t = frame as f64 / total_frames as f64;
            let eased_t = easing.apply(raw_t);
            let opacity = 1.0 - eased_t;

            let faded = match &target {
                FadeTarget::Background => apply_fade_toward(&last, opacity, crate::terminal::bg_color()),
                FadeTarget::Foreground => apply_fade_toward(&last, opacity, crate::terminal::fg_color()),
                FadeTarget::Color(c) => apply_fade_toward(&last, opacity, *c),
                FadeTarget::Gradient(g) => apply_fade_toward_gradient(&last, opacity, g, &text),
            };

            let mut buf = String::new();
            lines_printed = render_frame(&mut buf, &faded, lines_printed);
            {
                let mut stderr = std::io::stderr().lock();
                let _ = write!(stderr, "{}", buf);
                let _ = stderr.flush();
            }

            if frame < total_frames {
                tokio::time::sleep(delay).await;
            }
        }

        if settle {
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\n\x1B[?25h");
            let _ = stderr.flush();
        } else {
            let mut buf = String::new();
            if lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", lines_printed));
            }
            buf.push_str("\r\x1B[J\x1B[?25h\n");
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "{}", buf);
            let _ = stderr.flush();
        }
    }
}

impl Drop for Animation {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Don't abort — let the task finish its cleanup (cursor restore, line clear).
        // The task will see running=false on the next loop check and exit cleanly.
    }
}

fn spawn_animation<F>(text: &str, effect: F, delay_ms: u64, speed: f64) -> Animation
where
    F: Fn(&str, usize) -> String + Send + 'static,
{
    let running = Arc::new(AtomicBool::new(true));
    let text = Arc::new(Mutex::new(text.to_string()));
    let frame = Arc::new(AtomicUsize::new(0));
    let last_rendered = Arc::new(Mutex::new(String::new()));
    let lines_printed = Arc::new(AtomicUsize::new(0));
    let clear_on_stop = Arc::new(AtomicBool::new(true));

    let r = running.clone();
    let t = text.clone();
    let f = frame.clone();
    let lr = last_rendered.clone();
    let lp = lines_printed.clone();
    let cs = clear_on_stop.clone();
    let delay = Duration::from_millis((delay_ms as f64 / speed) as u64);

    let handle = tokio::spawn(async move {
        let mut local_lines_printed: usize = 0;

        // Eagerly probe terminal colors before any output
        crate::terminal::probe_colors();

        // Hide cursor
        {
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\x1B[?25l");
            let _ = stderr.flush();
        }

        while r.load(Ordering::SeqCst) {
            let current_frame = f.fetch_add(1, Ordering::SeqCst);
            let current_text = t.lock().unwrap().clone();
            let rendered = effect(&current_text, current_frame);

            // Store last rendered frame for fade methods
            *lr.lock().unwrap() = rendered.clone();

            let rendered = rendered.trim_end_matches('\n');
            let rendered_lines: Vec<&str> = rendered.split('\n').collect();
            let line_count = rendered_lines.len();

            let mut buf = String::new();
            if local_lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", local_lines_printed));
            }
            for (i, line) in rendered_lines.iter().enumerate() {
                buf.push('\r');
                buf.push_str(line);
                buf.push_str("\x1B[K");
                if i < line_count - 1 {
                    buf.push('\n');
                }
            }

            {
                let mut stderr = std::io::stderr().lock();
                let _ = write!(stderr, "{}", buf);
                let _ = stderr.flush();
            }

            local_lines_printed = line_count - 1;
            lp.store(local_lines_printed, Ordering::SeqCst);
            tokio::time::sleep(delay).await;
        }

        if cs.load(Ordering::SeqCst) {
            // Normal stop: clear the animation and show cursor
            let mut buf = String::new();
            if local_lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", local_lines_printed));
            }
            for i in 0..=local_lines_printed {
                buf.push_str("\r\x1B[K");
                if i < local_lines_printed {
                    buf.push('\n');
                }
            }
            if local_lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", local_lines_printed));
            }
            buf.push_str("\x1B[?25h\n");
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "{}", buf);
            let _ = stderr.flush();
        }
        // If !clear_on_stop, the fade method handles cleanup
    });

    Animation {
        running,
        text,
        handle: Some(handle),
        frame,
        last_rendered,
        lines_printed,
        clear_on_stop,
    }
}

// ── Render helper (shared by spawn_animation and Sequence) ──

fn render_frame(buf: &mut String, rendered: &str, lines_printed: usize) -> usize {
    let rendered = rendered.trim_end_matches('\n');
    let rendered_lines: Vec<&str> = rendered.split('\n').collect();
    let line_count = rendered_lines.len();
    let term_width = crate::terminal::terminal_width();

    if lines_printed > 0 {
        buf.push_str(&format!("\x1B[{}F", lines_printed));
    }
    for (i, line) in rendered_lines.iter().enumerate() {
        buf.push('\r');
        buf.push_str(&truncate_ansi(line, term_width));
        buf.push_str("\x1B[K");
        if i < line_count - 1 {
            buf.push('\n');
        }
    }
    line_count - 1
}

/// Truncate a string containing ANSI escape codes to `max_visible` visible characters.
///
/// Preserves escape sequences but stops emitting visible characters once the limit
/// is reached. Appends a reset sequence if truncation happened mid-color.
fn truncate_ansi(s: &str, max_visible: usize) -> String {
    let mut result = String::with_capacity(s.len());
    let mut visible = 0;
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if visible >= max_visible {
            // Append reset so we don't leak color into the next line
            result.push_str("\x1B[0m");
            break;
        }

        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // ANSI escape sequence — copy it through without counting as visible
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip the 'm'
            }
            result.push_str(&s[start..i]);
        } else {
            // Visible character (handle multi-byte UTF-8)
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 }
                else if byte < 0xE0 { 2 }
                else if byte < 0xF0 { 3 }
                else { 4 };
            let end = (i + char_len).min(bytes.len());
            result.push_str(&s[i..end]);
            visible += 1;
            i = end;
        }
    }

    result
}

// ── Sequence builder ──

type BoxEffect = Box<dyn Fn(&str, usize) -> String + Send + 'static>;

/// Target color for a fade transition.
pub enum FadeTarget {
    /// Fade toward terminal background (text disappears)
    Background,
    /// Fade toward terminal foreground (gradient calms down to solid text)
    Foreground,
    /// Fade toward a specific color
    Color(crate::color::Color),
    /// Fade toward a static gradient
    Gradient(Gradient),
}

/// A time range on the animation timeline, in seconds.
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    pub fn from_duration(start: Duration, end: Duration) -> Self {
        Self {
            start: start.as_secs_f64(),
            end: end.as_secs_f64(),
        }
    }

    fn contains(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }

    fn progress(&self, t: f64) -> f64 {
        let d = self.end - self.start;
        if d <= 0.0 {
            return 1.0;
        }
        ((t - self.start) / d).clamp(0.0, 1.0)
    }
}

struct EffectLayer {
    time: TimeRange,
    effect: BoxEffect,
    delay_ms: u64,
}

/// Direction of a fade transition.
pub enum FadeKind {
    /// Fade from target → effect colors (opacity 0→1)
    FadeFrom(FadeTarget),
    /// Fade from effect colors → target (opacity 1→0)
    FadeTo(FadeTarget),
}

struct FadeLayer {
    time: TimeRange,
    kind: FadeKind,
    easing: Easing,
}

/// Chain multiple animation effects into a sequence.
///
/// Effects and fades are placed on a shared timeline as composable layers.
/// The chaining API places them sequentially, but the internal model
/// supports overlapping layers for future power-user methods.
///
/// ```no_run
/// # async fn example() {
/// use std::time::Duration;
/// chromakopia::animate::Sequence::new("Hello, world!")
///     .fade_in(Duration::from_secs(1))
///     .glow(chromakopia::presets::dark_n_stormy(), Duration::from_secs(3))
///     .fade_out(Duration::from_secs(1))
///     .run(1.0).await;
/// # }
/// ```
pub struct Sequence {
    text: String,
    effect_layers: Vec<EffectLayer>,
    fade_layers: Vec<FadeLayer>,
    cursor: f64,
}

impl Sequence {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            effect_layers: Vec::new(),
            fade_layers: Vec::new(),
            cursor: 0.0,
        }
    }

    fn push_effect(&mut self, effect: BoxEffect, duration: Duration, delay_ms: u64) {
        let start = self.cursor;
        let end = start + duration.as_secs_f64();
        self.effect_layers.push(EffectLayer {
            time: TimeRange::new(start, end),
            effect,
            delay_ms,
        });
        self.cursor = end;
    }

    fn last_effect_range(&self) -> Option<(f64, f64)> {
        self.effect_layers.last().map(|e| (e.time.start, e.time.end))
    }

    /// Fade from black to white.
    pub fn fade_in(self, duration: Duration) -> Self {
        self.fade_in_color(crate::color::Color::new(255, 255, 255), duration)
    }

    /// Fade from black to a specific color.
    pub fn fade_in_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        let total = (duration.as_millis() / 30) as usize;
        self.push_effect(
            Box::new(move |text, frame| effects::fade_in(text, frame, total, color)),
            duration,
            30,
        );
        self
    }

    /// Fade from white to black.
    pub fn fade_out(self, duration: Duration) -> Self {
        self.fade_out_color(crate::color::Color::new(255, 255, 255), duration)
    }

    /// Fade from a specific color to black.
    pub fn fade_out_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        let total = (duration.as_millis() / 30) as usize;
        self.push_effect(
            Box::new(move |text, frame| effects::fade_out(text, frame, total, color)),
            duration,
            30,
        );
        self
    }

    /// Glow sweep with a gradient.
    pub fn glow(mut self, grad: Gradient, duration: Duration) -> Self {
        use crate::color::Color;
        let palette = grad.palette(3);
        let bright = palette[0];
        let mid = palette[palette.len() / 2];
        let dark = palette[palette.len() - 1];

        self.push_effect(
            Box::new(move |text, frame| {
                use colored::Colorize;
                let lines: Vec<&str> = text.split('\n').collect();
                let max_col = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                if max_col == 0 { return String::new(); }

                let glow_width = (max_col as f64 * 0.3).max(4.0);
                let period = max_col as f64 + glow_width * 2.0;
                let center = (frame as f64 * 0.4) % period - glow_width;

                lines.iter().map(|line| {
                    line.chars().enumerate().map(|(col, ch)| {
                        let dist = (col as f64 - center).abs();
                        let t = (dist / glow_width).min(1.0);
                        let t = t * t * (3.0 - 2.0 * t);
                        let c = if t < 0.5 {
                            Color::lerp_rgb(bright, mid, t * 2.0)
                        } else {
                            Color::lerp_rgb(mid, dark, (t - 0.5) * 2.0)
                        };
                        ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                    }).collect::<String>()
                }).collect::<Vec<_>>().join("\n")
            }),
            duration,
            30,
        );
        self
    }

    /// Rainbow effect.
    pub fn rainbow(mut self, duration: Duration) -> Self {
        self.push_effect(
            Box::new(effects::rainbow),
            duration,
            15,
        );
        self
    }

    /// Demoscene plasma effect with rainbow colors.
    pub fn plasma(mut self, duration: Duration) -> Self {
        self.push_effect(
            Box::new(|text, frame| effects::plasma(text, frame, None)),
            duration,
            30,
        );
        self
    }

    /// Demoscene plasma effect with a custom gradient.
    pub fn plasma_with(mut self, grad: Gradient, duration: Duration) -> Self {
        let palette = grad.palette(256);
        self.push_effect(
            Box::new(move |text, frame| effects::plasma(text, frame, Some(&palette))),
            duration,
            30,
        );
        self
    }

    /// Slide-in with bounce easing and rainbow colors.
    ///
    /// Text slides in from the given direction with a bounce at the end.
    /// The animation plays once and holds the final position.
    pub fn scroll(mut self, direction: effects::ScrollDirection, duration: Duration) -> Self {
        let fps = 30;
        let total_frames = (duration.as_secs_f64() * fps as f64) as usize;
        self.push_effect(
            Box::new(move |text, frame| effects::scroll(text, frame, total_frames, direction)),
            duration,
            fps,
        );
        self
    }

    /// Slide-in with bounce easing and a custom gradient.
    pub fn scroll_with(mut self, direction: effects::ScrollDirection, grad: Gradient, duration: Duration) -> Self {
        let fps = 30;
        let total_frames = (duration.as_secs_f64() * fps as f64) as usize;
        let gradient = grad.clone();
        self.push_effect(
            Box::new(move |text, frame| effects::scroll_with(text, frame, total_frames, direction, &gradient)),
            duration,
            fps,
        );
        self
    }

    /// Split-flap departure board with flughafen colors.
    pub fn flap(mut self, duration: Duration) -> Self {
        use crate::color::Color;
        let settled = Color::new(0xff, 0xcc, 0x00);
        let flipping = Color::new(0x99, 0x7a, 0x00);
        self.push_effect(
            Box::new(move |text, frame| effects::flap(text, frame, settled, flipping)),
            duration,
            60,
        );
        self
    }

    /// Split-flap with custom gradient colors.
    pub fn flap_with(mut self, grad: Gradient, duration: Duration) -> Self {
        let palette = grad.palette(2);
        let settled = palette[0];
        let flipping = palette[1];
        self.push_effect(
            Box::new(move |text, frame| effects::flap(text, frame, settled, flipping)),
            duration,
            60,
        );
        self
    }

    /// Cycle a gradient's colors.
    pub fn cycle(mut self, grad: Gradient, duration: Duration) -> Self {
        self.push_effect(
            Box::new(move |text, frame| {
                use colored::Colorize;
                let len = text.chars().filter(|c| !c.is_whitespace()).count().max(2);
                let palette = grad.palette(len * 2);
                let offset = frame % palette.len();
                let mut result = String::new();
                let mut color_idx = 0;
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        result.push(ch);
                    } else {
                        let c = palette[(color_idx + offset) % palette.len()];
                        result.push_str(&ch.to_string().truecolor(c.r, c.g, c.b).to_string());
                        color_idx += 1;
                    }
                }
                result
            }),
            duration,
            15,
        );
        self
    }

    /// Set fade-in and fade-out durations on the last added effect.
    ///
    /// Fades to/from the terminal background color (text appears/disappears).
    pub fn with_fade(mut self, fade_in: Duration, fade_out: Duration) -> Self {
        if let Some((start, end)) = self.last_effect_range() {
            // Remove any existing fade layers for this effect
            self.fade_layers.retain(|f| {
                let is_from_here = matches!(&f.kind, FadeKind::FadeFrom(_))
                    && (f.time.start - start).abs() < 0.001;
                let is_to_here = matches!(&f.kind, FadeKind::FadeTo(_))
                    && (f.time.end - end).abs() < 0.001;
                !is_from_here && !is_to_here
            });
            if fade_in > Duration::ZERO {
                self.fade_layers.push(FadeLayer {
                    time: TimeRange::new(start, start + fade_in.as_secs_f64()),
                    kind: FadeKind::FadeFrom(FadeTarget::Background),
                    easing: Easing::Linear,
                });
            }
            if fade_out > Duration::ZERO {
                self.fade_layers.push(FadeLayer {
                    time: TimeRange::new(end - fade_out.as_secs_f64(), end),
                    kind: FadeKind::FadeTo(FadeTarget::Background),
                    easing: Easing::Linear,
                });
            }
        }
        self
    }

    /// Fade the gradient into the terminal's foreground color.
    /// The text stays on screen as solid, readable text.
    pub fn fade_to_foreground(mut self, duration: Duration) -> Self {
        if let Some((_, end)) = self.last_effect_range() {
            self.fade_layers.retain(|f| {
                !(matches!(&f.kind, FadeKind::FadeTo(_)) && (f.time.end - end).abs() < 0.001)
            });
            self.fade_layers.push(FadeLayer {
                time: TimeRange::new(end - duration.as_secs_f64(), end),
                kind: FadeKind::FadeTo(FadeTarget::Foreground),
                easing: Easing::Linear,
            });
        }
        self
    }

    /// Fade the gradient into a specific color.
    /// The text stays on screen in that color.
    pub fn fade_to_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        if let Some((_, end)) = self.last_effect_range() {
            self.fade_layers.retain(|f| {
                !(matches!(&f.kind, FadeKind::FadeTo(_)) && (f.time.end - end).abs() < 0.001)
            });
            self.fade_layers.push(FadeLayer {
                time: TimeRange::new(end - duration.as_secs_f64(), end),
                kind: FadeKind::FadeTo(FadeTarget::Color(color)),
                easing: Easing::Linear,
            });
        }
        self
    }

    /// Fade the animation into a static gradient.
    /// The text stays on screen with the gradient applied.
    pub fn fade_to_gradient(mut self, grad: Gradient, duration: Duration) -> Self {
        if let Some((_, end)) = self.last_effect_range() {
            self.fade_layers.retain(|f| {
                !(matches!(&f.kind, FadeKind::FadeTo(_)) && (f.time.end - end).abs() < 0.001)
            });
            self.fade_layers.push(FadeLayer {
                time: TimeRange::new(end - duration.as_secs_f64(), end),
                kind: FadeKind::FadeTo(FadeTarget::Gradient(grad)),
                easing: Easing::Linear,
            });
        }
        self
    }

    /// Hold static text (useful for pauses between effects).
    pub fn hold(mut self, color: crate::color::Color, duration: Duration) -> Self {
        self.push_effect(
            Box::new(move |text, _frame| {
                use colored::Colorize;
                text.split('\n').map(|line| {
                    line.chars().map(|ch| {
                        ch.to_string().truecolor(color.r, color.g, color.b).to_string()
                    }).collect::<String>()
                }).collect::<Vec<_>>().join("\n")
            }),
            duration,
            30,
        );
        self
    }

    /// Place an effect at an explicit time range on the timeline.
    ///
    /// ```no_run
    /// # async fn example() {
    /// use std::time::Duration;
    /// use chromakopia::animate::{TimeRange, Sequence, rainbow_effect};
    ///
    /// Sequence::new("Hello!")
    ///     .effect(TimeRange::from_duration(Duration::ZERO, Duration::from_secs(5)), 15, rainbow_effect())
    ///     .run(1.0).await;
    /// # }
    /// ```
    pub fn effect<F>(mut self, time: TimeRange, delay_ms: u64, effect: F) -> Self
    where
        F: Fn(&str, usize) -> String + Send + 'static,
    {
        if time.end > self.cursor {
            self.cursor = time.end;
        }
        self.effect_layers.push(EffectLayer {
            time,
            effect: Box::new(effect),
            delay_ms,
        });
        self
    }

    /// Place a fade at an explicit time range with a specific easing curve.
    ///
    /// ```no_run
    /// # async fn example() {
    /// use std::time::Duration;
    /// use chromakopia::animate::{TimeRange, FadeKind, FadeTarget, Easing, Sequence};
    ///
    /// Sequence::new("Hello!")
    ///     .glow(chromakopia::presets::mist(), Duration::from_secs(5))
    ///     .fade(
    ///         TimeRange::from_duration(Duration::ZERO, Duration::from_secs(1)),
    ///         FadeKind::FadeFrom(FadeTarget::Background),
    ///         Easing::EaseOut,
    ///     )
    ///     .run(1.0).await;
    /// # }
    /// ```
    pub fn fade(mut self, time: TimeRange, kind: FadeKind, easing: Easing) -> Self {
        self.fade_layers.push(FadeLayer {
            time,
            kind,
            easing,
        });
        self
    }

    /// Set the easing curve on the last fade layer.
    ///
    /// ```no_run
    /// # async fn example() {
    /// use std::time::Duration;
    /// use chromakopia::animate::{Easing, Sequence};
    ///
    /// Sequence::new("Hello!")
    ///     .glow(chromakopia::presets::mist(), Duration::from_secs(5))
    ///     .with_fade(Duration::from_secs(1), Duration::ZERO)
    ///     .eased(Easing::EaseOut)
    ///     .fade_to_gradient(chromakopia::presets::dark_n_stormy(), Duration::from_secs(2))
    ///     .eased(Easing::EaseInOut)
    ///     .run(1.0).await;
    /// # }
    /// ```
    pub fn eased(mut self, easing: Easing) -> Self {
        if let Some(fade) = self.fade_layers.last_mut() {
            fade.easing = easing;
        }
        self
    }

    /// Run the sequence. Completes when all steps are done.
    pub async fn run(self, speed: f64) {
        let text = self.text;

        // Eagerly probe terminal colors before any output
        crate::terminal::probe_colors();

        // Hide cursor
        {
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\x1B[?25l");
            let _ = stderr.flush();
        }

        let total_duration = self.effect_layers.iter()
            .map(|l| l.time.end)
            .chain(self.fade_layers.iter().map(|l| l.time.end))
            .fold(0.0f64, f64::max);

        let mut lines_printed: usize = 0;
        let mut t = 0.0;

        while t < total_duration {
            // Find active effect layer (last one containing t)
            let active = self.effect_layers.iter()
                .rev()
                .find(|l| l.time.contains(t));

            let Some(active) = active else {
                // No active effect at this time — skip forward
                t += 0.030;
                continue;
            };

            let delay_ms = active.delay_ms;
            let frame = ((t - active.time.start) * 1000.0 / delay_ms as f64) as usize;
            let rendered = (active.effect)(&text, frame);

            // Apply fades — FadeFrom takes priority over FadeTo (matches original behavior)
            let active_from = self.fade_layers.iter()
                .find(|f| f.time.contains(t) && matches!(f.kind, FadeKind::FadeFrom(_)));
            let active_to = self.fade_layers.iter()
                .find(|f| f.time.contains(t) && matches!(f.kind, FadeKind::FadeTo(_)));

            let final_rendered = if let Some(fade) = active_from {
                let raw_t = fade.time.progress(t);
                let eased_t = fade.easing.apply(raw_t);
                apply_fade(&rendered, eased_t, &fade.kind, &text)
            } else if let Some(fade) = active_to {
                let raw_t = fade.time.progress(t);
                let eased_t = fade.easing.apply(raw_t);
                apply_fade(&rendered, eased_t, &fade.kind, &text)
            } else {
                rendered
            };

            let mut buf = String::new();
            lines_printed = render_frame(&mut buf, &final_rendered, lines_printed);
            {
                let mut stderr = std::io::stderr().lock();
                let _ = write!(stderr, "{}", buf);
                let _ = stderr.flush();
            }

            let delay = Duration::from_millis((delay_ms as f64 / speed) as u64);
            tokio::time::sleep(delay).await;
            t += delay_ms as f64 / 1000.0;
        }

        // Check if the last fade settles text on screen
        let settled = self.fade_layers.iter()
            .filter(|f| (f.time.end - total_duration).abs() < 0.001)
            .any(|f| matches!(&f.kind, FadeKind::FadeTo(target) if !matches!(target, FadeTarget::Background)));

        if settled {
            // Move to the line after the text and show cursor
            let mut buf = String::new();
            buf.push_str("\n\x1B[?25h");
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "{}", buf);
            let _ = stderr.flush();
        } else {
            // Clear the animation area and show cursor
            let mut buf = String::new();
            if lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", lines_printed));
            }
            buf.push_str("\r\x1B[J\x1B[?25h");
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "{}", buf);
            let _ = stderr.flush();
        }
    }
}

/// Lerp all truecolor RGB values in an ANSI string toward a target color
/// by `(1 - opacity)`. At opacity 1.0 the colors are unchanged;
/// at 0.0 they match the target exactly.
fn apply_fade_toward(s: &str, opacity: f64, target: crate::color::Color) -> String {
    let bg = target;
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;

            let seq_start = i;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                let seq = &s[seq_start..i];
                if let Some(rgb) = seq.strip_prefix("38;2;") {
                    let parts: Vec<&str> = rgb.split(';').collect();
                    if parts.len() == 3 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[0].parse::<u8>(),
                            parts[1].parse::<u8>(),
                            parts[2].parse::<u8>(),
                        ) {
                            let c = crate::color::Color::lerp_rgb(
                                bg,
                                crate::color::Color::new(r, g, b),
                                opacity,
                            );
                            result.push_str(&format!("\x1B[38;2;{};{};{}m", c.r, c.g, c.b));
                            i += 1;
                            continue;
                        }
                    }
                }
                result.push_str(&s[start..=i]);
                i += 1;
            }
        } else {
            // Correctly handle multi-byte UTF-8 characters
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 }
                else if byte < 0xE0 { 2 }
                else if byte < 0xF0 { 3 }
                else { 4 };
            let end = (i + char_len).min(bytes.len());
            result.push_str(&s[i..end]);
            i = end;
        }
    }

    result
}

/// Like `apply_fade_toward` but each character fades toward its
/// corresponding color in a gradient palette.
fn apply_fade_toward_gradient(s: &str, opacity: f64, grad: &Gradient, text: &str) -> String {
    // Build palette for every non-newline character. Effects like glow
    // color ALL characters (including spaces), so the palette must cover
    // every character that gets an ANSI color sequence.
    let char_count = text.chars().filter(|c| *c != '\n').count().max(2);
    let palette = grad.palette(char_count);

    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut color_idx: usize = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;

            let seq_start = i;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                let seq = &s[seq_start..i];
                if let Some(rgb) = seq.strip_prefix("38;2;") {
                    let parts: Vec<&str> = rgb.split(';').collect();
                    if parts.len() == 3 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[0].parse::<u8>(),
                            parts[1].parse::<u8>(),
                            parts[2].parse::<u8>(),
                        ) {
                            let target = palette[color_idx.min(palette.len() - 1)];
                            color_idx += 1;
                            let c = crate::color::Color::lerp_rgb(
                                target,
                                crate::color::Color::new(r, g, b),
                                opacity,
                            );
                            result.push_str(&format!("\x1B[38;2;{};{};{}m", c.r, c.g, c.b));
                            i += 1;
                            continue;
                        }
                    }
                }
                result.push_str(&s[start..=i]);
                i += 1;
            }
        } else {
            // Correctly handle multi-byte UTF-8 characters
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 }
                else if byte < 0xE0 { 2 }
                else if byte < 0xF0 { 3 }
                else { 4 };
            let end = (i + char_len).min(bytes.len());
            result.push_str(&s[i..end]);
            i = end;
        }
    }

    result
}

/// Apply a fade layer to rendered text.
fn apply_fade(rendered: &str, progress: f64, kind: &FadeKind, text: &str) -> String {
    match kind {
        FadeKind::FadeFrom(target) => {
            // progress 0→1: from target to effect colors
            let opacity = progress;
            match target {
                FadeTarget::Background => apply_fade_toward(rendered, opacity, crate::terminal::bg_color()),
                FadeTarget::Foreground => apply_fade_toward(rendered, opacity, crate::terminal::fg_color()),
                FadeTarget::Color(c) => apply_fade_toward(rendered, opacity, *c),
                FadeTarget::Gradient(g) => apply_fade_toward_gradient(rendered, opacity, g, text),
            }
        }
        FadeKind::FadeTo(target) => {
            // progress 0→1: from effect colors to target
            let opacity = 1.0 - progress;
            match target {
                FadeTarget::Background => apply_fade_toward(rendered, opacity, crate::terminal::bg_color()),
                FadeTarget::Foreground => apply_fade_toward(rendered, opacity, crate::terminal::fg_color()),
                FadeTarget::Color(c) => apply_fade_toward(rendered, opacity, *c),
                FadeTarget::Gradient(g) => apply_fade_toward_gradient(rendered, opacity, g, text),
            }
        }
    }
}

// ── Effect factory functions (for power-user `.effect()` API) ──

/// Create a rainbow effect closure for use with [`Sequence::effect`].
pub fn rainbow_effect() -> impl Fn(&str, usize) -> String + Send + 'static {
    |text, frame| effects::rainbow(text, frame)
}

/// Create a glow effect closure for use with [`Sequence::effect`].
pub fn glow_effect(grad: Gradient) -> impl Fn(&str, usize) -> String + Send + 'static {
    use crate::color::Color;
    let palette = grad.palette(3);
    let bright = palette[0];
    let mid = palette[palette.len() / 2];
    let dark = palette[palette.len() - 1];

    move |text, frame| {
        use colored::Colorize;
        let lines: Vec<&str> = text.split('\n').collect();
        let max_col = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        if max_col == 0 { return String::new(); }

        let glow_width = (max_col as f64 * 0.3).max(4.0);
        let period = max_col as f64 + glow_width * 2.0;
        let center = (frame as f64 * 0.4) % period - glow_width;

        lines.iter().map(|line| {
            line.chars().enumerate().map(|(col, ch)| {
                let dist = (col as f64 - center).abs();
                let t = (dist / glow_width).min(1.0);
                let t = t * t * (3.0 - 2.0 * t);
                let c = if t < 0.5 {
                    Color::lerp_rgb(bright, mid, t * 2.0)
                } else {
                    Color::lerp_rgb(mid, dark, (t - 0.5) * 2.0)
                };
                ch.to_string().truecolor(c.r, c.g, c.b).to_string()
            }).collect::<String>()
        }).collect::<Vec<_>>().join("\n")
    }
}

/// Create a cycle effect closure for use with [`Sequence::effect`].
pub fn cycle_effect(grad: Gradient) -> impl Fn(&str, usize) -> String + Send + 'static {
    move |text, frame| {
        use colored::Colorize;
        let len = text.chars().filter(|c| !c.is_whitespace()).count().max(2);
        let palette = grad.palette(len * 2);
        let offset = frame % palette.len();
        let mut result = String::new();
        let mut color_idx = 0;
        for ch in text.chars() {
            if ch.is_whitespace() {
                result.push(ch);
            } else {
                let c = palette[(color_idx + offset) % palette.len()];
                result.push_str(&ch.to_string().truecolor(c.r, c.g, c.b).to_string());
                color_idx += 1;
            }
        }
        result
    }
}

/// Create a flap (split-flap board) effect closure for use with [`Sequence::effect`].
pub fn flap_effect(settled: crate::color::Color, flipping: crate::color::Color) -> impl Fn(&str, usize) -> String + Send + 'static {
    move |text, frame| effects::flap(text, frame, settled, flipping)
}

/// Create a rainbow plasma effect closure for use with [`Sequence::effect`].
pub fn plasma_effect() -> impl Fn(&str, usize) -> String + Send + 'static {
    |text, frame| effects::plasma(text, frame, None)
}

/// Create a plasma effect closure with a custom gradient for use with [`Sequence::effect`].
pub fn plasma_gradient_effect(grad: Gradient) -> impl Fn(&str, usize) -> String + Send + 'static {
    let palette = grad.palette(256);
    move |text, frame| effects::plasma(text, frame, Some(&palette))
}

/// Create a bounce slide-in effect closure for use with [`Sequence::effect`].
///
/// `total_frames` controls how long the slide-in takes; after that the text holds.
pub fn scroll_effect(direction: effects::ScrollDirection, total_frames: usize) -> impl Fn(&str, usize) -> String + Send + 'static {
    move |text, frame| effects::scroll(text, frame, total_frames, direction)
}

/// Create a bounce slide-in effect closure with a custom gradient for use with [`Sequence::effect`].
pub fn scroll_gradient_effect(direction: effects::ScrollDirection, grad: Gradient, total_frames: usize) -> impl Fn(&str, usize) -> String + Send + 'static {
    move |text, frame| effects::scroll_with(text, frame, total_frames, direction, &grad)
}

// ── Standalone animations ──

/// Start a rainbow animation. Speed is a multiplier (1.0 = default).
pub fn rainbow(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::rainbow, 15, speed)
}

/// Start a pulse animation (red highlight expanding from center).
pub fn pulse(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::pulse, 16, speed)
}

/// Start a glitch animation (random character corruption).
pub fn glitch(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::glitch, 55, speed)
}

/// Start a radar animation (spotlight sweep).
pub fn radar(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::radar, 50, speed)
}

/// Start a neon animation (flickering bright/dim).
pub fn neon(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::neon, 500, speed)
}

/// Start a karaoke animation (progressive highlight).
pub fn karaoke(text: &str, speed: f64) -> Animation {
    spawn_animation(text, effects::karaoke, 50, speed)
}

/// Start a plasma animation (demoscene-style flowing color field).
///
/// Overlapping sine waves create a 2D plasma pattern that flows
/// through the text. Works best with multiline ASCII art.
///
/// ```no_run
/// let anim = chromakopia::animate::plasma("Hello!", 1.0);
/// anim.stop();
/// ```
pub fn plasma(text: &str, speed: f64) -> Animation {
    spawn_animation(text, |text, frame| effects::plasma(text, frame, None), 30, speed)
}

/// Start a plasma animation with a custom gradient.
pub fn plasma_with(grad: Gradient, text: &str, speed: f64) -> Animation {
    let palette = grad.palette(256);
    spawn_animation(text, move |text, frame| effects::plasma(text, frame, Some(&palette)), 30, speed)
}

/// Start a slide-in animation with bounce easing and rainbow colors.
pub fn scroll(direction: effects::ScrollDirection, text: &str, duration: Duration, speed: f64) -> Animation {
    let fps = 30u64;
    let total_frames = (duration.as_secs_f64() * fps as f64) as usize;
    spawn_animation(text, move |text, frame| effects::scroll(text, frame, total_frames, direction), fps, speed)
}

/// Start a slide-in animation with bounce easing and a custom gradient.
pub fn scroll_with(direction: effects::ScrollDirection, grad: Gradient, text: &str, duration: Duration, speed: f64) -> Animation {
    let fps = 30u64;
    let total_frames = (duration.as_secs_f64() * fps as f64) as usize;
    spawn_animation(text, move |text, frame| effects::scroll_with(text, frame, total_frames, direction, &grad), fps, speed)
}

/// Slow glow that sweeps left to right across any gradient.
///
/// A bright spot travels through the gradient's colors with a smooth
/// falloff — like embers catching oxygen, or light through mist.
///
/// ```no_run
/// # async fn example() {
/// let anim = chromakopia::animate::glow(chromakopia::presets::dark_n_stormy(), "Loading...", 1.0);
/// anim.stop();
/// # }
/// ```
pub fn glow(grad: Gradient, text: &str, speed: f64) -> Animation {
    spawn_animation(
        text,
        move |text, frame| {
            use colored::Colorize;
            use crate::color::Color;

            let palette = grad.palette(3);
            let bright = palette[0];
            let mid = palette[palette.len() / 2];
            let dark = palette[palette.len() - 1];

            let lines: Vec<&str> = text.split('\n').collect();
            let max_col = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            if max_col == 0 {
                return String::new();
            }

            let glow_width = (max_col as f64 * 0.3).max(4.0);
            let period = max_col as f64 + glow_width * 2.0;
            let center = (frame as f64 * 0.4) % period - glow_width;

            lines.iter().map(|line| {
                line.chars().enumerate().map(|(col, ch)| {
                    let dist = (col as f64 - center).abs();
                    let t = (dist / glow_width).min(1.0);
                    let t = t * t * (3.0 - 2.0 * t);

                    let c = if t < 0.5 {
                        Color::lerp_rgb(bright, mid, t * 2.0)
                    } else {
                        Color::lerp_rgb(mid, dark, (t - 0.5) * 2.0)
                    };

                    ch.to_string().truecolor(c.r, c.g, c.b).to_string()
                }).collect::<String>()
            }).collect::<Vec<_>>().join("\n")
        },
        30,
        speed,
    )
}

/// Split-flap departure board animation.
///
/// Characters flip through random letters before settling into place,
/// left to right, like an airport Solari board. Uses `flughafen` colors
/// by default (amber on dark gold).
///
/// ```no_run
/// # async fn example() {
/// let anim = chromakopia::animate::flap("DEPARTURES", 1.0);
/// anim.stop();
/// # }
/// ```
pub fn flap(text: &str, speed: f64) -> Animation {
    use crate::color::Color;
    let settled = Color::new(0xff, 0xcc, 0x00);  // bright amber
    let flipping = Color::new(0x99, 0x7a, 0x00);  // dark gold
    spawn_animation(
        text,
        move |text, frame| effects::flap(text, frame, settled, flipping),
        60,
        speed,
    )
}

/// Split-flap animation with custom colors from any gradient.
///
/// First color in the gradient is used for settled characters,
/// last color for flipping characters.
pub fn flap_with(grad: Gradient, text: &str, speed: f64) -> Animation {
    let palette = grad.palette(2);
    let settled = palette[0];
    let flipping = palette[1];
    spawn_animation(
        text,
        move |text, frame| effects::flap(text, frame, settled, flipping),
        60,
        speed,
    )
}

/// Animate any gradient by scrolling its colors across the text.
///
/// ```no_run
/// # async fn example() {
/// let anim = chromakopia::animate::cycle(chromakopia::presets::morning(), "Loading...", 1.0);
/// anim.stop();
/// # }
/// ```
pub fn cycle(grad: Gradient, text: &str, speed: f64) -> Animation {
    spawn_animation(
        text,
        move |text, frame| {
            use colored::Colorize;

            let len = text.chars().filter(|c| !c.is_whitespace()).count().max(2);
            let palette = grad.palette(len * 2);
            let offset = frame % palette.len();

            let mut result = String::new();
            let mut color_idx = 0;
            for ch in text.chars() {
                if ch.is_whitespace() {
                    result.push(ch);
                } else {
                    let c = palette[(color_idx + offset) % palette.len()];
                    result.push_str(&ch.to_string().truecolor(c.r, c.g, c.b).to_string());
                    color_idx += 1;
                }
            }
            result
        },
        15,
        speed,
    )
}
