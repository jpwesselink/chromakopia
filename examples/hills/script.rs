//! The scroll text runs the show. Control codes in curly braces — like
//! `{logo}` or `{pause 2}` — fire the moment their spot in the text
//! passes the middle of the screen.
//!
//! The text is parsed once into plain characters plus events, then turned
//! into a `Timeline` for one screen width: storyboard time → how far the
//! text has scrolled, and when each code fires. Everything on screen is a
//! function of time, so skipping around always lands in a consistent
//! state.

use chromakopia::animate::framebuffer::FPS;

use super::dive;
use super::looping::{self, Shape};
use super::scroller::{self, Font, Style};

/// Scroll speed, pixels per second (one pixel per frame).
const SPEED: f64 = FPS;

/// A control code in the scroll text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Code {
    /// The landscape builds up: sky slides down, hills rise layer by layer.
    Hills,
    /// CHROMAKOPIA drops in at the centre; the tagline types out under it.
    Logo,
    /// The logo jumps away, making room — until the next `{logo}`.
    LogoAway,
    /// The whole logo jumps off, C and all, as at `{bye}` — the next
    /// `{logo}` drops it in afresh.
    LogoOut,
    /// The text stands still for this many seconds (the wave keeps going).
    Pause(f64),
    /// Half a screen of empty space — so what comes before has scrolled
    /// off (or what comes after is still waiting off the right edge) when
    /// a code next to it reaches the middle.
    Gap,
    /// Everything after is set in this font.
    Font(Font),
    /// Everything after is drawn in this style.
    Style(Style),
    /// What everything after rides.
    Wave(Wave),
    /// Everything after zooms (true): each letter grows and shrinks on a
    /// wave of its own, in a slot as wide as its biggest.
    Zoom(bool),
    /// Everything after rides this path across the screen (None: back on
    /// the scroller's line).
    Path(Option<Shape>),
    /// The camera dives into the big C (true) — what follows rides its
    /// liquid's surface — or comes back out (false).
    Dive(bool),
    /// Everything leaves: the logo jumps away, the landscape takes
    /// itself down — before the text wraps and the show starts over.
    Bye,
    /// Side scroller `n` of `sides::SIDES` sets off across the screen.
    Side(usize),
    /// A skip stop with nothing attached (set by `with_lead`, not typed).
    Mark,
    /// Dived in, the liquid's layers spread into waves (set by `place`,
    /// not typed).
    Spread,
}

/// What a letter rides, switched with `{wave on|off|long}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wave {
    /// A straight line.
    Off,
    /// The scroller's ripple.
    On,
    /// A long, slow wave from corner to corner instead of the ripple,
    /// swelled in this much (0..=1) — it grows in after the code and dies
    /// away before the next.
    Long(f64),
    /// The surface of the liquid in the big C, once the camera dives in.
    Water,
}

/// Text pixels a long wave takes to swell in, and to die away.
const LONG_RAMP: f64 = 48.0;

/// The scroll text without its codes, and where each code sits.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub text: String,
    /// (character index in `text`, code) in text order.
    pub events: Vec<(usize, Code)>,
}

/// Split `src` into plain text and codes. Unknown or malformed codes are
/// an error — the text is fixed, so this fails loudly at startup.
pub fn parse(src: &str) -> Result<Script, String> {
    let mut text = String::new();
    let mut events = Vec::new();
    let mut rest = src;
    while let Some(open) = rest.find('{') {
        text.push_str(&rest[..open]);
        let close = rest[open..].find('}').ok_or_else(|| format!("unclosed code: {}", &rest[open..]))? + open;
        let body = &rest[open + 1..close];
        if body.is_empty() {
            // `{}` is text — a handle, not a code.
            text.push_str("{}");
            rest = &rest[close + 1..];
            continue;
        }
        let mut words = body.split_whitespace();
        let code = match (words.next(), words.next(), words.next()) {
            (Some("logo"), None, _) => Code::Logo,
            (Some("logo"), Some("away"), None) => Code::LogoAway,
            (Some("logo"), Some("out"), None) => Code::LogoOut,
            (Some("dive"), Some("in"), None) => Code::Dive(true),
            (Some("dive"), Some("out"), None) => Code::Dive(false),
            (Some("hills"), None, _) => Code::Hills,
            (Some("bye"), None, _) => Code::Bye,
            (Some("gap"), None, _) => Code::Gap,
            (Some("side"), Some(name), None) => {
                Code::Side(super::sides::named(name).ok_or_else(|| format!("unknown side scroller {{{body}}}"))?)
            }
            (Some("font"), Some(name), None) => {
                Code::Font(Font::named(name).ok_or_else(|| format!("unknown font {{{body}}}"))?)
            }
            (Some("wave"), Some("on"), None) => Code::Wave(Wave::On),
            (Some("wave"), Some("off"), None) => Code::Wave(Wave::Off),
            (Some("wave"), Some("long"), None) => Code::Wave(Wave::Long(1.0)),
            (Some("zoom"), Some("on"), None) => Code::Zoom(true),
            (Some("zoom"), Some("off"), None) => Code::Zoom(false),
            (Some("path"), Some("off"), None) => Code::Path(None),
            (Some("path"), Some(name), None) => {
                Code::Path(Some(Shape::named(name).ok_or_else(|| format!("unknown path {{{body}}}"))?))
            }
            (Some("style"), Some(name), None) => {
                Code::Style(Style::named(name).ok_or_else(|| format!("unknown style {{{body}}}"))?)
            }
            (Some("pause"), Some(secs), None) => {
                Code::Pause(secs.parse().map_err(|_| format!("bad seconds in {{{body}}}"))?)
            }
            _ => return Err(format!("unknown code {{{body}}}")),
        };
        events.push((text.chars().count(), code));
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    Ok(Script { text, events })
}

/// One letter of the scroll text, placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub ch: char,
    /// Left edge in text pixels (0 = start of the text).
    pub x: f64,
    pub font: Font,
    pub style: Style,
    /// What it rides (terminal letters in whole rows).
    pub wave: Wave,
    /// Rides this path across the screen instead.
    pub path: Option<Shape>,
    /// Grows and shrinks (`{zoom on}`).
    pub zoom: bool,
}

/// The script laid out for one screen width: every visible letter with its
/// position, font and style, and where each remaining code sits.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Visible letters only; spaces just advance.
    pub glyphs: Vec<Glyph>,
    /// (text pixel, code) for the codes that fire: logo, hills, pause.
    pub events: Vec<(f64, Code)>,
    /// Width of the whole text in pixels.
    pub length: f64,
}

impl Script {
    /// Lay the text out for a screen `width` columns wide: each letter
    /// as wide as its font makes it, `{gap}` as half a screen of space.
    /// After a path, what follows waits the length of the ride so the
    /// riders are out before it comes through. A letter its font doesn't
    /// have is an error.
    pub fn place(&self, width: usize) -> Result<Placed, String> {
        let (mut font, mut style, mut wave, mut path, mut zoom) = (Font::C64, Style::Candy, Wave::On, None, false);
        // Where each long wave starts and ends, in text pixels.
        let mut longs: Vec<(f64, f64)> = Vec::new();
        let mut dived = false;
        // Dived in, with the waves yet to spread.
        let mut to_spread = false;
        let (mut glyphs, mut events, mut x) = (Vec::new(), Vec::new(), 0.0);
        let mut codes = self.events.iter().peekable();
        let chars: Vec<char> = self.text.chars().collect();
        for i in 0..=chars.len() {
            while let Some(&&(at, code)) = codes.peek() {
                if at != i {
                    break;
                }
                codes.next();
                match code {
                    Code::Gap => x += width as f64 / 2.0,
                    Code::Font(f) => font = f,
                    Code::Style(st) => style = st,
                    Code::Wave(next) => {
                        match (wave, next) {
                            (Wave::Long(_), Wave::Long(_)) => {}
                            (Wave::Long(_), _) => longs.last_mut().unwrap().1 = x,
                            (_, Wave::Long(_)) => longs.push((x, f64::INFINITY)),
                            _ => {}
                        }
                        wave = next;
                    }
                    Code::Zoom(on) => zoom = on,
                    Code::Dive(true) => {
                        // Room for the camera to go in before what follows
                        // comes on; the waves spread later (see `spread`).
                        dived = true;
                        to_spread = true;
                        events.push((x, code));
                        x += width as f64 / 2.0 + dive::DIVE_SECS * SPEED;
                    }
                    Code::Dive(false) => {
                        if std::mem::take(&mut to_spread) {
                            spread(&mut x, &mut events, &glyphs, width);
                        }
                        dived = false;
                        events.push((x, code));
                        x += width as f64 / 2.0 + dive::OUT_SECS * SPEED;
                    }
                    Code::Path(next) => {
                        if let Some(shape) = path.filter(|&p| Some(p) != next) {
                            x += looping::extra(shape, width);
                        }
                        path = next;
                    }
                    _ => events.push((x, code)),
                }
            }
            let Some(&ch) = chars.get(i) else { break };
            if !scroller::has(font, ch) {
                return Err(format!("font {font:?} has no {ch:?} (in {:?})", chars[i.saturating_sub(10)..(i + 10).min(chars.len())].iter().collect::<String>()));
            }
            if to_spread && path != Some(Shape::Spin) {
                // The first letter to ride the waves: they spread first.
                to_spread = false;
                spread(&mut x, &mut events, &glyphs, width);
            }
            let g = Glyph { ch, x, font, style, wave: if dived { Wave::Water } else { wave }, path, zoom };
            let zooms = scroller::zooms(&g);
            if ch != ' ' || zooms {
                glyphs.push(g);
            }
            x += if zooms { scroller::zoom_advance(ch) } else { scroller::advance(font, ch) } as f64;
        }
        if let Some(last) = longs.last_mut().filter(|l| l.1.is_infinite()) {
            last.1 = x;
        }
        for g in &mut glyphs {
            if let Wave::Long(_) = g.wave {
                let &(start, end) = longs.iter().find(|&&(a, b)| (a..b).contains(&g.x)).unwrap();
                let right = g.x + scroller::advance(g.font, g.ch) as f64;
                let p = ((g.x - start).min(end - right) / LONG_RAMP).clamp(0.0, 1.0);
                g.wave = Wave::Long(p * p * (3.0 - 2.0 * p));
            }
        }
        Ok(Placed { glyphs, events, length: x })
    }
}

/// Once dived in, the liquid's layers spread into waves: once the camera
/// is in — and, with letters riding the turning surface, once it has come
/// to rest — at scroll `x`. What follows waits for the waves to spread.
fn spread(x: &mut f64, events: &mut Vec<(f64, Code)>, glyphs: &[Glyph], width: usize) {
    let mut at = *x;
    if glyphs.iter().any(|g| g.path == Some(Shape::Spin)) {
        if let Some(ride) = looping::ride(glyphs, width) {
            at = at.max(looping::spin_done(ride));
        }
    }
    events.push((at - width as f64 / 2.0, Code::Spread));
    *x = at + dive::WAVES_SECS * SPEED;
}

/// When a code fires.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue {
    /// Storyboard seconds.
    pub t: f64,
    pub code: Code,
}

/// The script played on a screen `width` columns wide.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub cues: Vec<Cue>,
    /// Seconds until the text has fully scrolled off and the show loops.
    pub total: f64,
    /// Pauses as (scrolled px, start time, seconds), in order.
    pauses: Vec<(f64, f64, f64)>,
    /// Seconds before the text starts scrolling in.
    lead: f64,
}

impl Timeline {
    pub fn new(placed: &Placed, width: usize) -> Self {
        let mut cues = Vec::new();
        let mut pauses = Vec::new();
        let mut paused = 0.0;
        for &(x, code) in &placed.events {
            // A glyph at text pixel p is at screen x = width - scrolled + p,
            // so it reaches the middle once scrolled = p + width / 2.
            let px = x + width as f64 / 2.0;
            let t = px / SPEED + paused;
            cues.push(Cue { t, code });
            if let Code::Pause(secs) = code {
                pauses.push((px, t, secs));
                paused += secs;
            }
        }
        let run = placed.length + width as f64;
        Self { cues, total: run / SPEED + paused, pauses, lead: 0.0 }
    }

    /// The same timeline starting `lead` seconds later — something else
    /// plays first — with extra skip stops at `marks` and at the moment the
    /// text starts.
    pub fn with_lead(mut self, lead: f64, marks: &[f64]) -> Self {
        for c in &mut self.cues {
            c.t += lead;
        }
        for p in &mut self.pauses {
            p.1 += lead;
        }
        let stops = marks.iter().copied().chain([lead]).map(|t| Cue { t, code: Code::Mark });
        self.cues.splice(0..0, stops);
        self.total += lead;
        self.lead = lead;
        self
    }

    /// Pixels the text has scrolled at time `t` (0 = just off the right edge).
    pub fn scrolled(&self, t: f64) -> f64 {
        let mut paused = 0.0;
        for &(px, start, secs) in &self.pauses {
            if t < start {
                break;
            }
            if t < start + secs {
                return px;
            }
            paused += secs;
        }
        (t - self.lead - paused) * SPEED
    }

    /// When the latest `code` at or before `t` fired.
    pub fn since(&self, code: Code, t: f64) -> Option<f64> {
        self.cues.iter().rev().find(|c| c.code == code && c.t <= t).map(|c| c.t)
    }

    /// The next cue strictly after `t`, or the start when there is none.
    pub fn next_after(&self, t: f64) -> f64 {
        self.cues.iter().map(|c| c.t).find(|&ct| ct > t + 1e-6).unwrap_or(0.0)
    }

    /// The cue before the one we're at, or the start. A cue only just
    /// passed counts as "the one we're at", so pressing back twice walks
    /// back two cues.
    pub fn prev_before(&self, t: f64) -> f64 {
        self.cues.iter().map(|c| c.t).filter(|&ct| ct < t - 0.5).last().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_codes_and_records_their_positions() {
        let s = parse("ab{logo}cd{pause 2}e{hills}{bye}").unwrap();
        assert_eq!(s.text, "abcde");
        assert_eq!(s.events, vec![(2, Code::Logo), (4, Code::Pause(2.0)), (5, Code::Hills), (5, Code::Bye)]);
    }

    #[test]
    fn empty_braces_are_text_not_a_code() {
        // `{}` is a handle, not a code: it scrolls by as written.
        let s = parse("Exel, {} and Hardbeat{logo}").unwrap();
        assert_eq!(s.text, "Exel, {} and Hardbeat");
        assert_eq!(s.events, vec![(21, Code::Logo)]);
    }

    #[test]
    fn path_marks_its_letters_and_leaves_room_for_the_ride() {
        let s = parse("a{path loop}bc{path off}d").unwrap();
        assert!(parse("{path sideways}").is_err());
        assert_eq!(parse("{logo away}").unwrap().events, vec![(0, Code::LogoAway)]);
        assert_eq!(parse("{logo out}").unwrap().events, vec![(0, Code::LogoOut)]);
        let p = s.place(100).unwrap();
        let paths: Vec<_> = p.glyphs.iter().map(|g| g.path).collect();
        assert_eq!(paths, [None, Some(Shape::Loop), Some(Shape::Loop), None]);
        // Letters and spaces sit as far apart as on the line.
        assert_eq!(p.glyphs[2].x - p.glyphs[1].x, 8.0);
        let spaced = parse("{path loop}a b{path off}").unwrap().place(100).unwrap();
        assert_eq!(spaced.glyphs[1].x - spaced.glyphs[0].x, 16.0);
        // What follows waits until the last looper is out.
        assert_eq!(p.glyphs[3].x, 24.0 + looping::LOOP_EXTRA);
        assert!(p.events.is_empty(), "nothing to fire: {:?}", p.events);
    }

    #[test]
    fn dived_in_the_spin_riders_come_first_then_the_waves_spread() {
        let w = 100;
        let p = parse("{path loop}ab{path off}{dive in}{path spin}cd{path off}ef{dive out}g").unwrap().place(w).unwrap();
        assert_eq!(parse("{path spin}").unwrap().events, vec![(0, Code::Path(Some(Shape::Spin)))]);
        let codes: Vec<Code> = p.events.iter().map(|e| e.1).collect();
        assert_eq!(codes, [Code::Dive(true), Code::Spread, Code::Dive(false)]);
        let fire = |i: usize| p.events[i].0 + w as f64 / 2.0;
        let at = |ch| p.glyphs.iter().find(|g| g.ch == ch).unwrap();
        // The spin riders come on once the camera is in...
        assert_eq!(at('c').x, fire(0) + dive::DIVE_SECS * SPEED);
        assert_eq!(at('c').path, Some(Shape::Spin));
        // ...the waves spread once the liquid has come to rest...
        let ride = looping::ride(&p.glyphs, w).unwrap();
        assert!(fire(1) >= looping::spin_done(ride) - 1e-9 && fire(1) >= at('c').x);
        // ...and the water's own text comes on once they have.
        assert_eq!(at('e').x, fire(1) + dive::WAVES_SECS * SPEED);
        assert_eq!(at('e').wave, Wave::Water);
    }

    #[test]
    fn without_spin_riders_the_waves_spread_once_the_camera_is_in() {
        let w = 100;
        let p = parse("{dive in}ab{dive out}").unwrap().place(w).unwrap();
        let fire = |i: usize| p.events[i].0 + w as f64 / 2.0;
        assert_eq!(p.events[1].1, Code::Spread);
        assert_eq!(fire(1), fire(0) + dive::DIVE_SECS * SPEED);
        assert_eq!(p.glyphs[0].x, fire(0) + dive::IN_SECS * SPEED);
    }

    #[test]
    fn parse_rejects_unknown_and_broken_codes() {
        assert!(parse("a{boom}b").unwrap_err().contains("boom"));
        assert!(parse("a{pause}b").is_err());
        assert!(parse("a{pause x}b").is_err());
        assert!(parse("a{logo").is_err());
    }

    #[test]
    fn parse_font_and_style_codes() {
        let s = parse("{font tiny}a{style flat}b{font term}{wave off}c{wave on}").unwrap();
        assert_eq!(
            s.events,
            vec![
                (0, Code::Font(Font::Tiny)),
                (1, Code::Style(Style::Flat)),
                (2, Code::Font(Font::Term)),
                (2, Code::Wave(Wave::Off)),
                (3, Code::Wave(Wave::On)),
            ]
        );
        assert!(parse("{wave sideways}").is_err());
        let p = s.place(100).unwrap();
        assert_eq!(p.glyphs.iter().map(|g| g.wave).collect::<Vec<_>>(), vec![Wave::On, Wave::On, Wave::Off]);
        assert!(parse("{font comic-sans}").unwrap_err().contains("comic-sans"));
        assert!(parse("{style}").is_err());
    }

    #[test]
    fn letters_in_a_dive_ride_the_water() {
        let s = parse("a{dive in}bc{dive out}d").unwrap();
        assert_eq!(s.events, vec![(1, Code::Dive(true)), (3, Code::Dive(false))]);
        assert!(parse("{dive deep}").is_err());
        let p = s.place(100).unwrap();
        assert_eq!(p.glyphs.iter().map(|g| g.wave).collect::<Vec<_>>(), vec![Wave::On, Wave::Water, Wave::Water, Wave::On]);
        assert_eq!(p.events.iter().map(|e| e.1).collect::<Vec<_>>(), vec![Code::Dive(true), Code::Spread, Code::Dive(false)]);
        // Each leaves room: half a screen, plus as far as the text scrolls
        // while the camera goes in (or out) — what follows waits for it.
        let xs: Vec<f64> = p.glyphs.iter().map(|g| g.x).collect();
        let (lead_in, lead_out) = (50.0 + dive::IN_SECS * SPEED, 50.0 + dive::OUT_SECS * SPEED);
        assert_eq!(xs, vec![0.0, 8.0 + lead_in, 16.0 + lead_in, 24.0 + lead_in + lead_out]);
    }

    #[test]
    fn long_waves_swell_in_and_out() {
        let s = parse("ab{wave long}cdefghijklmnopqrstuvwxy{wave on}z").unwrap();
        assert_eq!(s.events[0], (2, Code::Wave(Wave::Long(1.0))));
        let p = s.place(100).unwrap();
        let w: Vec<f64> = p.glyphs.iter().map(|g| if let Wave::Long(w) = g.wave { w } else { -1.0 }).collect();
        assert_eq!(&w[..2], &[-1.0, -1.0]);
        assert_eq!(*w.last().unwrap(), -1.0);
        // From the plain wave into the long one and back, not with a jump.
        let long = &w[2..w.len() - 1];
        assert!(long[0] < 0.2 && long[long.len() - 1] < 0.2, "{long:?}");
        assert!(long.iter().any(|&v| v == 1.0), "{long:?}");
        assert!(long.windows(2).take(4).all(|p| p[0] < p[1]), "{long:?}");
    }

    #[test]
    fn place_gives_every_letter_its_own_width() {
        let p = parse("ab{font big}cd{font tiny}e{font term}fg").unwrap().place(100).unwrap();
        let xs: Vec<(char, f64)> = p.glyphs.iter().map(|g| (g.ch, g.x)).collect();
        let e = scroller::advance(Font::Tiny, 'e') as f64;
        assert_eq!(xs, vec![('a', 0.0), ('b', 8.0), ('c', 16.0), ('d', 32.0), ('e', 48.0), ('f', 48.0 + e), ('g', 49.0 + e)]);
        assert_eq!(p.glyphs[4].font, Font::Tiny);
        assert_eq!(p.length, 50.0 + e);
    }

    #[test]
    fn zoomed_letters_get_room_to_grow() {
        let s = parse("ab{zoom on}c d{zoom off}e").unwrap();
        assert_eq!(s.events, vec![(2, Code::Zoom(true)), (5, Code::Zoom(false))]);
        assert!(parse("{zoom sideways}").is_err());
        let p = s.place(100).unwrap();
        // The space comes along: it grows and shrinks with its neighbours.
        let chars: String = p.glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, "abc de");
        assert_eq!(p.glyphs.iter().map(|g| g.zoom).collect::<Vec<_>>(), vec![false, false, true, true, true, false]);
        // Each zoomed letter takes as much room as it has ink, on average.
        let adv = |ch| scroller::zoom_advance(ch) as f64;
        let xs: Vec<f64> = p.glyphs.iter().map(|g| g.x).collect();
        let c = 16.0 + adv('c');
        assert_eq!(xs, vec![0.0, 8.0, 16.0, c, c + adv(' '), c + adv(' ') + adv('d')]);
        assert!(adv('i') < adv('m'));
    }

    #[test]
    fn place_rejects_letters_a_font_does_not_have() {
        let err = parse("{font mario}CHROMAhello").unwrap().place(100).unwrap_err();
        assert!(err.contains('h'), "{err}");
        assert!(parse("{font mario}CHROMAKOPIA").unwrap().place(100).is_ok());
    }

    #[test]
    fn gap_is_half_a_screen_of_space() {
        let p = parse("ab{gap}{logo}c").unwrap().place(32).unwrap();
        assert_eq!(p.glyphs.last().map(|g| g.x), Some(16.0 + 16.0));
        assert_eq!(p.events, vec![(32.0, Code::Logo)]);
    }

    // 10 characters (80 px) on a 100-column screen: a glyph at text pixel
    // p reaches the middle when the text has scrolled p + 50.
    fn timeline() -> Timeline {
        Timeline::new(&parse("abcd{logo}efgh{pause 2}ij").unwrap().place(100).unwrap(), 100)
    }

    #[test]
    fn codes_fire_when_they_reach_the_middle() {
        let tl = timeline();
        // {logo} at char 4 = 32 px → scrolled 82 → 82 / 60 s.
        assert!((tl.cues[0].t - 82.0 / 60.0).abs() < 1e-9);
        assert_eq!(tl.cues[0].code, Code::Logo);
        // {pause} at char 8 = 64 px → scrolled 114.
        assert!((tl.cues[1].t - 114.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn pause_holds_the_text_still() {
        let tl = timeline();
        let at = 114.0 / 60.0;
        assert!((tl.scrolled(at) - 114.0).abs() < 1e-9);
        assert!((tl.scrolled(at + 1.5) - 114.0).abs() < 1e-9);
        assert!((tl.scrolled(at + 2.5) - (114.0 + 30.0)).abs() < 1e-9);
        // Whole run: 80 px of text + 100 px of screen, plus the pause.
        assert!((tl.total - (180.0 / 60.0 + 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_lead_in_shifts_everything_and_adds_skip_stops() {
        let plain = timeline();
        let led = timeline().with_lead(5.0, &[1.0, 2.5]);
        assert!(led.scrolled(4.0) < 0.0, "text still off the right edge");
        assert!((led.scrolled(5.0 + 1.7) - plain.scrolled(1.7)).abs() < 1e-9);
        assert!((led.scrolled(5.0 + 3.0) - plain.scrolled(3.0)).abs() < 1e-9, "pause shifted too");
        assert!((led.total - (plain.total + 5.0)).abs() < 1e-9);
        assert_eq!(led.since(Code::Logo, 5.0 + 2.0), Some(5.0 + 82.0 / 60.0));
        // Skip stops: the marks, the end of the lead-in, then the codes.
        assert_eq!(led.next_after(0.0), 1.0);
        assert_eq!(led.next_after(1.0), 2.5);
        assert_eq!(led.next_after(2.5), 5.0);
        assert_eq!(led.next_after(5.0), 5.0 + 82.0 / 60.0);
    }

    #[test]
    fn logo_shows_from_its_cue_on() {
        let tl = timeline();
        assert_eq!(tl.since(Code::Logo, 1.0), None);
        assert_eq!(tl.since(Code::Logo, 2.0), Some(82.0 / 60.0));
        assert_eq!(tl.since(Code::Hills, 2.0), None);
    }

    #[test]
    fn skipping_walks_the_cues_and_wraps() {
        let tl = timeline();
        let (logo, pause) = (tl.cues[0].t, tl.cues[1].t);
        assert_eq!(tl.next_after(0.0), logo);
        assert_eq!(tl.next_after(logo), pause);
        assert_eq!(tl.next_after(pause), 0.0);
        assert_eq!(tl.prev_before(pause + 0.1), logo);
        assert_eq!(tl.prev_before(logo + 0.1), 0.0);
        assert_eq!(tl.prev_before(pause + 3.0), pause);
    }
}
