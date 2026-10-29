//! Declarative scene builder for framebuffer animations.

use crate::color::Color;
use super::effects::BlendMode;
use super::framebuffer::{Cell, Effect, FrameBuffer};
#[cfg(not(target_arch = "wasm32"))]
use super::framebuffer::AnimationHandle;

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
    /// An overlay with a "halo" — after compositing, cells within
    /// `halo_radius` of the overlay's non-space content get their existing
    /// colour lerp'd toward black by `halo_dim` (0..1). Reads as a soft
    /// shadow around the overlay's glyphs.
    OverlayHalo {
        effect: Box<dyn Effect>,
        height: usize,
        y_offset: i32,
        halo_radius: i32,
        halo_dim: f64,
    },
    /// An overlay composited with a blend mode and opacity.
    OverlayBlend {
        effect: Box<dyn Effect>,
        height: usize,
        y_offset: i32,
        mode: BlendMode,
        opacity: f64,
    },
}

impl SceneItem {
    fn height(&self) -> usize {
        match self {
            SceneItem::Effect { height, .. } => *height,
            SceneItem::Blank => 1,
            SceneItem::Overlay { .. } | SceneItem::OverlayHalo { .. } | SceneItem::OverlayBlend { .. } => 0,
        }
    }

    fn width(&self) -> usize {
        match self {
            SceneItem::Effect { width, .. } => *width,
            SceneItem::Blank => 0,
            SceneItem::Overlay { .. } | SceneItem::OverlayHalo { .. } | SceneItem::OverlayBlend { .. } => 0,
        }
    }
}

/// A scene: composable effects stacked vertically. Pure layout — knows
/// nothing about terminals or rendering targets.
pub struct Scene {
    items: Vec<SceneItem>,
    quit_key: Option<char>,
    fullscreen: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self { items: Vec::new(), quit_key: None, fullscreen: false }
    }

    /// Run on the terminal's alternate screen (like `vim` or `less`): the
    /// scene draws from the top of a blank screen, and when it ends —
    /// quit key, Ctrl-C or time up — the terminal is left exactly as it
    /// was. For full-screen demos; inline animations leave this off.
    pub fn fullscreen(mut self) -> Self {
        self.fullscreen = true;
        self
    }

    /// Stop the animation early when `key` is pressed (case-insensitive).
    pub fn quit_key(mut self, key: char) -> Self {
        self.quit_key = Some(key);
        self
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

    /// Like `overlay`, but also dims the existing buffer cells within
    /// `halo_radius` of the overlay's non-space content. `halo_dim` is the
    /// lerp factor toward black (0 = no change, 1 = fully black). Reads
    /// as a soft shadow around the overlay's glyphs that lets the layer
    /// behind partially show through.
    pub fn overlay_halo(
        mut self,
        effect: impl Effect,
        height: usize,
        y_offset: i32,
        halo_radius: i32,
        halo_dim: f64,
    ) -> Self {
        self.items.push(SceneItem::OverlayHalo {
            effect: Box::new(effect),
            height,
            y_offset,
            halo_radius,
            halo_dim,
        });
        self
    }

    /// Like `overlay`, but composited with `mode` at `opacity` (0..=1).
    /// Works for any content — glyphs, half-block pixels, background
    /// fills — over any content; empty cells stay transparent.
    pub fn overlay_blend(
        mut self,
        effect: impl Effect,
        height: usize,
        y_offset: i32,
        mode: BlendMode,
        opacity: f64,
    ) -> Self {
        self.items.push(SceneItem::OverlayBlend {
            effect: Box::new(effect),
            height,
            y_offset,
            mode,
            opacity: opacity.clamp(0.0, 1.0),
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn(self) -> AnimationHandle {
        let width = crate::terminal::terminal_width();
        let height = self.height();
        let quit_key = self.quit_key;
        super::framebuffer::spawn_effect(self, width.max(1), height.max(1), 1.0, quit_key)
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
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn run(self, seconds: f64) {
        use std::time::Duration;
        let width = crate::terminal::terminal_width().max(1);
        let height = self.height().max(1);
        let (quit_key, fullscreen) = (self.quit_key, self.fullscreen);
        super::framebuffer::run_effect_in(self, width, height, Duration::from_secs_f64(seconds), 1.0, quit_key, fullscreen).await;
    }
}

impl Effect for Scene {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        buf.clear();
        let mut y = 0;
        // (base_y, height, effect, halo_radius, halo_dim, blend) —
        // halo_radius=0 means no halo; blend=None is a regular overlay.
        let mut overlays: Vec<(usize, usize, &dyn Effect, i32, f64, Option<(BlendMode, f64)>)> = Vec::new();

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
                        overlays.push((base as usize, *height, effect.as_ref(), 0, 0.0, None));
                    }
                }
                SceneItem::OverlayHalo { effect, height, y_offset, halo_radius, halo_dim } => {
                    let base = (y as i32) + y_offset;
                    if base >= 0 {
                        overlays.push((base as usize, *height, effect.as_ref(), *halo_radius, *halo_dim, None));
                    }
                }
                SceneItem::OverlayBlend { effect, height, y_offset, mode, opacity } => {
                    let base = (y as i32) + y_offset;
                    if base >= 0 {
                        overlays.push((base as usize, *height, effect.as_ref(), 0, 0.0, Some((*mode, *opacity))));
                    }
                }
            }
        }

        // Second pass: render overlays, compositing non-space cells onto
        // whatever's already in `buf`. When the overlay cell has no
        // explicit bg, inherit the destination cell's foreground colour
        // (if it's a non-space glyph — i.e., the layer behind has a real
        // shape there). This is proper compositing — a tree `▲` over a
        // brown ground cell picks up brown as its bg so the triangle reads
        // on the ground instead of leaving a default-bg "halo."
        for (base_y, height, effect, halo_radius, halo_dim, blend) in overlays {
            let sub_width = buf.width;
            let mut sub = FrameBuffer::new(sub_width, height);
            effect.render(&mut sub, frame);

            // Halo pass (before compositing): dim main-buffer cells around
            // every non-space cell in the sub-buffer. The dimmed cells keep
            // their original char but lerp colour toward black.
            if halo_radius > 0 && halo_dim > 0.0 {
                let mut content: Vec<(i32, i32)> = Vec::new();
                for sy in 0..height {
                    for sx in 0..sub_width.min(buf.width) {
                        if sub.get(sx, sy).ch != ' ' {
                            content.push((sx as i32, (base_y + sy) as i32));
                        }
                    }
                }
                use std::collections::HashSet;
                let content_set: HashSet<(i32, i32)> = content.iter().copied().collect();
                let mut dimmed: HashSet<(i32, i32)> = HashSet::new();
                let black = crate::color::Color::new(0, 0, 0);
                for (cx, cy) in &content {
                    for dy in -halo_radius..=halo_radius {
                        for dx in -halo_radius..=halo_radius {
                            if dx == 0 && dy == 0 { continue; }
                            let hx = cx + dx;
                            let hy = cy + dy;
                            if hx < 0 || hy < 0
                                || hx >= buf.width as i32 || hy >= buf.height as i32 { continue; }
                            if content_set.contains(&(hx, hy)) { continue; }
                            if !dimmed.insert((hx, hy)) { continue; }
                            let existing = buf.get(hx as usize, hy as usize);
                            if existing.ch == ' ' && existing.bg.is_none() { continue; }
                            let new_fg = crate::color::Color::lerp_rgb(existing.color, black, halo_dim);
                            let new_bg = existing.bg.map(|b| crate::color::Color::lerp_rgb(b, black, halo_dim));
                            let new_cell = Cell { ch: existing.ch, color: new_fg, bg: new_bg };
                            buf.set(hx as usize, hy as usize, new_cell);
                        }
                    }
                }
            }

            for sy in 0..height {
                let target_y = base_y + sy;
                if target_y >= buf.height { continue; }
                for sx in 0..sub_width.min(buf.width) {
                    let cell = sub.get(sx, sy);
                    if let Some((mode, opacity)) = blend {
                        let under = buf.get(sx, target_y);
                        buf.set(sx, target_y, mode.composite(under, cell, opacity));
                        continue;
                    }
                    if cell.ch == ' ' { continue; }
                    let merged = if cell.bg.is_none() {
                        // Inherit bg only if the destination opted in by
                        // setting its own bg explicitly. Effects that want
                        // to act as "solid background layers" (mountains,
                        // ground) should set Cell.bg = their fg. Sparse
                        // effects (stars, particles) leave bg=None so
                        // overlays on top of them don't pick up their
                        // bright fg as a halo.
                        let dest = buf.get(sx, target_y);
                        Cell { ch: cell.ch, color: cell.color, bg: dest.bg }
                    } else {
                        cell
                    };
                    buf.set(sx, target_y, merged);
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
    fn overlay_blend_composites_half_blocks() {
        struct Fill(Cell);
        impl Effect for Fill {
            fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
                for x in 0..buf.width { buf.set(x, 0, self.0); }
            }
            fn size(&self) -> (usize, usize) { (2, 1) }
        }
        let red = Color::new(200, 0, 0);
        let blue = Color::new(0, 0, 200);
        let scene = Scene::new()
            .add(Fill(Cell::with_bg('▀', red, blue)))
            .overlay_blend(Fill(Cell::new('▄', red)), 1, -1, BlendMode::Screen, 1.0);
        let mut buf = FrameBuffer::new(2, 1);
        scene.render(&mut buf, 0);
        let cell = buf.get(0, 0);
        assert_eq!((cell.ch, cell.color, cell.bg), ('▀', red, Some(BlendMode::Screen.blend(blue, red))));
    }

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
    fn quit_key_builder_stores_key() {
        let scene = Scene::new().quit_key('q');
        assert_eq!(scene.quit_key, Some('q'));
        assert_eq!(Scene::new().quit_key, None);
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
