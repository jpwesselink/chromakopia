//! The show's running order and the clock that walks it.

pub const INTRO_SECS: f64 = 7.0;
pub const LOGO_EXIT_SECS: f64 = 1.5;
pub const HEADING_SECS: f64 = 1.2;
pub const PANEL_SECS: f64 = 8.0;
pub const FIELD_SECS: f64 = 9.0;
pub const FINALE_SECS: f64 = 6.0;
pub const LAST_BLOCK: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelKind {
    Plasma, Rainbow, Glow, Pulse, Neon, Radar, Karaoke,
    Glitch, Flap, Scroll, Spread, Marquee, VSineWave, Dycp, Fld, Wind,
    VectorField, HeightMap,
    Screen, Multiply, Transition,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepKind {
    Intro,
    LogoExit,
    Heading { title: &'static str },
    Panel { kind: PanelKind, name: &'static str, subtitle: &'static str },
    Finale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub kind: StepKind,
    /// 1 = intro, 2 = color, 3 = shape & layout, 4 = fields, 5 = mixing, 6 = finale.
    pub block: usize,
    pub secs: f64,
}

/// The whole show, in order.
pub fn storyboard() -> Vec<Step> {
    use PanelKind::*;
    let step = |kind, block, secs| Step { kind, block, secs };
    let heading = |title, block| step(StepKind::Heading { title }, block, HEADING_SECS);
    let panel = |kind, name, subtitle, block, secs| step(StepKind::Panel { kind, name, subtitle }, block, secs);

    let mut s = vec![step(StepKind::Intro, 1, INTRO_SECS), step(StepKind::LogoExit, 1, LOGO_EXIT_SECS)];

    s.push(heading("COLOR EFFECTS", 2));
    for (kind, name, sub) in [
        (Plasma, "PLASMA", "smooth as butter"),
        (Rainbow, "RAINBOW", "every hue, all the time"),
        (Glow, "GLOW", "a spotlight with a gradient"),
        (Pulse, "PULSE", "heartbeat from the centre"),
        (Neon, "NEON", "buzzing like a bar sign"),
        (Radar, "RADAR", "sweep, sweep, ping"),
        (Karaoke, "KARAOKE", "sing along, one letter at a time"),
    ] {
        s.push(panel(kind, name, sub, 2, PANEL_SECS));
    }

    s.push(heading("SHAPE & LAYOUT", 3));
    for (kind, name, sub) in [
        (Glitch, "GLITCH", "signal lost"),
        (Flap, "FLAP", "departures, now boarding"),
        (Scroll, "SCROLL", "sliding into place"),
        (Spread, "SPREAD", "fanning out from one line"),
        (Marquee, "MARQUEE", "round and round"),
        (VSineWave, "VSINEWAVE", "riding the wave"),
        (Dycp, "DYCP", "every letter its own height"),
        (Fld, "FLD", "rows floating free"),
        (Wind, "WIND", "blown away"),
    ] {
        s.push(panel(kind, name, sub, 3, PANEL_SECS));
    }

    // Fields fill the panel, so their name rides in the subtitle.
    s.push(heading("FIELDS", 4));
    s.push(panel(VectorField, "VECTORFIELD", "VectorField: follow the flow", 4, FIELD_SECS));
    s.push(panel(HeightMap, "HEIGHTMAP", "HeightMap: hills you can read", 4, FIELD_SECS));

    s.push(heading("MIXING", 5));
    for (kind, name, sub) in [
        (Screen, "SCREEN", "light on light"),
        (Multiply, "MULTIPLY", "shadows with colour"),
        (Transition, "TRANSITION", "from one to the other"),
    ] {
        s.push(panel(kind, name, sub, 5, PANEL_SECS));
    }

    s.push(step(StepKind::Finale, LAST_BLOCK, FINALE_SECS));
    s
}

pub fn total(steps: &[Step]) -> f64 {
    steps.iter().map(|s| s.secs).sum()
}

pub fn start_of(steps: &[Step], i: usize) -> f64 {
    steps[..i].iter().map(|s| s.secs).sum()
}

/// Which step is playing at storyboard time `t`, and how far into it.
/// Before 0 → step 0; at or past the end → the last step (clamped).
pub fn locate(steps: &[Step], t: f64) -> (usize, f64) {
    let mut start = 0.0;
    for (i, s) in steps.iter().enumerate() {
        if t < start + s.secs || i == steps.len() - 1 {
            return (i, (t - start).max(0.0).min(s.secs));
        }
        start += s.secs;
    }
    unreachable!("storyboard is never empty")
}

/// Where the show continues after the finale: the logo exit.
pub fn loop_target(steps: &[Step]) -> usize {
    steps.iter().position(|s| s.kind == StepKind::LogoExit).expect("storyboard has a logo exit")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seek {
    Next,
    Prev,
    NextBlock,
    PrevBlock,
    Block(usize),
}

fn first_of(steps: &[Step], block: usize) -> Option<usize> {
    steps.iter().position(|s| s.block == block)
}

/// The step a seek request lands on, from step `current`.
pub fn seek_target(steps: &[Step], current: usize, seek: Seek) -> usize {
    let block = steps[current].block;
    match seek {
        Seek::Next if current + 1 < steps.len() => current + 1,
        Seek::Next => loop_target(steps),
        Seek::Prev => current.saturating_sub(1),
        Seek::NextBlock => first_of(steps, block + 1).unwrap_or_else(|| loop_target(steps)),
        Seek::PrevBlock => first_of(steps, block.saturating_sub(1).max(1)).unwrap_or(0),
        Seek::Block(n) => first_of(steps, n.clamp(1, LAST_BLOCK)).unwrap_or(0),
    }
}

/// Maps wall time to storyboard time. Seeking moves the origin; running
/// past the finale moves it back to the loop target.
#[derive(Debug)]
pub struct Clock {
    origin: f64,
}

impl Clock {
    pub fn new() -> Self {
        Self { origin: 0.0 }
    }

    /// A clock that reads storyboard time `t` at wall time 0.
    #[cfg(test)]
    pub fn at(t: f64) -> Self {
        Self { origin: -t }
    }

    /// Storyboard time at wall time `now` (seconds), after applying `seek`.
    pub fn time(&mut self, steps: &[Step], now: f64, seek: Option<Seek>) -> f64 {
        if let Some(seek) = seek {
            let (current, _) = locate(steps, now - self.origin);
            self.origin = now - start_of(steps, seek_target(steps, current, seek));
        }
        let (end, back) = (total(steps), start_of(steps, loop_target(steps)));
        while now - self.origin >= end {
            self.origin += end - back;
        }
        now - self.origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_of_block(steps: &[Step], block: usize) -> usize {
        steps.iter().position(|s| s.block == block).unwrap()
    }

    #[test]
    fn storyboard_shape() {
        let s = storyboard();
        assert_eq!(s[0].kind, StepKind::Intro);
        assert_eq!(s[1].kind, StepKind::LogoExit);
        assert_eq!(s.last().unwrap().kind, StepKind::Finale);
        let panels = |b| s.iter().filter(|st| st.block == b && matches!(st.kind, StepKind::Panel { .. })).count();
        assert_eq!((panels(2), panels(3), panels(4), panels(5)), (7, 9, 2, 3));
        assert!(s.windows(2).all(|w| w[0].block <= w[1].block));
    }

    #[test]
    fn captions_use_only_c64_glyphs() {
        for st in storyboard() {
            let text = match st.kind {
                StepKind::Heading { title } => title,
                StepKind::Panel { subtitle, .. } => subtitle,
                _ => continue,
            };
            for ch in text.chars() {
                assert!(crate::glyphs::has_c64(ch), "{text:?} needs {ch:?}");
            }
        }
    }

    #[test]
    fn locate_boundaries() {
        let s = storyboard();
        assert_eq!(locate(&s, 0.0), (0, 0.0));
        assert_eq!(locate(&s, start_of(&s, 1)), (1, 0.0));
        let (i, local) = locate(&s, start_of(&s, 1) - 0.001);
        assert_eq!(i, 0);
        assert!((local - (s[0].secs - 0.001)).abs() < 1e-9);
        assert_eq!(locate(&s, total(&s) + 5.0).0, s.len() - 1);
    }

    #[test]
    fn next_and_prev() {
        let s = storyboard();
        assert_eq!(seek_target(&s, 3, Seek::Next), 4);
        assert_eq!(seek_target(&s, 3, Seek::Prev), 2);
    }

    #[test]
    fn prev_at_start_stays() {
        assert_eq!(seek_target(&storyboard(), 0, Seek::Prev), 0);
    }

    #[test]
    fn next_on_finale_loops() {
        let s = storyboard();
        assert_eq!(seek_target(&s, s.len() - 1, Seek::Next), loop_target(&s));
        assert_eq!(s[loop_target(&s)].kind, StepKind::LogoExit);
    }

    #[test]
    fn block_seeks() {
        let s = storyboard();
        let b3 = first_of_block(&s, 3);
        assert_eq!(seek_target(&s, 0, Seek::NextBlock), first_of_block(&s, 2));
        assert_eq!(seek_target(&s, b3 + 2, Seek::PrevBlock), first_of_block(&s, 2));
        assert_eq!(seek_target(&s, 0, Seek::Block(4)), first_of_block(&s, 4));
        assert_eq!(seek_target(&s, s.len() - 1, Seek::NextBlock), loop_target(&s));
    }

    #[test]
    fn block_jump_clamps() {
        let s = storyboard();
        assert_eq!(seek_target(&s, 5, Seek::Block(0)), 0);
        assert_eq!(seek_target(&s, 5, Seek::Block(9)), first_of_block(&s, 6));
    }

    #[test]
    fn clock_loops_after_finale() {
        let s = storyboard();
        let mut clock = Clock::new();
        let t = clock.time(&s, total(&s) + 0.5, None);
        assert!((t - (start_of(&s, loop_target(&s)) + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn clock_at_reads_t_at_zero() {
        let s = storyboard();
        assert!((Clock::at(20.0).time(&s, 0.0, None) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn clock_applies_seek_once() {
        let s = storyboard();
        let mut clock = Clock::new();
        let b3 = start_of(&s, first_of_block(&s, 3));
        assert!((clock.time(&s, 10.0, Some(Seek::Block(3))) - b3).abs() < 1e-9);
        // One second later, no new request: time runs on from the new origin.
        assert!((clock.time(&s, 11.0, None) - (b3 + 1.0)).abs() < 1e-9);
    }
}
