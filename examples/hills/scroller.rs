//! The scroller along the bottom: the scroll text riding a sine wave,
//! letter by letter in whichever font and style the script says.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::animate::Cell;
use chromakopia::prelude::*;
use std::collections::HashMap;
use std::f64::consts::TAU;

use super::glyphs::{c64, has_c64, has_mario, mario, shade, stamp, LETTER_HIGHLIGHT, LETTER_UPPER};
use super::looping::{self, angle_index, track, ANGLES};
#[cfg(test)]
use super::glyphs::LETTER_LOWER;
use super::script::Glyph;
use super::tiny;

// Scroller: riding a gentle sine through screen x — classic cracktro
// bottom scroller. Its speed lives in the script's timeline.
const SCROLL_WAVE: f64 = 4.0;
const SCROLL_WAVE_PERIOD: f64 = 0.8;
/// Every font's baseline sits on `y0 + BASE` (plus the wave).
const BASE: i64 = 7;

/// A scroller font, switched with `{font …}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    /// The C64 character ROM, 8×8.
    C64,
    /// C64 at double size, 16×16.
    Big,
    /// Tom Thumb, 3×5 — tiny.
    Tiny,
    /// The wordmark's chunky capitals at 2× — only CHROMAKOPIA's letters.
    Mario,
    /// The terminal's own font: one letter per cell.
    Term,
}

impl Font {
    pub fn named(name: &str) -> Option<Font> {
        Some(match name {
            "c64" => Font::C64,
            "big" => Font::Big,
            "tiny" => Font::Tiny,
            "mario" => Font::Mario,
            "term" => Font::Term,
            _ => return None,
        })
    }
}

/// How letters are drawn, switched with `{style …}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    /// Pink-to-lavender face, dark outline, violet extrusion.
    Candy,
    /// One flat cream colour — no gradient, outline or extrusion. Cream
    /// reads on a bare dark terminal as well as on the landscape.
    Flat,
}

impl Style {
    pub fn named(name: &str) -> Option<Style> {
        Some(match name {
            "candy" => Style::Candy,
            "flat" => Style::Flat,
            _ => return None,
        })
    }
}

/// Whether `font` can draw `ch` (a space always works).
pub fn has(font: Font, ch: char) -> bool {
    ch == ' '
        || match font {
            Font::C64 | Font::Big => has_c64(ch),
            Font::Tiny => tiny::has(ch),
            Font::Mario => has_mario(ch),
            Font::Term => !ch.is_control(),
        }
}

/// How far `ch` moves the pen in `font`, in pixels (= columns).
pub fn advance(font: Font, ch: char) -> i64 {
    match font {
        Font::C64 => 8,
        Font::Big | Font::Mario => 16,
        Font::Tiny => tiny::advance(ch),
        Font::Term => 1,
    }
}

/// Pixels from a glyph's top to its baseline — where capitals end and
/// descenders begin.
fn baseline(font: Font) -> i64 {
    match font {
        Font::C64 => 7,
        Font::Big => 14,
        Font::Mario => 16,
        Font::Tiny => 5,
        Font::Term => 2,
    }
}

/// Whole pixels the text has moved, `scrolled` (fractional) pixels in.
/// Rounded, not floored: scroll positions land on whole pixels give or
/// take float noise (245.9999…), and flooring that drops a step — the text
/// holds a frame, then jumps two.
pub fn pen(scrolled: f64) -> i64 {
    scrolled.round() as i64
}

/// A letter turned to each of the `ANGLES`, as `looping::turned` faces.
type Turns = Vec<Vec<(i64, i64, Color)>>;

/// The placed scroll text, scrolling right to left along the bottom.
pub struct Scroller {
    glyphs: Vec<Glyph>,
    y0: i64,
    /// Every looped letter at every angle, worked out once.
    turns: HashMap<(char, Font), Turns>,
}

/// The pixel font `font` draws `ch` from, and at what scale — None for the
/// fonts that can't loop.
fn bitmap(font: Font, ch: char) -> Option<([u8; 8], i64)> {
    match font {
        Font::C64 => Some((c64(ch), 1)),
        Font::Big => Some((c64(ch), 2)),
        Font::Mario => Some((mario(ch), 2)),
        Font::Tiny | Font::Term => None,
    }
}

impl Scroller {
    pub fn new(glyphs: Vec<Glyph>, y0: i64) -> Self {
        let mut turns = HashMap::new();
        for g in glyphs.iter().filter(|g| g.looped) {
            let Some((rows, scale)) = bitmap(g.font, g.ch) else { continue };
            turns.entry((g.ch, g.font)).or_insert_with(|| (0..ANGLES).map(|i| looping::turned(&rows, scale, i)).collect());
        }
        Self { glyphs, y0, turns }
    }

    /// Draw looped letter `g` wherever it is on the track, `scrolled`
    /// pixels in. False when it isn't one (or its font can't loop).
    fn paint_looped(&self, c: &mut Canvas, g: &Glyph, scrolled: f64, scratch: &mut Vec<(i64, i64, Color)>) -> bool {
        let Some(turns) = self.turns.get(&(g.ch, g.font)) else { return false };
        let adv = advance(g.font, g.ch);
        let d = (pen(scrolled) - g.x as i64) as f64 - adv as f64 / 2.0;
        let spot = track(d, c.width, (self.y0 + BASE) as f64);
        if spot.x < -(adv as f64) - 4.0 || spot.x > c.width as f64 + adv as f64 {
            return true;
        }
        // The letter's middle sits `rise` above the track, along its own up.
        let rise = (baseline(g.font) - adv / 2) as f64;
        let (sin, cos) = spot.angle.sin_cos();
        let (cx, cy) = ((spot.x + rise * sin).round() as i64, (spot.y - rise * cos).round() as i64);
        scratch.clear();
        scratch.extend(turns[angle_index(spot.angle)].iter().map(|&(x, y, color)| (cx + x, cy + y, color)));
        match g.style {
            Style::Candy => shade(c, scratch, if g.font == Font::C64 { 1 } else { 2 }),
            Style::Flat => {
                for &(x, y, _) in scratch.iter() {
                    c.set(x, y, LETTER_HIGHLIGHT);
                }
            }
        }
        true
    }

    /// Screen x and wave-shifted bottom line of `g`, `scrolled` pixels in,
    /// or None when it's off screen.
    fn spot(&self, g: &Glyph, width: usize, scrolled: f64, t: f64) -> Option<(i64, i64)> {
        let x = width as i64 - pen(scrolled) + g.x as i64;
        if x < -(advance(g.font, g.ch) + 4) || x > width as i64 {
            return None;
        }
        let wave = if g.wave { (TAU * t / SCROLL_WAVE_PERIOD + x as f64 * 0.05).sin() } else { 0.0 };
        Some((x, self.y0 + BASE + (SCROLL_WAVE * wave).round() as i64))
    }

    /// Paint the pixel-font letters `scrolled` pixels in (0 = just off the
    /// right edge). Terminal-font letters go on in `paint_cells`.
    pub fn paint(&self, c: &mut Canvas, scrolled: f64) {
        let t = c.frame as f64 / FPS;
        let mut face = Vec::new();
        for g in self.glyphs.iter().filter(|g| g.font != Font::Term) {
            if g.looped && self.paint_looped(c, g, scrolled, &mut face) {
                continue;
            }
            let Some((x, base)) = self.spot(g, c.width, scrolled, t) else { continue };
            let top = base - baseline(g.font);
            match (g.font, g.style) {
                (Font::C64, Style::Candy) => stamp(c, &c64(g.ch), (x, top), 1, 1, &mut face),
                (Font::Big, Style::Candy) => stamp(c, &c64(g.ch), (x, top), 2, 2, &mut face),
                (Font::Mario, Style::Candy) => stamp(c, &mario(g.ch), (x, top), 2, 2, &mut face),
                (Font::Tiny, Style::Candy) => tiny::stamp_char(c, g.ch, (x, top)),
                (Font::C64, Style::Flat) => flat(c, &c64(g.ch), (x, top), 1),
                (Font::Big, Style::Flat) => flat(c, &c64(g.ch), (x, top), 2),
                (Font::Mario, Style::Flat) => flat(c, &mario(g.ch), (x, top), 2),
                (Font::Tiny, Style::Flat) => flat(c, &tiny::rows(g.ch), (x, top), 1),
                (Font::Term, _) => {}
            }
        }
    }

    /// Put the terminal-font letters on as real characters over whatever
    /// is beneath them, riding the wave in whole rows (or one straight row
    /// with `{wave off}`). Run after the pixel layers have rendered.
    pub fn paint_cells(&self, buf: &mut FrameBuffer, scrolled: f64, frame: usize) {
        let t = frame as f64 / FPS;
        for g in self.glyphs.iter().filter(|g| g.font == Font::Term) {
            let Some((x, base)) = self.spot(g, buf.width, scrolled, t) else { continue };
            let row = (base - 4).div_euclid(2);
            if x < 0 || x as usize >= buf.width || row < 0 || row as usize >= buf.height {
                continue;
            }
            let color = match g.style {
                Style::Candy => LETTER_UPPER,
                Style::Flat => LETTER_HIGHLIGHT,
            };
            let (x, row) = (x as usize, row as usize);
            buf.set(x, row, BlendMode::Normal.composite(buf.get(x, row), Cell::new(g.ch, color), 1.0));
        }
    }
}

/// A glyph's lit pixels in flat cream, `scale`× — no outline or
/// extrusion.
fn flat(c: &mut Canvas, rows: &[u8], (x, y): (i64, i64), scale: i64) {
    for (gy, row) in rows.iter().enumerate() {
        for gx in 0..8 {
            if row & (0x80 >> gx) != 0 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        c.set(x + gx * scale + dx, y + gy as i64 * scale + dy, LETTER_HIGHLIGHT);
                    }
                }
            }
        }
    }
}

/// The scroll text, cracktro style: brag about what's on screen, never
/// explain how.
pub const PITCH: &str = "{hills}{style flat}Welly well, what do we have here?{style candy}{gap}{logo}{pause 3}{gap}\
...and yes, before you ask: this is a TERMINAL. No browser, no video, no tricks you can see, just the good old terminal you have known and loved...   which you will now love a little bit more.   {font tiny}(psst... the small print: no pixels were harmed in the making of this demo - because there are no pixels, just text and ANSI escape codes!){font c64}   \
Count the layers behind me: eight bands of clouds and hills, all swinging to their own beat.   \
Clouds that drift and throw shadows on the land. Never shade. They're above that.   \
A logo that will not sit still, bouncing every letter on its own little wave...   in fact,{logo away} it won't even stay.   \
{loop on}And all of it at a silky 60 frames per second.{loop off}   {logo}\
How is it done?   Ha!   A magician never tells...   {font term}(this is your terminal's own font, by the way - blink and you miss it){font c64}   \
...but a crate does:   {font big}cargo add chromakopia{font c64}   {style flat}{wave off}...ofc you should type it without the colours, just like we did in 1986...{wave on}{style candy}   \
Greetings fly out to Exel, {} and Hardbeat - still sorry, Apache: no Microsoft BackOffice was harmed - Paradox - Pitr / Eindbazen - ISO - Danny Styles - Spidey - Humasect - Nightdead - Si-Goat - Stephen Parker - Koffiekop - Kevinjel - Sanity - \
Quaddepos - Janglad - KVE - Aftertwelve and the rest of DMKC - Matteo Collina - Luca Maraschi - Heapwolf - \
Ashley Williams (you were right about Rust) - Steve Klabnik - \
and to everyone who still believes a terminal can be beautiful, \
to all the coders, pixel pushers and scrollers of the past, and to YOU for reading this far!   {font mario}CHROMAKOPIA{font c64}   \
Keys: space or arrows to skip, q to quit.   {bye}Text wraps... now!          ";

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ROWS, SCROLLER_Y};
    use std::f64::consts::PI;

    #[test]
    fn pitch_places_on_any_screen() {
        let script = super::super::script::parse(PITCH).expect("pitch parses");
        for w in [80, 112, 200] {
            script.place(w).expect("every letter in its font");
        }
        assert!(script.text.contains("cargo add chromakopia"));
    }

    fn one(font: Font, style: Style, ch: char) -> Scroller {
        Scroller::new(vec![Glyph { ch, x: 0.0, font, style, wave: true, looped: false }], 40)
    }

    /// Ink rows (pixel) and colours of `s` painted with its glyph at x = 10.
    fn ink(s: Scroller) -> (Vec<i64>, Vec<Color>) {
        let layers = Layers::new(40).layer(Layer::paint(move |c| s.paint(c, (c.width - 10) as f64)));
        let mut buf = FrameBuffer::new(60, 40);
        layers.render(&mut buf, 0);
        let (mut rows, mut colors) = (Vec::new(), Vec::new());
        for y in 0..40 {
            for x in 0..60 {
                let cell = buf.get(x, y);
                if cell.ch == '▀' || cell.ch == '▄' {
                    rows.push(y as i64);
                    colors.push(cell.color);
                }
            }
        }
        (rows, colors)
    }

    #[test]
    fn big_is_twice_as_tall_and_shares_the_baseline() {
        let (c64_rows, _) = ink(one(Font::C64, Style::Flat, 'M'));
        let (big_rows, _) = ink(one(Font::Big, Style::Flat, 'M'));
        let span = |r: &[i64]| r.iter().max().unwrap() - r.iter().min().unwrap();
        assert!(span(&big_rows) > span(&c64_rows) + 2, "{big_rows:?} vs {c64_rows:?}");
        assert_eq!(big_rows.iter().max(), c64_rows.iter().max());
    }

    #[test]
    fn flat_has_one_colour_candy_has_its_gradient() {
        let (_, flat) = ink(one(Font::C64, Style::Flat, 'A'));
        assert!(flat.iter().all(|&c| c == LETTER_HIGHLIGHT));
        let (_, candy) = ink(one(Font::C64, Style::Candy, 'A'));
        assert!(candy.iter().any(|&c| c == LETTER_UPPER || c == LETTER_LOWER));
    }

    #[test]
    fn wave_off_rides_a_straight_line_flat_still_waves() {
        let tops = |wave: bool| -> Vec<i64> {
            (0..48)
                .map(|frame| {
                    let s = Scroller::new(vec![Glyph { ch: 'M', x: 0.0, font: Font::Big, style: Style::Flat, wave, looped: false }], 40);
                    let layers = Layers::new(40).layer(Layer::paint(move |c| s.paint(c, (c.width - 10) as f64)));
                    let mut buf = FrameBuffer::new(60, 40);
                    layers.render(&mut buf, frame);
                    (0..40).find(|&y| (0..60).any(|x| buf.get(x, y).ch != ' ')).unwrap() as i64
                })
                .collect()
        };
        let straight = tops(false);
        assert!(straight.iter().all(|&t| t == straight[0]), "{straight:?}");
        let waving = tops(true);
        assert!(waving.iter().any(|&t| t != waving[0]), "flat letters should still wave");
    }

    #[test]
    fn term_letters_wave_in_whole_rows_unless_wave_is_off() {
        let rows = |wave: bool| -> Vec<usize> {
            let s = Scroller::new(vec![Glyph { ch: 'Q', x: 0.0, font: Font::Term, style: Style::Candy, wave, looped: false }], 40);
            (0..48)
                .map(|frame| {
                    let mut buf = FrameBuffer::new(60, 40);
                    s.paint_cells(&mut buf, 50.0, frame);
                    (0..40).find(|&y| (0..60).any(|x| buf.get(x, y).ch == 'Q')).unwrap()
                })
                .collect()
        };
        let waving = rows(true);
        assert!(waving.iter().any(|&r| r != waving[0]), "{waving:?}");
        let straight = rows(false);
        assert!(straight.iter().all(|&r| r == straight[0]), "{straight:?}");
    }

    #[test]
    fn looped_letters_ride_over_the_top_upside_down() {
        let w = 200;
        let g = Glyph { ch: 'M', x: 0.0, font: Font::Big, style: Style::Flat, wave: true, looped: true };
        // On the real screen: the scroller's own line, full height.
        let base = (SCROLLER_Y + BASE) as f64;
        // Scrolled so the letter's middle is halfway round the loop.
        let scrolled = looping::loop_start(w) + PI * looping::LOOP_R + 8.0;
        let s = Scroller::new(vec![g], SCROLLER_Y);
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| s.paint(c, scrolled)));
        let mut buf = FrameBuffer::new(w, ROWS);
        layers.render(&mut buf, 0);
        let ink: Vec<usize> = (0..ROWS * 2).filter(|&y| (0..w).any(|x| {
            let cell = buf.get(x, y / 2);
            cell.ch != ' ' && if y % 2 == 0 { cell.color == LETTER_HIGHLIGHT } else { cell.bg == Some(LETTER_HIGHLIGHT) }
        })).collect();
        let top = base - 2.0 * looping::LOOP_R;
        assert!(!ink.is_empty());
        // Hanging from the top of the loop, feet up, head towards the middle.
        assert!(ink.iter().all(|&y| (y as f64) >= top - 2.0 && (y as f64) <= top + 16.0), "{ink:?} vs top {top}");
    }

    #[test]
    fn term_letters_are_real_characters() {
        let s = one(Font::Term, Style::Candy, 'Q');
        let mut buf = FrameBuffer::new(60, 40);
        s.paint_cells(&mut buf, 50.0, 0);
        let found = (0..40).any(|y| (0..60).any(|x| buf.get(x, y).ch == 'Q'));
        assert!(found);
        // ...and never as pixels.
        assert!(ink(one(Font::Term, Style::Candy, 'Q')).0.is_empty());
    }
}
