//! The demo's music: "orbital 3", played from its notes (see
//! `orbital3.rs`, generated from the Ableton set) by a small chip synth —
//! pulse pad, FM bells, acid saws through a resonant filter, a sub thud,
//! and drums out of sine sweeps and noise. A few kilobytes of notes
//! instead of megabytes of audio, demoscene style.

use std::f32::consts::TAU;

use super::orbital3::{BEATS, BPM, NOTES, TRACKS};

/// One note of the arrangement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    /// Beats from the start of the song.
    pub at: f64,
    /// Beats it is held.
    pub len: f64,
    /// Index into `TRACKS`.
    pub track: u8,
    /// MIDI key.
    pub key: u8,
    /// MIDI velocity, 0–127.
    pub vel: u8,
}

/// What a track is played with.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Instrument {
    Pad,
    FmBell,
    Acid,
    Drums,
    Mallet,
    Thud,
}

/// Each track's instrument, the semitones it sounds above its notes, its
/// level, and whether it goes through the echo. Tracks not here (the
/// marimbas, which the arrangement never plays) stay silent.
fn voicing(track: u8) -> Option<(Instrument, i32, f32, bool)> {
    Some(match track {
        0 => (Instrument::Pad, 0, 0.16, false),
        // Operator: transposed -4, carrier at twice the pitch.
        1 => (Instrument::FmBell, 8, 0.9, true),
        2 => (Instrument::Acid, 0, 0.55, false),
        3 => (Instrument::Drums, 0, 0.45, false),
        4 => (Instrument::Mallet, 12, 0.35, true),
        6 => (Instrument::Acid, 0, 0.9, false),
        7 => (Instrument::Thud, 0, 0.55, false),
        _ => return None,
    })
}

/// Seconds of echo, and how much comes back each time.
const ECHO_SECS: f32 = 0.375;
const ECHO_FEEDBACK: f32 = 0.35;
const ECHO_WET: f32 = 0.35;
/// Most notes sounding at once; the oldest makes way.
const VOICES: usize = 48;

fn hz(key: i32) -> f32 {
    440.0 * 2f32.powf((key - 69) as f32 / 12.0)
}

/// A sounding note.
#[derive(Debug, Clone)]
struct Voice {
    instrument: Instrument,
    key: u8,
    freq: f32,
    vel: f32,
    gain: f32,
    echo: bool,
    /// Samples since it started, and samples it is held.
    age: u32,
    gate: u32,
    phase: [f32; 2],
    /// Filter state: low and band.
    low: f32,
    band: f32,
}

impl Voice {
    /// Its next sample, or None once it has died away.
    fn next(&mut self, sr: f32, noise: f32) -> Option<f32> {
        let t = self.age as f32 / sr;
        let held = self.gate as f32 / sr;
        let released = (t - held).max(0.0);
        self.age += 1;
        let step = |phase: &mut f32, f: f32| {
            *phase = (*phase + f / sr).fract();
            *phase
        };
        let out = match self.instrument {
            Instrument::Pad => {
                if released > 3.0 {
                    return None;
                }
                let duty = 0.5 + 0.2 * (TAU * 0.3 * t).sin();
                let pulse = |p: f32| if p < duty { 1.0 } else { -1.0 };
                let a = pulse(step(&mut self.phase[0], self.freq));
                let b = pulse(step(&mut self.phase[1], self.freq * 1.004));
                self.low += (0.5 * (a + b) - self.low) * 0.08;
                let env = (t / 0.8).min(1.0) * (-released / 0.6).exp();
                self.low * env
            }
            Instrument::FmBell => {
                if t > 4.0 {
                    return None;
                }
                let m = (TAU * step(&mut self.phase[1], self.freq * 3.5)).sin() * 2.5 * (-t / 0.25).exp();
                (TAU * step(&mut self.phase[0], self.freq) + m).sin() * (-t / 0.9).exp()
            }
            Instrument::Acid => {
                if released > 0.08 {
                    return None;
                }
                let saw = 2.0 * step(&mut self.phase[0], self.freq) - 1.0;
                let cutoff = 180.0 + 2400.0 * (-t / 0.15).exp() * self.vel;
                let f = 2.0 * (std::f32::consts::PI * cutoff / sr).sin();
                self.low += f * self.band;
                let high = saw - self.low - 0.25 * self.band;
                self.band += f * high;
                self.low * (-released / 0.01).exp()
            }
            Instrument::Mallet => {
                if t > 3.0 {
                    return None;
                }
                let tri = 4.0 * (step(&mut self.phase[0], self.freq) - 0.5).abs() - 1.0;
                tri * (-t / 0.5).exp()
            }
            Instrument::Thud => {
                if released > 0.1 {
                    return None;
                }
                let f = self.freq * (1.0 + 0.5 * (-t / 0.02).exp());
                let s = (TAU * step(&mut self.phase[0], f)).sin();
                (2.0 * s).tanh() * (-t / 0.5).exp() * (-released / 0.05).exp()
            }
            Instrument::Drums => {
                // High-passed noise for the metal.
                self.low += (noise - self.low) * 0.3;
                let hiss = noise - self.low;
                let (sound, length) = match self.key {
                    36 => ((TAU * step(&mut self.phase[0], 45.0 + 140.0 * (-t / 0.025).exp())).sin() * (-t / 0.3).exp(), 1.0),
                    38 => (0.55 * noise * (-t / 0.13).exp() + 0.45 * (TAU * step(&mut self.phase[0], 190.0)).sin() * (-t / 0.06).exp(), 0.6),
                    44 => (hiss * (-t / 0.06).exp(), 0.4),
                    47 => ((TAU * step(&mut self.phase[0], 200.0 + 80.0 * (-t / 0.04).exp())).sin() * (-t / 0.25).exp(), 1.0),
                    50 => (0.5 * hiss * (-t / 0.4).exp(), 2.0),
                    51 => (0.5 * hiss * (-t / 0.12).exp(), 1.0),
                    _ => (0.0, 0.0),
                };
                if t > length {
                    return None;
                }
                // A little bit-crush: five bits, as a sampler chip would.
                (sound * 16.0).round() / 16.0
            }
        };
        Some(out * self.vel * self.gain)
    }
}

/// Plays the song from its notes, sample by sample, round and round.
pub struct Synth {
    sr: f32,
    /// Samples since the song (re)started.
    pos: u64,
    /// Next note to start.
    next: usize,
    voices: Vec<Voice>,
    echo: Vec<f32>,
    echo_at: usize,
    seed: u32,
    /// Play only this track (tests and balancing).
    solo: Option<u8>,
}

impl Synth {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        Synth {
            sr,
            pos: 0,
            next: 0,
            voices: Vec::new(),
            echo: vec![0.0; (ECHO_SECS * sr) as usize],
            echo_at: 0,
            seed: 0x9E37_79B9,
            solo: None,
        }
    }

    #[cfg(test)]
    fn solo(mut self, track: u8) -> Self {
        self.solo = Some(track);
        self
    }

    /// Samples in one beat.
    fn beat(&self) -> f64 {
        self.sr as f64 * 60.0 / BPM
    }

    fn noise(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn start(&mut self, note: Note) {
        if self.solo.is_some_and(|s| s != note.track) {
            return;
        }
        let Some((instrument, shift, gain, echo)) = voicing(note.track) else { return };
        if self.voices.len() >= VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice {
            instrument,
            key: note.key,
            freq: hz(note.key as i32 + shift),
            vel: note.vel as f32 / 127.0,
            gain: gain * TRACKS[note.track as usize].1,
            echo,
            age: 0,
            gate: (note.len * self.beat()) as u32,
            phase: [0.0; 2],
            low: 0.0,
            band: 0.0,
        });
    }

    /// The next sample, -1..1.
    pub fn next(&mut self) -> f32 {
        let beat = self.pos as f64 / self.beat();
        if beat >= BEATS {
            self.pos = 0;
            self.next = 0;
        }
        let beat = self.pos as f64 / self.beat();
        while self.next < NOTES.len() && NOTES[self.next].at <= beat {
            self.start(NOTES[self.next]);
            self.next += 1;
        }
        self.pos += 1;

        let noise = self.noise();
        let (mut dry, mut wet) = (0.0, 0.0);
        let sr = self.sr;
        self.voices.retain_mut(|v| match v.next(sr, noise) {
            Some(s) => {
                if v.echo {
                    wet += s;
                }
                dry += s;
                true
            }
            None => false,
        });
        let echoed = self.echo[self.echo_at];
        self.echo[self.echo_at] = wet + echoed * ECHO_FEEDBACK;
        self.echo_at = (self.echo_at + 1) % self.echo.len();
        (0.8 * (dry + ECHO_WET * echoed)).tanh()
    }
}

/// Start the music on the default output device; it plays as long as the
/// returned stream lives. None when there is no sound to be had (no
/// device, an unusual sample format) — the demo just runs silent.
pub fn play() -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;
    let mut synth = Synth::new(config.sample_rate.0);
    let quiet = |_| {};
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    frame.fill(synth.next());
                }
            },
            quiet,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    frame.fill((synth.next() * i16::MAX as f32) as i16);
                }
            },
            quiet,
            None,
        ),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 22_050;

    /// `secs` of `synth`, from `from` seconds in.
    fn render(mut synth: Synth, from: f64, secs: f64) -> Vec<f32> {
        for _ in 0..(from * SR as f64) as usize {
            synth.next();
        }
        (0..(secs * SR as f64) as usize).map(|_| synth.next()).collect()
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    }

    #[test]
    fn notes_are_in_order_and_in_the_song() {
        assert!(NOTES.windows(2).all(|w| w[0].at <= w[1].at));
        assert!(NOTES.iter().all(|n| n.at >= 0.0 && n.at < BEATS && n.len > 0.0 && (n.track as usize) < TRACKS.len()));
        // Every track the arrangement plays has a voice.
        for n in NOTES {
            assert!(voicing(n.track).is_some(), "track {} ({}) unvoiced", n.track, TRACKS[n.track as usize].0);
        }
    }

    #[test]
    fn plays_cleanly() {
        // From the start, where it's drums and bass, and the busy middle.
        for from in [0.0, 70.0] {
            let s = render(Synth::new(SR), from, 4.0);
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(rms(&s) > 0.02, "silent from {from}s: {}", rms(&s));
        }
    }

    #[test]
    fn every_track_sounds_when_it_plays() {
        for (track, (name, _)) in TRACKS.iter().enumerate() {
            let Some(first) = NOTES.iter().find(|n| n.track as usize == track) else { continue };
            let from = first.at * 60.0 / BPM;
            let s = render(Synth::new(SR).solo(track as u8), from, 2.0);
            assert!(rms(&s) > 0.005, "{name} is silent: {}", rms(&s));
        }
    }

    #[test]
    fn voices_die_away_and_the_song_starts_over() {
        let mut synth = Synth::new(8_000);
        let song = (BEATS * 60.0 / BPM * 8_000.0) as usize;
        let mut most = 0;
        for _ in 0..song + 8_000 {
            synth.next();
            most = most.max(synth.voices.len());
        }
        assert!(most <= VOICES);
        // A second in to the second time round: back near the start.
        assert!(synth.next < 20, "at note {}", synth.next);
    }

    /// The whole song as a WAV to listen to without the demo:
    /// `cargo test --release --example hills render_song -- --ignored`,
    /// then open `target/orbital3.wav`. Prints each track's level.
    #[test]
    #[ignore]
    fn render_song() {
        let sr = 44_100;
        for (track, (name, _)) in TRACKS.iter().enumerate() {
            if NOTES.iter().any(|n| n.track as usize == track) {
                let mut synth = Synth::new(sr).solo(track as u8);
                let s: Vec<f32> = (0..(BEATS * 60.0 / BPM) as usize * sr as usize).map(|_| synth.next()).collect();
                let peak = s.iter().fold(0f32, |a, x| a.max(x.abs()));
                println!("{name:>18}: rms {:.3}  peak {peak:.2}", rms(&s));
            }
        }
        let mut synth = Synth::new(sr);
        let samples: Vec<i16> = (0..(BEATS * 60.0 / BPM) as usize * sr as usize).map(|_| (synth.next() * 32_000.0) as i16).collect();
        let mut wav = Vec::new();
        let data = samples.len() as u32 * 2;
        wav.extend(b"RIFF");
        wav.extend((36 + data).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(sr.to_le_bytes());
        wav.extend((sr * 2).to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(data.to_le_bytes());
        for s in samples {
            wav.extend(s.to_le_bytes());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/orbital3.wav");
        std::fs::write(&path, wav).unwrap();
        println!("wrote {}", path.display());
    }
}
