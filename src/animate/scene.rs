//! Declarative scene builder for framebuffer animations.

use crate::color::Color;
use super::framebuffer::{Cell, Effect, FrameBuffer, AnimationHandle};
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
/// Use for mixing static and animated text on one row.
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

    /// Add an animated segment. The effect should be a bare color transform
    /// (e.g. `Rainbow::new()`), NOT wrapped with `.on()` — the text is
    /// provided here by the Line, not by the effect.
    pub fn animated(mut self, text: &str, effect: impl Effect) -> Self {
        self.segments.push(Segment::Animated {
            chars: text.chars().collect(),
            effect: Box::new(effect),
        });
        self
    }

    /// Shorthand: a single animated segment spanning the full line.
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

impl Effect for Line {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut x_offset = 0;
        for segment in &self.segments {
            match segment {
                Segment::Static(chars, color) => {
                    if frame == 0 {
                        for (i, &ch) in chars.iter().enumerate() {
                            let x = x_offset + i;
                            if x < buf.width {
                                buf.set(x, 0, Cell::new(ch, *color));
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
                            sub.set(i, 0, Cell::new(ch, super::framebuffer::DEFAULT_TEXT_COLOR));
                        }
                    }
                    effect.render(&mut sub, frame);
                    for i in 0..sub_width {
                        let x = x_offset + i;
                        if x < buf.width {
                            buf.set(x, 0, sub.get(i, 0));
                        }
                    }
                    x_offset += seg_width;
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) { (self.width(), 1) }
}

/// An item in the scene.
enum SceneItem {
    /// An effect with inherent size (from .on() or layout effects).
    Effect {
        effect: Box<dyn Effect>,
        height: usize,
        width: usize,
    },
    /// A blank row.
    Blank,
    /// An overlay: rendered after all other items, compositing only non-space
    /// cells on top. Does not consume vertical layout space.
    Overlay {
        effect: Box<dyn Effect>,
        height: usize,
        y_offset: i32,
    },
}

impl SceneItem {
    fn height(&self) -> usize {
        match self {
            SceneItem::Effect { height, .. } => *height,
            SceneItem::Blank => 1,
            SceneItem::Overlay { .. } => 0,
        }
    }

    fn width(&self) -> usize {
        match self {
            SceneItem::Effect { width, .. } => *width,
            SceneItem::Blank => 0,
            SceneItem::Overlay { .. } => 0,
        }
    }
}

/// A scene: composable effects stacked vertically. Pure layout — knows
/// nothing about terminals or rendering targets.
pub struct Scene {
    items: Vec<SceneItem>,
}

impl Scene {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add an effect. Uses the effect's size() for layout.
    /// Wrap color effects with `.on("text")` to give them text and size.
    pub fn add(mut self, effect: impl Effect) -> Self {
        let (w, h) = effect.size();
        let height = h.max(1);
        self.items.push(SceneItem::Effect {
            effect: Box::new(effect),
            height,
            width: w,
        });
        self
    }

    /// Add a blank row.
    pub fn blank(mut self) -> Self {
        self.items.push(SceneItem::Blank);
        self
    }

    /// Add an overlay — rendered on top of everything else, only non-space
    /// cells are composited. Does not consume vertical layout space.
    pub fn overlay(mut self, effect: impl Effect, height: usize, y_offset: i32) -> Self {
        self.items.push(SceneItem::Overlay {
            effect: Box::new(effect),
            height,
            y_offset,
        });
        self
    }

    pub fn width(&self) -> usize {
        self.items.iter().map(|item| item.width()).max().unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.items.iter().map(|item| item.height()).sum()
    }

    /// Spawn in a terminal area. Runs until `.stop()` or `.fade_out()`.
    pub fn spawn(self) -> AnimationHandle {
        let width = crate::terminal::terminal_width();
        let height = self.height();
        super::framebuffer::spawn_effect(self, width.max(1), height.max(1), 1.0)
    }

    /// Render a single frame to an ANSI string. For inline use.
    pub fn frame(&self, frame: usize) -> String {
        let width = self.width().max(1);
        let height = self.height().max(1);
        let mut buf = FrameBuffer::new(width, height);
        self.render(&mut buf, frame);
        buf.to_ansi_string()
    }

    /// Run in a terminal area for `seconds`, then stop.
    pub async fn run(self, seconds: f64) {
        let width = crate::terminal::terminal_width().max(1);
        let height = self.height().max(1);
        super::framebuffer::run_effect(self, width, height, Duration::from_secs_f64(seconds), 1.0).await;
    }
}

impl Effect for Scene {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        buf.clear();
        let mut y = 0;
        let mut overlays: Vec<(usize, usize, &dyn Effect)> = Vec::new();

        for item in &self.items {
            match item {
                SceneItem::Effect { effect, height, .. } => {
                    let block_height = *height;
                    if y + block_height > buf.height { break; }

                    let sub_width = buf.width;
                    let mut sub = FrameBuffer::new(sub_width, block_height);
                    effect.render(&mut sub, frame);

                    for sy in 0..block_height {
                        for sx in 0..sub_width.min(buf.width) {
                            if y + sy < buf.height {
                                buf.set(sx, y + sy, sub.get(sx, sy));
                            }
                        }
                    }
                    y += block_height;
                }
                SceneItem::Blank => {
                    y += 1;
                }
                SceneItem::Overlay { effect, height, y_offset } => {
                    let base = (y as i32) + y_offset;
                    if base >= 0 {
                        overlays.push((base as usize, *height, effect.as_ref()));
                    }
                }
            }
        }

        // Second pass: render overlays, compositing only non-space cells
        for (base_y, height, effect) in overlays {
            let sub_width = buf.width;
            let mut sub = FrameBuffer::new(sub_width, height);
            effect.render(&mut sub, frame);

            for sy in 0..height {
                let target_y = base_y + sy;
                if target_y >= buf.height { continue; }
                for sx in 0..sub_width.min(buf.width) {
                    let cell = sub.get(sx, sy);
                    if cell.ch != ' ' {
                        buf.set(sx, target_y, cell);
                    }
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) { (self.width(), self.height()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::framebuffer::EffectExt;

    #[test]
    fn scene_dimensions() {
        let scene = Scene::new()
            .add(Line::static_text("hello", Color::new(255, 255, 255)))
            .blank()
            .add(Line::static_text("world", Color::new(255, 255, 255)));
        assert_eq!(scene.width(), 5);
        assert_eq!(scene.height(), 3);
    }

    #[test]
    fn scene_block_dimensions() {
        let scene = Scene::new()
            .add(Line::static_text("top", Color::new(255, 255, 255)))
            .add(super::super::effects::Rainbow::new().on("ab\ncd\nef"));
        assert_eq!(scene.height(), 4); // 1 line + 3-row block
    }

    #[test]
    fn scene_renders_static() {
        let scene = Scene::new()
            .add(Line::static_text("ab", Color::new(255, 0, 0)));
        let mut buf = FrameBuffer::new(2, 1);
        scene.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 0).color, Color::new(255, 0, 0));
    }

    #[test]
    fn scene_block_renders_multirow() {
        let scene = Scene::new()
            .add(super::super::effects::Rainbow::new().on("ab\ncd"));
        let mut buf = FrameBuffer::new(2, 2);
        scene.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 1).ch, 'c');
    }

    #[test]
    fn scene_animated_segment() {
        use super::super::effects::Rainbow;

        let scene = Scene::new()
            .add(Line::new()
                .text("hi ", Color::new(255, 255, 255))
                .animated("world", Rainbow::new())
            );
        let mut buf = FrameBuffer::new(8, 1);
        scene.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(3, 0).ch, 'w');
        assert_eq!(buf.get(7, 0).ch, 'd');
    }
}
