//! Declarative scene builder for framebuffer animations.
//!
//! Compose static text and animated effects at arbitrary positions.
//!
//! ```ignore
//! Scene::new()
//!     .line(Line::new()
//!         .text("Hello, ", Color::new(255, 255, 255))
//!         .animated("world", Rainbow::new("world"))
//!         .text("!", Color::new(255, 255, 255))
//!     )
//!     .line(Line::blank())
//!     .line(Line::static_text("footer", Color::new(100, 100, 100)))
//!     .run(Duration::from_secs(5)).await;
//! ```

use crate::color::Color;
use super::framebuffer::{Cell, Effect, FrameBuffer};
use std::time::Duration;

/// A segment within a line — either static text or an animated region.
enum Segment {
    Static(Vec<char>, Color),
    Animated {
        chars: Vec<char>,
        effect: Box<dyn Effect>,
    },
}

/// A single line in the scene, composed of segments.
pub struct Line {
    segments: Vec<Segment>,
}

impl Line {
    pub fn new() -> Self {
        Self { segments: Vec::new() }
    }

    /// A fully blank spacer line.
    pub fn blank() -> Self {
        Self { segments: Vec::new() }
    }

    /// A line of static text in one color.
    pub fn static_text(text: &str, color: Color) -> Self {
        let mut line = Self::new();
        line.segments.push(Segment::Static(text.chars().collect(), color));
        line
    }

    /// Append static text to this line.
    pub fn text(mut self, text: &str, color: Color) -> Self {
        self.segments.push(Segment::Static(text.chars().collect(), color));
        self
    }

    /// Append an animated region to this line.
    ///
    /// The effect receives a sub-buffer sized to this segment's width × 1 row.
    /// The scene composites it at the correct x offset.
    pub fn animated(mut self, text: &str, effect: impl Effect) -> Self {
        self.segments.push(Segment::Animated {
            chars: text.chars().collect(),
            effect: Box::new(effect),
        });
        self
    }

    /// Append a full line of animated text.
    pub fn full(text: &str, effect: impl Effect) -> Self {
        Self::new().animated(text, effect)
    }

    fn width(&self) -> usize {
        self.segments.iter().map(|s| match s {
            Segment::Static(chars, _) => chars.len(),
            Segment::Animated { chars, .. } => chars.len(),
        }).sum()
    }
}

/// A scene: multiple lines, each with static and animated segments.
pub struct Scene {
    lines: Vec<Line>,
}

impl Scene {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Add a line to the scene.
    pub fn line(mut self, line: Line) -> Self {
        self.lines.push(line);
        self
    }

    /// Add multiple lines from multiline text, all with the same effect factory.
    pub fn text_block(mut self, text: &str, make_line: impl Fn(&str) -> Line) -> Self {
        for line_text in text.lines() {
            self.lines.push(make_line(line_text));
        }
        self
    }

    pub fn width(&self) -> usize {
        self.lines.iter().map(|l| l.width()).max().unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    /// Run the scene with the framebuffer renderer.
    ///
    /// Width is clamped to terminal width so scroll/elastic have room to overshoot.
    pub async fn run(self, duration: Duration) {
        let term_width = crate::terminal::terminal_width();
        let width = self.width().max(term_width);
        let height = self.height();
        if width == 0 || height == 0 { return; }
        let effect = SceneEffect { scene: self };
        super::framebuffer::run_effect(effect, width, height, duration, 1.0).await;
    }
}

/// Internal: wraps a Scene as an Effect for the framebuffer renderer.
struct SceneEffect {
    scene: Scene,
}

impl Effect for SceneEffect {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        for (y, line) in self.scene.lines.iter().enumerate() {
            if y >= buf.height { break; }

            let mut x_offset = 0;

            for segment in &line.segments {
                match segment {
                    Segment::Static(chars, color) => {
                        if frame == 0 {
                            for (i, &ch) in chars.iter().enumerate() {
                                let x = x_offset + i;
                                if x < buf.width {
                                    buf.set(x, y, Cell::new(ch, *color));
                                }
                            }
                        }
                        x_offset += chars.len();
                    }
                    Segment::Animated { chars, effect } => {
                        let seg_width = chars.len();
                        // Sub-buffer gets remaining width so scroll/elastic can overshoot
                        let sub_width = buf.width.saturating_sub(x_offset).max(seg_width);
                        let mut sub = FrameBuffer::new(sub_width, 1);
                        // Initialize with the segment's chars
                        for (i, &ch) in chars.iter().enumerate() {
                            if i < sub_width {
                                sub.set(i, 0, Cell::new(ch, Color::new(204, 204, 204)));
                            }
                        }
                        // Let the effect write to the sub-buffer
                        effect.render(&mut sub, frame);
                        // Copy sub-buffer into main buffer at the right offset
                        for i in 0..sub_width {
                            let x = x_offset + i;
                            if x < buf.width {
                                buf.set(x, y, sub.get(i, 0));
                            }
                        }
                        x_offset += seg_width;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_dimensions() {
        let scene = Scene::new()
            .line(Line::static_text("hello", Color::new(255, 255, 255)))
            .line(Line::blank())
            .line(Line::static_text("world", Color::new(255, 255, 255)));
        assert_eq!(scene.width(), 5);
        assert_eq!(scene.height(), 3);
    }

    #[test]
    fn scene_text_block() {
        let scene = Scene::new()
            .text_block("ab\ncd", |l| Line::static_text(l, Color::new(255, 255, 255)));
        assert_eq!(scene.height(), 2);
        assert_eq!(scene.width(), 2);
    }

    #[test]
    fn scene_mixed_segments_width() {
        let scene = Scene::new()
            .line(Line::new()
                .text("hi ", Color::new(255, 255, 255))
                .text("world", Color::new(255, 0, 0))
            );
        assert_eq!(scene.width(), 8); // "hi " + "world"
    }

    #[test]
    fn scene_renders_static() {
        let scene = Scene::new()
            .line(Line::static_text("ab", Color::new(255, 0, 0)));
        let effect = SceneEffect { scene };
        let mut buf = FrameBuffer::new(2, 1);
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 0).color, Color::new(255, 0, 0));
        assert_eq!(buf.get(1, 0).ch, 'b');
    }

    #[test]
    fn scene_static_only_writes_frame_zero() {
        let scene = Scene::new()
            .line(Line::static_text("ab", Color::new(255, 0, 0)));
        let effect = SceneEffect { scene };
        let mut buf = FrameBuffer::new(2, 1);
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        // Overwrite manually
        buf.set(0, 0, Cell::new('x', Color::new(0, 0, 0)));
        // Frame 1 should NOT overwrite — static only writes on frame 0
        effect.render(&mut buf, 1);
        assert_eq!(buf.get(0, 0).ch, 'x');
    }

    #[test]
    fn scene_animated_segment() {
        use super::super::effects::Rainbow;

        let scene = Scene::new()
            .line(Line::new()
                .text("hi ", Color::new(255, 255, 255))
                .animated("world", Rainbow::new("world"))
            );
        let effect = SceneEffect { scene };
        let mut buf = FrameBuffer::new(8, 1);
        effect.render(&mut buf, 0);

        // Static part
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(1, 0).ch, 'i');
        assert_eq!(buf.get(2, 0).ch, ' ');
        // Animated part — chars preserved, colors from rainbow
        assert_eq!(buf.get(3, 0).ch, 'w');
        assert_eq!(buf.get(7, 0).ch, 'd');

        // Frame 10 should change animated colors
        let c0 = buf.get(3, 0).color;
        effect.render(&mut buf, 10);
        let c1 = buf.get(3, 0).color;
        assert_ne!(c0, c1);
        // Static part unchanged
        assert_eq!(buf.get(0, 0).ch, 'h');
    }
}
