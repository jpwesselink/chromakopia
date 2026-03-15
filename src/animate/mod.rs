//! Animated terminal gradients — rainbow, pulse, glitch, radar, neon, karaoke.
//!
//! ```no_run
//! # async fn example() {
//! let anim = shimmer::animate::rainbow("Loading...", 1.0);
//! // ... do async work ...
//! anim.stop();
//! # }
//! ```

mod effects;

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
}

impl Drop for Animation {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

fn spawn_animation<F>(text: &str, effect: F, delay_ms: u64, speed: f64) -> Animation
where
    F: Fn(&str, usize) -> String + Send + 'static,
{
    let running = Arc::new(AtomicBool::new(true));
    let text = Arc::new(Mutex::new(text.to_string()));
    let frame = Arc::new(AtomicUsize::new(0));

    let r = running.clone();
    let t = text.clone();
    let f = frame.clone();
    let delay = Duration::from_millis((delay_ms as f64 / speed) as u64);

    let handle = tokio::spawn(async move {
        let mut lines_printed: usize = 0;

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
            let rendered = rendered.trim_end_matches('\n');
            let rendered_lines: Vec<&str> = rendered.split('\n').collect();
            let line_count = rendered_lines.len();

            let mut buf = String::new();
            if lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", lines_printed));
            }
            for (i, line) in rendered_lines.iter().enumerate() {
                buf.push('\r');
                buf.push_str(line);
                buf.push_str("\x1B[K"); // clear remainder after content
                if i < line_count - 1 {
                    buf.push('\n');
                }
            }

            {
                let mut stderr = std::io::stderr().lock();
                let _ = write!(stderr, "{}", buf);
                let _ = stderr.flush();
            }

            lines_printed = line_count - 1; // cursor is on the last line, not below it
            tokio::time::sleep(delay).await;
        }

        // Clear the animation and show cursor
        {
            let mut buf = String::new();
            if lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", lines_printed));
            }
            for i in 0..=lines_printed {
                buf.push_str("\r\x1B[K");
                if i < lines_printed {
                    buf.push('\n');
                }
            }
            if lines_printed > 0 {
                buf.push_str(&format!("\x1B[{}F", lines_printed));
            }
            buf.push_str("\x1B[?25h"); // show cursor
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "{}", buf);
            let _ = stderr.flush();
        }
    });

    Animation {
        running,
        text,
        handle: Some(handle),
        frame,
    }
}

// ── Render helper (shared by spawn_animation and Sequence) ──

fn render_frame(buf: &mut String, rendered: &str, lines_printed: usize) -> usize {
    let rendered = rendered.trim_end_matches('\n');
    let rendered_lines: Vec<&str> = rendered.split('\n').collect();
    let line_count = rendered_lines.len();

    if lines_printed > 0 {
        buf.push_str(&format!("\x1B[{}F", lines_printed));
    }
    for (i, line) in rendered_lines.iter().enumerate() {
        buf.push('\r');
        buf.push_str(line);
        buf.push_str("\x1B[K");
        if i < line_count - 1 {
            buf.push('\n');
        }
    }
    line_count - 1
}

// ── Sequence builder ──

type BoxEffect = Box<dyn Fn(&str, usize) -> String + Send + 'static>;

enum FadeTarget {
    /// Fade toward terminal background (text disappears)
    Background,
    /// Fade toward terminal foreground (gradient calms down to solid text)
    Foreground,
    /// Fade toward a specific color
    Color(crate::color::Color),
    /// Fade toward a static gradient
    Gradient(Gradient),
}

struct Step {
    effect: BoxEffect,
    duration: Duration,
    delay_ms: u64,
    fade_in: Duration,
    fade_out: Duration,
    fade_out_target: FadeTarget,
}

/// Chain multiple animation effects into a sequence.
///
/// ```no_run
/// # async fn example() {
/// use std::time::Duration;
/// shimmer::animate::Sequence::new("Hello, world!")
///     .fade_in(Duration::from_secs(1))
///     .glow(shimmer::presets::dark_n_stormy(), Duration::from_secs(3))
///     .fade_out(Duration::from_secs(1))
///     .run(1.0).await;
/// # }
/// ```
pub struct Sequence {
    text: String,
    steps: Vec<Step>,
}

impl Sequence {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            steps: Vec::new(),
        }
    }

    /// Fade from black to white.
    pub fn fade_in(self, duration: Duration) -> Self {
        self.fade_in_color(crate::color::Color::new(255, 255, 255), duration)
    }

    /// Fade from black to a specific color.
    pub fn fade_in_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        let total = (duration.as_millis() / 30) as usize;
        self.steps.push(Step {
            effect: Box::new(move |text, frame| effects::fade_in(text, frame, total, color)),
            duration,
            delay_ms: 30,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Fade from white to black.
    pub fn fade_out(self, duration: Duration) -> Self {
        self.fade_out_color(crate::color::Color::new(255, 255, 255), duration)
    }

    /// Fade from a specific color to black.
    pub fn fade_out_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        let total = (duration.as_millis() / 30) as usize;
        self.steps.push(Step {
            effect: Box::new(move |text, frame| effects::fade_out(text, frame, total, color)),
            duration,
            delay_ms: 30,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Glow sweep with a gradient.
    pub fn glow(mut self, grad: Gradient, duration: Duration) -> Self {
        use crate::color::Color;
        let palette = grad.palette(3);
        let bright = palette[0];
        let mid = palette[palette.len() / 2];
        let dark = palette[palette.len() - 1];

        self.steps.push(Step {
            effect: Box::new(move |text, frame| {
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
            delay_ms: 30,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Rainbow effect.
    pub fn rainbow(mut self, duration: Duration) -> Self {
        self.steps.push(Step {
            effect: Box::new(|text, frame| effects::rainbow(text, frame)),
            duration,
            delay_ms: 15,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Split-flap departure board with flughafen colors.
    pub fn flap(mut self, duration: Duration) -> Self {
        use crate::color::Color;
        let settled = Color::new(0xff, 0xcc, 0x00);
        let flipping = Color::new(0x99, 0x7a, 0x00);
        self.steps.push(Step {
            effect: Box::new(move |text, frame| effects::flap(text, frame, settled, flipping)),
            duration,
            delay_ms: 60,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Split-flap with custom gradient colors.
    pub fn flap_with(mut self, grad: Gradient, duration: Duration) -> Self {
        let palette = grad.palette(2);
        let settled = palette[0];
        let flipping = palette[1];
        self.steps.push(Step {
            effect: Box::new(move |text, frame| effects::flap(text, frame, settled, flipping)),
            duration,
            delay_ms: 60,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Cycle a gradient's colors.
    pub fn cycle(mut self, grad: Gradient, duration: Duration) -> Self {
        self.steps.push(Step {
            effect: Box::new(move |text, frame| {
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
            delay_ms: 15,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Set fade-in and fade-out durations on the last added step.
    ///
    /// Fades to/from the terminal background color (text appears/disappears).
    pub fn with_fade(mut self, fade_in: Duration, fade_out: Duration) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.fade_in = fade_in;
            step.fade_out = fade_out;
            step.fade_out_target = FadeTarget::Background;
        }
        self
    }

    /// Fade the gradient into the terminal's foreground color.
    /// The text stays on screen as solid, readable text.
    pub fn fade_to_foreground(mut self, duration: Duration) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.fade_out = duration;
            step.fade_out_target = FadeTarget::Foreground;
        }
        self
    }

    /// Fade the gradient into a specific color.
    /// The text stays on screen in that color.
    pub fn fade_to_color(mut self, color: crate::color::Color, duration: Duration) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.fade_out = duration;
            step.fade_out_target = FadeTarget::Color(color);
        }
        self
    }

    /// Fade the animation into a static gradient.
    /// The text stays on screen with the gradient applied.
    pub fn fade_to_gradient(mut self, grad: Gradient, duration: Duration) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.fade_out = duration;
            step.fade_out_target = FadeTarget::Gradient(grad);
        }
        self
    }

    /// Hold static text (useful for pauses between effects).
    pub fn hold(mut self, color: crate::color::Color, duration: Duration) -> Self {
        self.steps.push(Step {
            effect: Box::new(move |text, _frame| {
                use colored::Colorize;
                text.split('\n').map(|line| {
                    line.chars().map(|ch| {
                        ch.to_string().truecolor(color.r, color.g, color.b).to_string()
                    }).collect::<String>()
                }).collect::<Vec<_>>().join("\n")
            }),
            duration,
            delay_ms: 30,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
            fade_out_target: FadeTarget::Background,
        });
        self
    }

    /// Run the sequence. Completes when all steps are done.
    pub async fn run(self, speed: f64) {
        let text = self.text;

        // Hide cursor
        {
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\x1B[?25l");
            let _ = stderr.flush();
        }

        let mut lines_printed: usize = 0;

        for step in &self.steps {
            let delay = Duration::from_millis((step.delay_ms as f64 / speed) as u64);
            let total_frames = (step.duration.as_millis() as f64 / step.delay_ms as f64) as usize;
            let fade_in_frames = (step.fade_in.as_millis() as f64 / step.delay_ms as f64) as usize;
            let fade_out_frames = (step.fade_out.as_millis() as f64 / step.delay_ms as f64) as usize;

            for frame in 0..total_frames {
                let rendered = (step.effect)(&text, frame);

                // Apply opacity envelope
                let fade_in_opacity = if frame < fade_in_frames {
                    Some(frame as f64 / fade_in_frames.max(1) as f64)
                } else {
                    None
                };
                let fade_out_opacity = if fade_out_frames > 0 && frame >= total_frames - fade_out_frames {
                    Some((total_frames - frame) as f64 / fade_out_frames.max(1) as f64)
                } else {
                    None
                };

                let final_rendered = match (fade_in_opacity, fade_out_opacity) {
                    (Some(t), _) => {
                        // Fade-in always goes from bg → full color
                        apply_fade_toward(&rendered, t, crate::terminal::bg_color())
                    }
                    (_, Some(t)) => {
                        // Fade-out target depends on step config
                        match &step.fade_out_target {
                            FadeTarget::Gradient(grad) => {
                                apply_fade_toward_gradient(&rendered, t, grad, &text)
                            }
                            other => {
                                let target = match other {
                                    FadeTarget::Background => crate::terminal::bg_color(),
                                    FadeTarget::Foreground => crate::terminal::fg_color(),
                                    FadeTarget::Color(c) => *c,
                                    FadeTarget::Gradient(_) => unreachable!(),
                                };
                                apply_fade_toward(&rendered, t, target)
                            }
                        }
                    }
                    _ => rendered,
                };

                let mut buf = String::new();
                lines_printed = render_frame(&mut buf, &final_rendered, lines_printed);
                {
                    let mut stderr = std::io::stderr().lock();
                    let _ = write!(stderr, "{}", buf);
                    let _ = stderr.flush();
                }
                tokio::time::sleep(delay).await;
            }
        }

        // Check if the last step settled (text should stay on screen)
        let settled = self.steps.last().map_or(false, |s| {
            s.fade_out > Duration::ZERO
                && matches!(
                    s.fade_out_target,
                    FadeTarget::Foreground | FadeTarget::Color(_) | FadeTarget::Gradient(_)
                )
        });

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
                if seq.starts_with("38;2;") {
                    let parts: Vec<&str> = seq[5..].split(';').collect();
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
            result.push(bytes[i] as char);
            i += 1;
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
                if seq.starts_with("38;2;") {
                    let parts: Vec<&str> = seq[5..].split(';').collect();
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
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

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

/// Slow glow that sweeps left to right across any gradient.
///
/// A bright spot travels through the gradient's colors with a smooth
/// falloff — like embers catching oxygen, or light through mist.
///
/// ```no_run
/// # async fn example() {
/// let anim = shimmer::animate::glow(shimmer::presets::dark_n_stormy(), "Loading...", 1.0);
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
/// let anim = shimmer::animate::flap("DEPARTURES", 1.0);
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
/// let anim = shimmer::animate::cycle(shimmer::presets::morning(), "Loading...", 1.0);
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
