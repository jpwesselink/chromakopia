//! Universal compositing: one cell (or colour) over another under a
//! `BlendMode`, at any opacity.
//!
//! Block cells — space, `▀`, `▄`, `█` — are split into their two pixels
//! (top/bottom) and blended per pixel, so half-block art stacks exactly.
//! Any other glyph keeps its shape: when it is the over cell its ink is
//! blended against what's beneath; when it is the under cell, a uniform
//! plane over it tints both its ink and its background.
//!
//! `None` colours are transparent (the terminal's own background): over
//! nothing, the over colour is drawn as-is.

use super::effects::BlendMode;
use super::framebuffer::Cell;
use crate::color::Color;

type Pixels = [Option<Color>; 2];

impl BlendMode {
    /// `over` blended onto `under`, faded in by `opacity` (0..=1).
    pub fn mix(self, under: Color, over: Color, opacity: f64) -> Color {
        let blended = self.blend(under, over);
        if opacity >= 1.0 {
            blended
        } else {
            Color::lerp_rgb(under, blended, opacity.max(0.0))
        }
    }

    /// `over` composited onto `under`. Works for every kind of cell:
    /// half-block pixels, solid planes, background-only cells and glyphs.
    pub fn composite(self, under: Cell, over: Cell, opacity: f64) -> Cell {
        let mix = |u: Option<Color>, o: Option<Color>| match (u, o) {
            (u, None) => u,
            (None, o) => o,
            (Some(u), Some(o)) => Some(self.mix(u, o, opacity)),
        };
        match (pixels(&under), pixels(&over)) {
            // Fully transparent over cell.
            (_, Some([None, None])) => under,
            // Pixels on pixels: exact per-pixel blend.
            (Some(u), Some(o)) => encode([mix(u[0], o[0]), mix(u[1], o[1])]),
            // Uniform plane over a glyph: keep the glyph, tint ink and bg.
            (None, Some([Some(t), Some(b)])) if t == b => Cell {
                ch: under.ch,
                color: self.mix(under.color, t, opacity),
                bg: mix(under.bg, Some(t)),
            },
            // Uneven pixels over a glyph: the glyph can't be split, so the
            // pixels blend against what it reads as from afar.
            (None, Some(o)) => {
                let back = backdrop(&under);
                encode([mix(back, o[0]), mix(back, o[1])])
            }
            // A glyph over anything: blend its ink and bg onto the backdrop.
            (_, None) => {
                let back = backdrop(&under);
                Cell {
                    ch: over.ch,
                    color: mix(back, Some(over.color)).unwrap_or(over.color),
                    bg: match over.bg {
                        Some(b) => mix(back, Some(b)),
                        None => back,
                    },
                }
            }
        }
    }
}

/// Split a block cell into (top, bottom) pixels. `None` for other glyphs.
fn pixels(cell: &Cell) -> Option<Pixels> {
    let fg = Some(cell.color);
    match cell.ch {
        ' ' => Some([cell.bg, cell.bg]),
        '█' => Some([fg, fg]),
        '▀' => Some([fg, cell.bg]),
        '▄' => Some([cell.bg, fg]),
        _ => None,
    }
}

/// Pack two pixels back into a cell. Solid cells use `█` with bg = fg,
/// the convention Scene overlays read as "opaque layer here".
fn encode(px: Pixels) -> Cell {
    match px {
        [None, None] => Cell::space(),
        [Some(t), Some(b)] if t == b => Cell::with_bg('█', t, t),
        [Some(t), Some(b)] => Cell::with_bg('▀', t, b),
        [Some(t), None] => Cell::new('▀', t),
        [None, Some(b)] => Cell::new('▄', b),
    }
}

/// The single colour a cell reads as: its pixels averaged, or for a
/// glyph its background (falling back to its ink).
fn backdrop(cell: &Cell) -> Option<Color> {
    match pixels(cell) {
        Some([Some(t), Some(b)]) => Some(Color::lerp_rgb(t, b, 0.5)),
        Some([t, b]) => t.or(b),
        None => Some(cell.bg.unwrap_or(cell.color)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color { r: 200, g: 0, b: 0 };
    const BLUE: Color = Color { r: 0, g: 0, b: 200 };
    const GREY: Color = Color { r: 100, g: 100, b: 100 };

    fn half(top: Color, bottom: Color) -> Cell {
        Cell::with_bg('▀', top, bottom)
    }

    #[test]
    fn transparent_over_leaves_under() {
        let under = half(RED, BLUE);
        assert_eq!(BlendMode::Multiply.composite(under, Cell::space(), 1.0), under);
    }

    #[test]
    fn half_blocks_blend_per_pixel() {
        // Over covers only the bottom pixel: top stays, bottom blends.
        let out = BlendMode::Average.composite(half(RED, BLUE), Cell::new('▄', GREY), 1.0);
        assert_eq!(out, half(RED, BlendMode::Average.blend(BLUE, GREY)));
    }

    #[test]
    fn opaque_normal_plane_hides_glyph_translucent_shows_it() {
        let text = Cell::new('A', RED);
        let plane = Cell::with_bg(' ', BLUE, BLUE);
        let hidden = BlendMode::Normal.composite(text, plane, 1.0);
        assert_eq!((hidden.ch, hidden.color, hidden.bg), ('A', BLUE, Some(BLUE)));
        let ghost = BlendMode::Normal.composite(text, plane, 0.5);
        assert_eq!(ghost.color, Color::lerp_rgb(RED, BLUE, 0.5));
    }

    #[test]
    fn glyph_over_block_blends_ink_against_backdrop() {
        let out = BlendMode::Screen.composite(Cell::with_bg('█', GREY, GREY), Cell::new('▲', RED), 1.0);
        assert_eq!(out.ch, '▲');
        assert_eq!(out.color, BlendMode::Screen.blend(GREY, RED));
        assert_eq!(out.bg, Some(GREY));
    }

    #[test]
    fn over_nothing_draws_as_is() {
        let out = BlendMode::Multiply.composite(Cell::space(), Cell::new('▀', RED), 1.0);
        assert_eq!(out, Cell::new('▀', RED));
    }
}
