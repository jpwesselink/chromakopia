use crate::color::Color;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// A progress bar with configurable characters, colors, and template strings.
///
/// Inspired by indicatif — `.chars()`, `.template()`, `.set_message()`, spinners.
///
/// ```
/// use chromakopia::{Color, ProgressBar};
///
/// let mut bar = ProgressBar::new(100)
///     .width(30)
///     .chars("█▓░")
///     .template("{spinner} {bar} {pos}/{len} {msg}")
///     .filled_color(Color::new(0, 255, 136))
///     .empty_color(Color::new(40, 40, 40));
///
/// bar.set_position(42);
/// bar.set_message("downloading...");
/// print!("\r{}", bar.render_template());
/// ```
pub struct ProgressBar {
    len: u64,
    pos: u64,
    bar_width: usize,
    filled_char: char,
    empty_char: char,
    head_char: Option<char>,
    filled_color: Color,
    empty_color: Color,
    head_color: Option<Color>,
    message: String,
    prefix: String,
    template: String,
    spinner_chars: Vec<char>,
    tick: usize,
    #[cfg(not(target_arch = "wasm32"))]
    start: Instant,
}

impl ProgressBar {
    /// Create a progress bar with `len` total units.
    pub fn new(len: u64) -> Self {
        Self {
            len,
            pos: 0,
            bar_width: 40,
            filled_char: '█',
            empty_char: '░',
            head_char: None,
            filled_color: Color::new(0, 255, 136),
            empty_color: Color::new(60, 60, 60),
            head_color: None,
            message: String::new(),
            prefix: String::new(),
            template: "{bar} {pos}/{len}".to_string(),
            spinner_chars: vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            tick: 0,
            #[cfg(not(target_arch = "wasm32"))]
            start: Instant::now(),
        }
    }

    /// Create a spinner (no length / indeterminate).
    pub fn spinner() -> Self {
        let mut pb = Self::new(0);
        pb.template = "{spinner} {msg}".to_string();
        pb
    }

    /// Bar display width in characters.
    pub fn width(mut self, w: usize) -> Self { self.bar_width = w; self }

    /// Set fill and empty chars. 2 chars: `"█░"`. 3 chars: `"█▓░"` (filled, head, empty).
    pub fn chars(mut self, chars: &str) -> Self {
        let c: Vec<char> = chars.chars().collect();
        if c.len() >= 2 {
            self.filled_char = c[0];
            self.empty_char = c[c.len() - 1];
            if c.len() >= 3 {
                self.head_char = Some(c[1]);
            }
        }
        self
    }

    /// Template string. Placeholders: `{bar}`, `{bar:WIDTH}`, `{pos}`, `{len}`,
    /// `{percent}`, `{msg}`, `{prefix}`, `{spinner}`, `{elapsed}`, `{eta}`.
    pub fn template(mut self, t: &str) -> Self { self.template = t.to_string(); self }

    /// Spinner characters (default: braille dots).
    pub fn spinner_chars(mut self, chars: &str) -> Self {
        self.spinner_chars = chars.chars().collect();
        self
    }

    pub fn filled_color(mut self, color: Color) -> Self { self.filled_color = color; self }
    pub fn empty_color(mut self, color: Color) -> Self { self.empty_color = color; self }
    pub fn head_color(mut self, color: Color) -> Self { self.head_color = Some(color); self }

    pub fn filled(mut self, ch: char, color: Color) -> Self {
        self.filled_char = ch; self.filled_color = color; self
    }

    pub fn empty(mut self, ch: char, color: Color) -> Self {
        self.empty_char = ch; self.empty_color = color; self
    }

    // ── Runtime methods ──

    pub fn set_position(&mut self, pos: u64) { self.pos = pos; self.tick += 1; }
    pub fn inc(&mut self, n: u64) { self.pos += n; self.tick += 1; }
    pub fn set_message(&mut self, msg: &str) { self.message = msg.to_string(); }
    pub fn set_prefix(&mut self, prefix: &str) { self.prefix = prefix.to_string(); }
    pub fn set_length(&mut self, len: u64) { self.len = len; }
    pub fn tick(&mut self) { self.tick += 1; }
    pub fn position(&self) -> u64 { self.pos }
    pub fn length(&self) -> u64 { self.len }

    pub fn finish(&mut self) { self.pos = self.len; }
    pub fn finish_with_message(&mut self, msg: &str) { self.pos = self.len; self.message = msg.to_string(); }

    /// Progress as 0.0–1.0.
    pub fn progress(&self) -> f64 {
        if self.len == 0 { return 0.0; }
        (self.pos as f64 / self.len as f64).clamp(0.0, 1.0)
    }

    // ── Rendering ──

    /// Plain bar text (no template, no colors) at current position. Use with effects.
    pub fn text(&self, progress: f64) -> String {
        self.bar_string(progress, self.bar_width)
    }

    /// Render the template with ANSI colors at current position.
    /// In CI (no TTY on stderr), falls back to plain text automatically.
    pub fn render_template(&self) -> String {
        if Self::is_tty() {
            self.expand_template(&self.template, true)
        } else {
            self.expand_template(&self.template, false)
        }
    }

    /// Render the template as plain text (no ANSI colors).
    pub fn render_template_plain(&self) -> String {
        self.expand_template(&self.template, false)
    }

    /// Force ANSI-colored template output regardless of TTY detection.
    pub fn render_template_ansi(&self) -> String {
        self.expand_template(&self.template, true)
    }

    /// Check if color output is enabled (respects NO_COLOR, FORCE_COLOR, TTY).
    fn is_tty() -> bool {
        crate::color_enabled()
    }

    /// Colored bar at a given progress (0.0–1.0). No template.
    /// Auto-detects TTY — plain text in CI.
    pub fn render(&self, progress: f64) -> String {
        if Self::is_tty() {
            self.render_bar(progress, self.bar_width)
        } else {
            self.bar_string(progress, self.bar_width)
        }
    }

    /// Force ANSI-colored bar regardless of TTY.
    pub fn render_ansi(&self, progress: f64) -> String {
        self.render_bar(progress, self.bar_width)
    }

    // ── Internal ──

    fn bar_string(&self, progress: f64, width: usize) -> String {
        let p = progress.clamp(0.0, 1.0);
        let filled_count = (p * width as f64).round() as usize;
        let empty_count = width.saturating_sub(filled_count);

        let mut s = String::with_capacity(width);
        if self.head_char.is_some() && filled_count > 0 && empty_count > 0 {
            for _ in 0..filled_count - 1 { s.push(self.filled_char); }
            s.push(self.head_char.unwrap());
            for _ in 0..empty_count { s.push(self.empty_char); }
        } else {
            for _ in 0..filled_count { s.push(self.filled_char); }
            for _ in 0..empty_count { s.push(self.empty_char); }
        }
        s
    }

    fn render_bar(&self, progress: f64, width: usize) -> String {
        let p = progress.clamp(0.0, 1.0);
        let filled_count = (p * width as f64).round() as usize;
        let empty_count = width.saturating_sub(filled_count);

        let fc = self.filled_color;
        let ec = self.empty_color;
        let hc = self.head_color.unwrap_or(self.filled_color);

        let mut out = String::with_capacity(width * 4);
        out.push_str(&format!("\x1B[38;2;{};{};{}m", fc.r, fc.g, fc.b));
        if self.head_char.is_some() && filled_count > 0 && empty_count > 0 {
            for _ in 0..filled_count - 1 { out.push(self.filled_char); }
            out.push_str(&format!("\x1B[38;2;{};{};{}m{}", hc.r, hc.g, hc.b, self.head_char.unwrap()));
        } else {
            for _ in 0..filled_count { out.push(self.filled_char); }
        }
        out.push_str(&format!("\x1B[38;2;{};{};{}m", ec.r, ec.g, ec.b));
        for _ in 0..empty_count { out.push(self.empty_char); }
        out.push_str("\x1B[0m");
        out
    }

    fn expand_template(&self, template: &str, ansi: bool) -> String {
        let progress = self.progress();

        #[cfg(not(target_arch = "wasm32"))]
        let (elapsed_secs, eta_secs) = {
            let elapsed = self.start.elapsed();
            let e = elapsed.as_secs();
            let eta = if progress > 0.0 {
                ((elapsed.as_secs_f64() / progress) * (1.0 - progress)) as u64
            } else { 0 };
            (e, eta)
        };
        #[cfg(target_arch = "wasm32")]
        let (elapsed_secs, eta_secs) = (0u64, 0u64);

        let spinner = if self.spinner_chars.is_empty() {
            ' '
        } else {
            self.spinner_chars[self.tick % self.spinner_chars.len()]
        };

        let mut out = template.to_string();

        // {bar:WIDTH} with optional width
        while let Some(start) = out.find("{bar") {
            let end = out[start..].find('}').map(|i| start + i + 1).unwrap_or(out.len());
            let spec = &out[start + 4..end - 1]; // everything between "{bar" and "}"
            let w = if spec.starts_with(':') {
                spec[1..].parse::<usize>().unwrap_or(self.bar_width)
            } else {
                self.bar_width
            };
            let bar = if ansi { self.render_bar(progress, w) } else { self.bar_string(progress, w) };
            out = format!("{}{}{}", &out[..start], bar, &out[end..]);
        }

        out = out.replace("{pos}", &self.pos.to_string());
        out = out.replace("{len}", &self.len.to_string());
        out = out.replace("{percent}", &format!("{:.0}", progress * 100.0));
        out = out.replace("{msg}", &self.message);
        out = out.replace("{prefix}", &self.prefix);
        out = out.replace("{spinner}", &spinner.to_string());
        out = out.replace("{elapsed}", &format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60));
        out = out.replace("{eta}", &format!("{}:{:02}", eta_secs / 60, eta_secs % 60));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_at_zero() {
        let bar = ProgressBar::new(100).width(10);
        assert_eq!(bar.text(0.0), "░░░░░░░░░░");
    }

    #[test]
    fn text_at_half() {
        let bar = ProgressBar::new(100).width(10);
        assert_eq!(bar.text(0.5), "█████░░░░░");
    }

    #[test]
    fn text_at_full() {
        let bar = ProgressBar::new(100).width(10);
        assert_eq!(bar.text(1.0), "██████████");
    }

    #[test]
    fn custom_chars() {
        let bar = ProgressBar::new(100).width(10).chars("=#");
        assert_eq!(bar.text(0.3), "===#######");
    }

    #[test]
    fn head_char() {
        let bar = ProgressBar::new(100).width(10).chars("=>-");
        assert_eq!(bar.text(0.5), "====>-----");
    }

    #[test]
    fn clamps_progress() {
        let bar = ProgressBar::new(100).width(5);
        assert_eq!(bar.text(-1.0), "░░░░░");
        assert_eq!(bar.text(2.0), "█████");
    }

    #[test]
    fn set_position_and_progress() {
        let mut bar = ProgressBar::new(100);
        bar.set_position(50);
        assert_eq!(bar.progress(), 0.5);
        bar.inc(25);
        assert_eq!(bar.progress(), 0.75);
    }

    #[test]
    fn template_basic() {
        let mut bar = ProgressBar::new(100).width(10);
        bar.set_position(30);
        let out = bar.render_template_plain();
        assert!(out.contains("30/100"));
        assert!(out.contains("███"));
    }

    #[test]
    fn template_bar_width() {
        let mut bar = ProgressBar::new(10)
            .width(20)
            .template("{bar:10} {percent}%");
        bar.set_position(5);
        let out = bar.render_template_plain();
        assert!(out.contains("50%"));
        assert_eq!(out.split(' ').next().unwrap().chars().count(), 10);
    }

    #[test]
    fn template_message_and_prefix() {
        let mut bar = ProgressBar::new(10)
            .template("{prefix} {bar} {msg}");
        bar.set_prefix("dl");
        bar.set_message("crate.io");
        bar.set_position(5);
        let out = bar.render_template_plain();
        assert!(out.starts_with("dl "));
        assert!(out.ends_with("crate.io"));
    }

    #[test]
    fn spinner() {
        let mut bar = ProgressBar::spinner()
            .template("{spinner} {msg}");
        bar.set_message("loading");
        bar.tick();
        let out = bar.render_template_plain();
        assert!(out.contains("loading"));
        assert_eq!(out.chars().next().unwrap(), '⠙'); // tick=1
    }

    #[test]
    fn finish_with_message() {
        let mut bar = ProgressBar::new(100);
        bar.finish_with_message("done!");
        assert_eq!(bar.progress(), 1.0);
        assert_eq!(bar.message, "done!");
    }
}
