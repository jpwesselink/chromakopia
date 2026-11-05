//! The cold open, before the scroller starts: four lines, each a step
//! louder, each on one of the pad's swells (every 4 seconds). The beat
//! comes in on the fifth swell, at `BEAT_AT`, as "Welly well" reaches the
//! middle.
//!
//! 1. "hey there" — hard on, hard off.
//! 2. "how nice of you to come along" — fading in and out
//!    (`AlphaIn` / `AlphaOut`).
//! 3. "we've been expecting you" — in the terminal's own font, easing in
//!    from the right, holding in the middle, easing out to the left.
//! 4. "let's see if we can make this more entertaining, shall we?" — in
//!    the terminal's own font too, in the letters' colours: sliding in from
//!    the right on a DYCP wave that dies down as it arrives, lying still in
//!    the middle to be read, then sliding off to the left as the wave
//!    swells again.
//!
//! All four are a library `Timeline` of cell effects around the
//! scroller's line.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;

use chromakopia::animate::Cell;

use super::glyphs;

const LINE1: &str = "hey there";
const LINE2: &str = "how nice of you to come along";
const LINE3: &str = "we've been expecting you";
const LINE4: &str = "let's see if we can make this more entertaining, shall we?";

/// The pad swells every 4 seconds; a line to each.
const STEP1: (f64, f64) = (0.3, 3.0);
const STEP2: (f64, f64) = (4.2, 7.6);
const FADE_SECS: f64 = 0.8;
const STEP3_START: f64 = 8.2;
/// Step 3: seconds easing in, holding in the middle, easing out.
const IN_SECS: f64 = 1.2;
const HOLD_SECS: f64 = 1.2;
const OUT_SECS: f64 = 0.8;
/// Step 4, the whip, comes in on the fourth swell.
pub const STEP4_START: f64 = 12.0;
/// The fifth swell: the beat. The scroller sets off so that its first
/// code — "Welly well" starting the beat — reaches the middle just
/// before it, and the beat lands on this bar.
pub const BEAT_AT: f64 = 16.0;
/// Step 4, the DYCP: seconds sliding in from the right on the full wave,
/// waving on in the middle, settling, lying still to be read, and
/// sliding out to the left as the wave swells again.
const SWAY_IN: f64 = 0.9;
const SWAY_WAVE: f64 = 1.0;
const SWAY_SETTLE: f64 = 0.4;
const SWAY_REST: f64 = 0.8;
const SWAY_OUT: f64 = 0.8;
/// From arriving in the middle to setting off again.
const SWAY_HOLD: f64 = SWAY_WAVE + SWAY_SETTLE + SWAY_REST;
/// The wave: huge — rows up and down at its biggest, riding this many
/// rows above the line while it waves (so the troughs stay on screen),
/// radians from one letter to the next (a long wave: neighbours at most
/// two rows apart, one sweeping wave, not scattered letters), and seconds
/// a swing.
const SWAY_AMP: f64 = 11.0;
const SWAY_ABOVE: f64 = 5.0;
const SWAY_SPREAD: f64 = 0.14;
const SWAY_PERIOD: f64 = 1.3;

/// The cold open laid out for one screen.
pub struct Intro {
    pub timeline: Timeline,
    /// Seconds before the scroller starts.
    pub secs: f64,
    /// Seconds until the last of it has gone (line 4 may still be on its
    /// way out as the scroller sets off).
    pub shown: f64,
    /// When steps 2, 3 and 4 start — skip stops.
    pub marks: [f64; 3],
}

impl Intro {
    /// The cold open for a `width` × `height` screen, on the scroller's
    /// line: `scroll_y` is the scroller's glyph top in pixel rows.
    pub fn new(width: usize, height: usize, scroll_y: i64) -> Self {
        // Lines 1 and 2 in the terminal's own font, centred across, on the
        // middle of the scroller's letters.
        let row = (scroll_y.max(0) as usize + 4) / 2;
        let centred = |line: &str| text(line, glyphs::LETTER_HIGHLIGHT).pad(row, 0, 0, width.saturating_sub(line.len()) / 2);
        let step3_end = STEP3_START + IN_SECS + HOLD_SECS + OUT_SECS;
        let step4_end = STEP4_START + SWAY_IN + SWAY_HOLD + SWAY_OUT;
        let timeline = Timeline::from_buffer(FrameBuffer::new(width, height), step4_end)
            .at(STEP1.0..STEP1.1, centred(LINE1))
            .at(STEP2.0..STEP2.1, centred(LINE2))
            .at(STEP2.0..STEP2.0 + FADE_SECS, AlphaIn::new(FADE_SECS))
            .at(STEP2.1 - FADE_SECS..STEP2.1, AlphaOut::new(FADE_SECS))
            .at(STEP3_START..step3_end, Slide { inner: Box::new(text(LINE3, glyphs::LETTER_HIGHLIGHT)), width, top: row as i64 })
            .at(STEP4_START..step4_end, Sway { line: LINE4, width, row });
        // The scroller's first code reaches the middle half a screen after
        // it sets off (a pixel a frame); a hair before the beat's bar.
        let secs = (BEAT_AT - 0.05 - width as f64 / 2.0 / FPS).max(step3_end + 0.2);
        let shown = step4_end.max(secs);
        Self { timeline, secs, shown, marks: [STEP2.0, STEP3_START, STEP4_START] }
    }
}

/// `line` on row `row` of a screen `width` wide, as a DYCP: sliding in
/// from the right on a wave that dies down as it arrives, lying still in
/// the middle, sliding off to the left as the wave swells again.
struct Sway {
    line: &'static str,
    width: usize,
    row: usize,
}

impl Sway {
    /// Left column of the line, `t` seconds in.
    fn x(&self, t: f64) -> f64 {
        let len = self.line.chars().count() as f64;
        let (right, mid, gone) = (self.width as f64, (self.width as f64 - len) / 2.0, -len);
        if t < SWAY_IN {
            right + (mid - right) * Easing::EaseOut.apply(t / SWAY_IN)
        } else if t < SWAY_IN + SWAY_HOLD {
            mid
        } else {
            mid + (gone - mid) * Easing::EaseIn.apply(((t - SWAY_IN - SWAY_HOLD) / SWAY_OUT).min(1.0))
        }
    }

    /// How big the wave is, 0–1, `t` seconds in.
    fn swell(t: f64) -> f64 {
        let settle = t - SWAY_IN - SWAY_WAVE;
        if settle < 0.0 {
            // The full wave all the way in, and a moment more in the middle.
            1.0
        } else if settle < SWAY_SETTLE {
            1.0 - Easing::EaseInOut.apply(settle / SWAY_SETTLE)
        } else if t < SWAY_IN + SWAY_HOLD {
            0.0
        } else {
            // Swelling quickly, so it waves the moment it moves.
            ((t - SWAY_IN - SWAY_HOLD) / SWAY_OUT).clamp(0.0, 1.0).sqrt()
        }
    }

    /// Rows letter `i` rides off the line, `t` seconds in.
    fn lift(i: usize, t: f64) -> f64 {
        let swing = (std::f64::consts::TAU * t / SWAY_PERIOD + i as f64 * SWAY_SPREAD).sin();
        Self::swell(t) * (SWAY_ABOVE + SWAY_AMP * swing)
    }

    /// Letter `i`'s colour: a calm run along the line, pink to cream to
    /// lavender.
    fn colour(i: usize, len: usize) -> Color {
        let k = 2.0 * i as f64 / (len.max(2) - 1) as f64;
        if k < 1.0 {
            Color::lerp_rgb(glyphs::LETTER_UPPER, glyphs::LETTER_HIGHLIGHT, k)
        } else {
            Color::lerp_rgb(glyphs::LETTER_HIGHLIGHT, glyphs::LETTER_LOWER, k - 1.0)
        }
    }
}

impl Effect for Sway {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 / FPS;
        let len = self.line.chars().count();
        let x0 = self.x(t).round() as i64;
        for (i, ch) in self.line.chars().enumerate() {
            let (x, y) = (x0 + i as i64, self.row as i64 - Self::lift(i, t).round() as i64);
            if ch != ' ' && x >= 0 && y >= 0 && (x as usize) < buf.width && (y as usize) < buf.height {
                buf.set(x as usize, y as usize, Cell::new(ch, Self::colour(i, len)));
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.row + (SWAY_AMP - SWAY_ABOVE) as usize + 1)
    }
}

/// `inner` with its top on row `top`: easing in from just off the right
/// edge to the middle, holding there, easing out past the left edge.
struct Slide {
    inner: Box<dyn Effect>,
    width: usize,
    top: i64,
}

impl Slide {
    /// Left column of the content `t` seconds in.
    fn x(&self, t: f64) -> i64 {
        let iw = self.inner.size().0 as f64;
        let (right, mid, gone) = (self.width as f64, (self.width as f64 - iw) / 2.0, -iw);
        let x = if t < IN_SECS {
            right + (mid - right) * Easing::EaseOut.apply(t / IN_SECS)
        } else if t < IN_SECS + HOLD_SECS {
            mid
        } else {
            mid + (gone - mid) * Easing::EaseIn.apply(((t - IN_SECS - HOLD_SECS) / OUT_SECS).min(1.0))
        };
        x.round() as i64
    }
}

impl Effect for Slide {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let (iw, ih) = self.inner.size();
        let mut sub = FrameBuffer::new(iw, ih);
        self.inner.render(&mut sub, frame);
        let x0 = self.x(frame as f64 / FPS);
        for y in 0..ih {
            for x in 0..iw {
                let cell = sub.get(x, y);
                let (tx, ty) = (x0 + x as i64, self.top + y as i64);
                if cell.ch != ' ' && tx >= 0 && ty >= 0 && (tx as usize) < buf.width && (ty as usize) < buf.height {
                    buf.set(tx as usize, ty as usize, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.top as usize + self.inner.size().1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 112;
    const H: usize = 50;
    /// The scroller's glyph top, in pixel rows.
    const SCROLL_Y: i64 = 74;

    fn frame_at(intro: &Intro, t: f64) -> FrameBuffer {
        let mut buf = FrameBuffer::new(W, H);
        intro.timeline.render(&mut buf, (t * FPS).round() as usize);
        buf
    }

    fn text_of(buf: &FrameBuffer) -> String {
        (0..buf.height)
            .map(|y| (0..buf.width).map(|x| buf.get(x, y).ch).collect::<String>().trim().to_string())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// (column, row) where `line` starts on screen.
    fn line_at(buf: &FrameBuffer, line: &str) -> Option<(usize, usize)> {
        (0..buf.height).find_map(|y| {
            let row: String = (0..buf.width).map(|x| buf.get(x, y).ch).collect();
            row.find(line).map(|x| (x, y))
        })
    }

    #[test]
    fn step_one_is_a_hard_cut_in_the_middle() {
        let intro = Intro::new(W, H, SCROLL_Y);
        assert_eq!(text_of(&frame_at(&intro, 0.2)), "");
        let buf = frame_at(&intro, 1.0);
        assert_eq!(text_of(&buf), LINE1);
        // On the scroller's line: the middle of its 8-pixel letters.
        assert_eq!(line_at(&buf, LINE1), Some(((W - LINE1.len()) / 2, (SCROLL_Y as usize + 4) / 2)));
        // Full colour from the first frame on: no fade.
        let (x, y) = line_at(&buf, LINE1).unwrap();
        assert_eq!(frame_at(&intro, STEP1.0).get(x, y).color, buf.get(x, y).color);
        assert_eq!(text_of(&frame_at(&intro, STEP1.1 + 0.2)), "");
    }

    #[test]
    fn step_two_fades_in_and_out() {
        let intro = Intro::new(W, H, SCROLL_Y);
        let luma = |t: f64| {
            let buf = frame_at(&intro, t);
            let (x, y) = line_at(&buf, LINE2).expect("line 2 on screen");
            buf.get(x, y).color.luma()
        };
        let full = luma(STEP2.0 + 2.0);
        assert!(luma(STEP2.0 + 0.2) < full);
        assert!(luma(STEP2.1 - 0.2) < full);
        assert_eq!(text_of(&frame_at(&intro, STEP2.1 + 0.2)), "");
    }

    #[test]
    fn step_three_scrolls_in_holds_and_scrolls_out() {
        let intro = Intro::new(W, H, SCROLL_Y);
        assert_eq!(intro.marks, [STEP2.0, STEP3_START, STEP4_START]);
        let row = (SCROLL_Y as usize + 4) / 2;
        // Where the line's first letter is, if on screen.
        let at = |t: f64| -> Option<usize> {
            let buf = frame_at(&intro, t);
            (0..W).find(|&x| buf.get(x, row).ch != ' ')
        };
        // Starts off the right edge; scrolls in easing out — big steps
        // first, small ones as it settles.
        assert_eq!(text_of(&frame_at(&intro, STEP3_START)), "");
        let xs: Vec<usize> = (1..(IN_SECS * FPS) as usize).filter_map(|f| at(STEP3_START + f as f64 / FPS)).collect();
        assert!(xs.windows(2).all(|w| w[1] <= w[0]), "always leftward: {xs:?}");
        assert!(xs[0] - xs[3] > xs[xs.len() - 4] - xs[xs.len() - 1], "eases out: {xs:?}");
        // Holds still, whole and centred, in the terminal's own font.
        let held = STEP3_START + IN_SECS + 0.3;
        let buf = frame_at(&intro, held);
        assert_eq!(line_at(&buf, LINE3), Some(((W - LINE3.len()) / 2, row)));
        assert_eq!(at(held + 0.5), at(held));
        // Then leaves to the left, gone before line 4 comes in.
        assert_eq!(text_of(&frame_at(&intro, STEP4_START - 0.02)), "");
    }

    /// Rows the line's letters are on, `t` seconds into the cold open.
    fn rows_at(intro: &Intro, t: f64) -> std::collections::BTreeSet<usize> {
        let buf = frame_at(intro, t);
        (0..H).filter(|&y| (0..W).any(|x| buf.get(x, y).ch != ' ')).collect()
    }

    /// Leftmost column with ink, `t` seconds into the cold open.
    fn leftmost(intro: &Intro, t: f64) -> Option<usize> {
        let buf = frame_at(intro, t);
        (0..W).find(|&x| (0..H).any(|y| buf.get(x, y).ch != ' '))
    }

    #[test]
    fn step_four_slides_in_and_out_as_a_dycp_and_lies_still_between() {
        let intro = Intro::new(W, H, SCROLL_Y);
        let row = (SCROLL_Y as usize + 4) / 2;
        let len = LINE4.len();
        // Sliding in from the right, waving.
        let coming = STEP4_START + 0.3;
        assert!(leftmost(&intro, coming).unwrap() > (W - len) / 2, "not coming from the right");
        assert!(rows_at(&intro, coming).len() >= 3, "{:?}", rows_at(&intro, coming));
        // One wave, never scattered letters: neighbours at most two rows apart.
        for f in 0..((SWAY_IN + SWAY_HOLD + SWAY_OUT) * FPS) as usize {
            let t = f as f64 / FPS;
            for i in 1..len {
                let (a, b) = (Sway::lift(i - 1, t).round(), Sway::lift(i, t).round());
                assert!((a - b).abs() <= 2.0, "letters {} and {i} at {t:.2}s: {a} and {b}", i - 1);
            }
            // On screen, inside the border, crests and troughs alike.
            for i in 0..len {
                let y = row as f64 - Sway::lift(i, t).round();
                assert!((1.0..(H - 1) as f64).contains(&y), "letter {i} off screen at {t:.2}s: row {y}");
            }
        }
        // Waving on in the middle first — a huge wave, well over 15 rows...
        let waving = STEP4_START + SWAY_IN + SWAY_WAVE / 2.0;
        assert!(rows_at(&intro, waving).len() >= 15, "{:?}", rows_at(&intro, waving));
        // ...then still.
        for t in [STEP4_START + SWAY_IN + SWAY_WAVE + SWAY_SETTLE + 0.05, STEP4_START + SWAY_IN + SWAY_HOLD - 0.05] {
            assert_eq!(line_at(&frame_at(&intro, t), LINE4), Some(((W - len) / 2, row)), "at {t}");
        }
        // Sliding out to the left, waving again; then gone.
        let going = STEP4_START + SWAY_IN + SWAY_HOLD + 0.4;
        assert!(leftmost(&intro, going).unwrap() < (W - len) / 2, "not going left");
        assert!(rows_at(&intro, going).len() >= 3, "{:?}", rows_at(&intro, going));
        assert_eq!(text_of(&frame_at(&intro, STEP4_START + SWAY_IN + SWAY_HOLD + SWAY_OUT + 0.05)), "");
        // In colour: not all one.
        let buf = frame_at(&intro, STEP4_START + SWAY_IN + SWAY_WAVE + SWAY_SETTLE + 0.3);
        let colours: std::collections::BTreeSet<(u8, u8, u8)> =
            (0..W).map(|x| buf.get(x, row)).filter(|c| c.ch != ' ').map(|c| (c.color.r, c.color.g, c.color.b)).collect();
        assert!(colours.len() > 3);
    }

    #[test]
    fn every_line_comes_in_on_a_pad_swell_and_the_beat_on_the_fifth() {
        for (start, swell) in [STEP1.0, STEP2.0, STEP3_START, STEP4_START].iter().zip([0.0, 4.0, 8.0, 12.0]) {
            assert!((0.0..0.5).contains(&(start - swell)), "line at {start}, swell at {swell}");
        }
        // The scroller's first code reaches the middle just before the beat.
        for w in [80, 112, 160, 260] {
            let intro = Intro::new(w, H, SCROLL_Y);
            let first_code = intro.secs + w as f64 / 2.0 / FPS;
            assert!((BEAT_AT - 2.0..BEAT_AT).contains(&first_code), "w {w}: {first_code}");
        }
    }
}
