//! An 808, modelled the analog way: the kick a sine diving onto its low
//! note with a long boom, the snare two tuned sines and snappy noise, the
//! hats and cymbals six square waves at the 808's own metallic pitches
//! through a band- and a high-pass — a closed hat chokes an open one.
//! It plays the set's 606 part (`orbital3.rs` track 3).

use std::f32::consts::TAU;

/// The 808 cymbal and hat oscillators, Hz.
const METAL: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

/// A drum, by the 606 pad its notes came in on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drum {
    Kick,
    Snare,
    Tom,
    ClosedHat,
    OpenHat,
    Cymbal,
}

impl Drum {
    /// The drum on 606 pad `key` (36 kick, 38 snare, 44 hats, 47 tom, 50
    /// and 51 cymbals).
    pub fn on(key: u8) -> Option<Drum> {
        Some(match key {
            36 => Drum::Kick,
            38 => Drum::Snare,
            44 => Drum::ClosedHat,
            47 => Drum::Tom,
            50 => Drum::OpenHat,
            51 => Drum::Cymbal,
            _ => return None,
        })
    }
}

/// A simple RBJ biquad.
#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn new(sr: f32, hz: f32, q: f32, band: bool) -> Self {
        let w = TAU * hz / sr;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b = if band { [alpha, 0.0, -alpha] } else { [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0] };
        Biquad { b: b.map(|v| v / a0), a: [-2.0 * c / a0, (1.0 - alpha) / a0], ..Default::default() }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// One hit, sounding.
#[derive(Debug, Clone)]
struct Hit {
    drum: Drum,
    vel: f32,
    /// Samples since it was struck.
    age: u32,
    phase: [f32; 6],
    band: Biquad,
    high: Biquad,
}

impl Hit {
    fn next(&mut self, sr: f32, noise: f32) -> Option<f32> {
        let t = self.age as f32 / sr;
        self.age += 1;
        let mut osc = |i: usize, hz: f32| {
            self.phase[i] = (self.phase[i] + hz / sr).fract();
            self.phase[i]
        };
        let out = match self.drum {
            Drum::Kick => {
                if t > 1.2 {
                    return None;
                }
                let hz = 52.0 * (1.0 + 1.3 * (-t / 0.025).exp());
                let body = (TAU * osc(0, hz)).sin() * (-t / 0.45).exp();
                let click = noise * (-t / 0.002).exp() * 0.3;
                (1.6 * (body + click)).tanh()
            }
            Drum::Snare => {
                if t > 0.5 {
                    return None;
                }
                let tone = 0.5 * (TAU * osc(0, 180.0)).sin() * (-t / 0.09).exp() + 0.3 * (TAU * osc(1, 330.0)).sin() * (-t / 0.06).exp();
                let snap = self.high.run(noise) * (-t / 0.16).exp();
                tone + 0.8 * snap
            }
            Drum::Tom => {
                if t > 0.8 {
                    return None;
                }
                (TAU * osc(0, 160.0 * (1.0 + 0.3 * (-t / 0.05).exp()))).sin() * (-t / 0.28).exp()
            }
            Drum::ClosedHat | Drum::OpenHat | Drum::Cymbal => {
                let decay = match self.drum {
                    Drum::ClosedHat => 0.045,
                    Drum::OpenHat => 0.28,
                    _ => 0.7,
                };
                if t > decay * 6.0 {
                    return None;
                }
                let metal: f32 = (0..6).map(|i| if osc(i, METAL[i]) < 0.5 { 1.0 } else { -1.0 }).sum::<f32>() / 6.0;
                self.high.run(self.band.run(metal)) * (-t / decay).exp() * 1.6
            }
        };
        Some(out * self.vel)
    }
}

/// The kit: every hit still sounding, mixed.
pub struct Kit {
    sr: f32,
    hits: Vec<Hit>,
    seed: u32,
}

impl Kit {
    pub fn new(sample_rate: u32) -> Self {
        Kit { sr: sample_rate as f32, hits: Vec::new(), seed: 0x2545_F491 }
    }

    /// Strike `drum` at velocity `vel` (0–127).
    pub fn strike(&mut self, drum: Drum, vel: u8) {
        // A closed hat chokes an open one, as on the real thing.
        if drum == Drum::ClosedHat {
            self.hits.retain(|h| h.drum != Drum::OpenHat);
        }
        let sr = self.sr;
        let (band, high) = match drum {
            Drum::Snare => (Biquad::default(), Biquad::new(sr, 1_800.0, 0.7, false)),
            Drum::ClosedHat | Drum::OpenHat => (Biquad::new(sr, 10_000.0, 0.9, true), Biquad::new(sr, 7_000.0, 0.7, false)),
            Drum::Cymbal => (Biquad::new(sr, 8_000.0, 0.7, true), Biquad::new(sr, 5_000.0, 0.7, false)),
            _ => (Biquad::default(), Biquad::default()),
        };
        self.hits.push(Hit { drum, vel: vel as f32 / 127.0, age: 0, phase: [0.0; 6], band, high });
    }

    /// The next sample.
    pub fn next(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        let noise = self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let sr = self.sr;
        let mut sum = 0.0;
        self.hits.retain_mut(|h| match h.next(sr, noise) {
            Some(s) => {
                sum += s;
                true
            }
            None => false,
        });
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    fn hit(drum: Drum, secs: f32) -> Vec<f32> {
        let mut kit = Kit::new(SR);
        kit.strike(drum, 127);
        (0..(secs * SR as f32) as usize).map(|_| kit.next()).collect()
    }

    /// Zero crossings a second: a rough pitch.
    fn crossings(s: &[f32]) -> f32 {
        s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f32 / (s.len() as f32 / SR as f32)
    }

    #[test]
    fn every_606_pad_has_its_808() {
        for key in [36, 38, 44, 47, 50, 51] {
            let drum = Drum::on(key).unwrap();
            let s = hit(drum, 0.2);
            assert!(s.iter().all(|x| x.is_finite() && x.abs() < 2.0), "{drum:?} blows up");
            assert!(s.iter().any(|x| x.abs() > 0.05), "{drum:?} is silent");
        }
    }

    #[test]
    fn the_kick_booms_low_and_long() {
        let s = hit(Drum::Kick, 1.0);
        // Settles near 52 Hz: about 104 crossings a second.
        let tail = &s[(0.2 * SR as f32) as usize..(0.6 * SR as f32) as usize];
        assert!((90.0..120.0).contains(&crossings(tail)), "{}", crossings(tail));
        assert!(tail.iter().any(|x| x.abs() > 0.2), "no boom left by 0.2 s");
    }

    #[test]
    fn hats_are_high_and_short_and_closed_chokes_open() {
        let s = hit(Drum::ClosedHat, 0.3);
        assert!(crossings(&s[..2_000]) > 6_000.0, "{}", crossings(&s[..2_000]));
        assert!(s[(0.25 * SR as f32) as usize..].iter().all(|x| x.abs() < 0.01), "closed hat rings on");
        let mut kit = Kit::new(SR);
        kit.strike(Drum::OpenHat, 127);
        kit.strike(Drum::ClosedHat, 127);
        assert_eq!(kit.hits.iter().filter(|h| h.drum == Drum::OpenHat).count(), 0);
    }
}
