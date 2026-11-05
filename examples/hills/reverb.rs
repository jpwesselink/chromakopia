//! A stereo hall: Freeverb (Jezar's public-domain Schroeder–Moorer
//! design) — eight damped combs into four allpasses per side, the right
//! side's delays a little longer for width.

/// Comb and allpass lengths at 44.1 kHz, and the right side's extra.
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
/// Input level into the combs.
const GAIN: f32 = 0.015;

struct Comb {
    buf: Vec<f32>,
    at: usize,
    store: f32,
}

impl Comb {
    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.at];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.at] = x + self.store * feedback;
        self.at = (self.at + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    at: usize,
}

impl Allpass {
    fn run(&mut self, x: f32) -> f32 {
        let held = self.buf[self.at];
        self.buf[self.at] = x + held * 0.5;
        self.at = (self.at + 1) % self.buf.len();
        held - x
    }
}

struct Side {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Side {
    fn new(scale: f32, extra: usize) -> Self {
        let len = |n: usize| (((n + extra) as f32 * scale) as usize).max(1);
        Side {
            combs: COMBS.iter().map(|&n| Comb { buf: vec![0.0; len(n)], at: 0, store: 0.0 }).collect(),
            allpasses: ALLPASSES.iter().map(|&n| Allpass { buf: vec![0.0; len(n)], at: 0 }).collect(),
        }
    }

    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let wet: f32 = self.combs.iter_mut().map(|c| c.run(x * GAIN, feedback, damp)).sum();
        self.allpasses.iter_mut().fold(wet, |acc, a| a.run(acc))
    }
}

pub struct Reverb {
    left: Side,
    right: Side,
    feedback: f32,
    damp: f32,
}

impl Reverb {
    /// A hall at `sample_rate`: `room` 0–1 for how long it rings, `damp`
    /// 0–1 for how dark its tail is.
    pub fn new(sample_rate: u32, room: f32, damp: f32) -> Self {
        let scale = sample_rate as f32 / 44_100.0;
        Reverb { left: Side::new(scale, 0), right: Side::new(scale, SPREAD), feedback: 0.7 + 0.28 * room, damp: damp * 0.4 }
    }

    /// The hall's answer to a mono `send`, left and right.
    pub fn run(&mut self, send: f32) -> (f32, f32) {
        (self.left.run(send, self.feedback, self.damp), self.right.run(send, self.feedback, self.damp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_impulse_rings_on_in_stereo_and_dies_away() {
        let sr = 22_050;
        let mut hall = Reverb::new(sr, 0.85, 0.5);
        let out: Vec<(f32, f32)> = (0..sr as usize * 6).map(|i| hall.run(if i == 0 { 1.0 } else { 0.0 })).collect();
        let level = |a: f32, b: f32| out[(a * sr as f32) as usize..(b * sr as f32) as usize].iter().map(|s| s.0.abs()).fold(0.0, f32::max);
        assert!(out.iter().all(|s| s.0.is_finite() && s.1.is_finite()));
        // Still ringing after a second, quieter by five.
        assert!(level(1.0, 1.5) > 1e-4, "no tail: {}", level(1.0, 1.5));
        assert!(level(5.0, 6.0) < level(0.1, 0.6) / 10.0, "doesn't die away");
        // Left and right differ: width.
        assert!(out.iter().any(|s| (s.0 - s.1).abs() > 1e-4));
    }
}
