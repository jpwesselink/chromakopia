//! Declarative scene builder for framebuffer animations.

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

    pub fn blank() -> Self {
        Self { segments: Vec::new() }
    }

    pub fn static_text(text: &str, color: Color) -> Self {
        Self::new().text(text, color)
    }

    pub fn text(mut self, text: &str, color: Color) -> Self {
        self.segments.push(Segment::Static(text.chars().collect(), color));
        self
    }

    pub fn animated(mut self, text: &str, effect: impl Effect) -> Self {
        self.segments.push(Segment::Animated {
            chars: text.chars().collect(),
            effect: Box::new(effect),
        });
        self
    }

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

/// An item in the scene — either a single line or a multi-row block.
enum SceneItem {
    Line(Line),
    /// A multi-row block: the effect gets one sub-buffer for all rows.
    Block {
        text: String,
        height: usize,
        width: usize,
        effect: Box<dyn Effect>,
    },
}

impl SceneItem {
    fn height(&self) -> usize {
        match self {
            SceneItem::Line(_) => 1,
            SceneItem::Block { height, .. } => *height,
        }
    }

    fn width(&self) -> usize {
        match self {
            SceneItem::Line(line) => line.width(),
            SceneItem::Block { width, .. } => *width,
        }
    }
}

/// A scene: lines and blocks composed into a full-screen animation.
pub struct Scene {
    items: Vec<SceneItem>,
}

impl Scene {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add a single line.
    pub fn line(mut self, line: Line) -> Self {
        self.items.push(SceneItem::Line(line));
        self
    }

    /// Add a multi-row block with one effect that sees all rows at once.
    ///
    /// The effect gets a sub-buffer sized to the block's full width × height,
    /// so plasma/glow/etc compute a coherent field across all rows.
    pub fn block(mut self, text: &str, effect: impl Effect) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let height = lines.len();
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        self.items.push(SceneItem::Block {
            text: text.to_string(),
            height,
            width,
            effect: Box::new(effect),
        });
        self
    }

    /// Add multiple lines, each with its own effect (created by the factory).
    pub fn text_block(mut self, text: &str, make_line: impl Fn(&str) -> Line) -> Self {
        for line_text in text.lines() {
            self.items.push(SceneItem::Line(make_line(line_text)));
        }
        self
    }

    pub fn width(&self) -> usize {
        self.items.iter().map(|item| item.width()).max().unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.items.iter().map(|item| item.height()).sum()
    }

    pub async fn run(self, duration: Duration) {
        let term_width = crate::terminal::terminal_width();
        let width = self.width().max(term_width);
        let height = self.height();
        if width == 0 || height == 0 { return; }
        let effect = SceneEffect { scene: self };
        super::framebuffer::run_effect(effect, width, height, duration, 1.0).await;
    }
}

struct SceneEffect {
    scene: Scene,
}

impl Effect for SceneEffect {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut y = 0;

        for item in &self.scene.items {
            match item {
                SceneItem::Line(line) => {
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
                                let sub_width = buf.width.saturating_sub(x_offset).max(seg_width);
                                let mut sub = FrameBuffer::new(sub_width, 1);
                                for (i, &ch) in chars.iter().enumerate() {
                                    if i < sub_width {
                                        sub.set(i, 0, Cell::new(ch, Color::new(204, 204, 204)));
                                    }
                                }
                                effect.render(&mut sub, frame);
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
                    y += 1;
                }
                SceneItem::Block { text, height, width: _, effect } => {
                    let block_height = *height;
                    if y + block_height > buf.height { break; }

                    // Create a multi-row sub-buffer for the whole block
                    let sub_width = buf.width;
                    let mut sub = FrameBuffer::from_text(text, Color::new(204, 204, 204));
                    // Resize to match buf width
                    if sub.width < sub_width {
                        let mut resized = FrameBuffer::new(sub_width, block_height);
                        for sy in 0..block_height {
                            for sx in 0..sub.width.min(sub_width) {
                                resized.set(sx, sy, sub.get(sx, sy));
                            }
                        }
                        sub = resized;
                    }

                    effect.render(&mut sub, frame);

                    // Copy into main buffer
                    for sy in 0..block_height {
                        for sx in 0..sub_width.min(buf.width) {
                            if y + sy < buf.height {
                                buf.set(sx, y + sy, sub.get(sx, sy));
                            }
                        }
                    }
                    y += block_height;
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
    fn scene_block_dimensions() {
        let scene = Scene::new()
            .line(Line::static_text("top", Color::new(255, 255, 255)))
            .block("ab\ncd\nef", super::super::effects::Rainbow::new("ab\ncd\nef"));
        assert_eq!(scene.height(), 4); // 1 line + 3-row block
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
    }

    #[test]
    fn scene_block_renders_multirow() {
        let scene = Scene::new()
            .block("ab\ncd", super::super::effects::Rainbow::new("ab\ncd"));
        let effect = SceneEffect { scene };
        let mut buf = FrameBuffer::new(2, 2);
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 1).ch, 'c');
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
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(3, 0).ch, 'w');
        assert_eq!(buf.get(7, 0).ch, 'd');
    }
}
