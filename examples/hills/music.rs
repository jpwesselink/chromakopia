//! The demo's music: "orbital 3", played from its notes (see
//! `orbital3.rs`, generated from the Ableton set) on three emulated C64
//! SIDs — MOS 6581s by reSID — the way a C64 music routine does it: a
//! player writes the chips' registers 200 times a second. Nine voices,
//! three filters ("3SID"), and the 606 part on an 808 (`drums.rs`):
//!
//! | chip | voice 1 | voice 2 | voice 3 | filter |
//! |---|---|---|---|---|
//! | A, centre | 303 | — | thud bass | the 303's acid |
//! | B, left | second 303, quieter | bells, arpeggiated | mallets | the second 303's |
//! | C, right | pad, low note | pad, the rest | the mallets' echo | — |

use resid::{ChipModel, SamplingMethod, Sid};

use super::drums::{Drum, Kit};
use super::orbital3::{BEATS, BPM, NOTES};

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

/// A PAL C64's clock, cycles a second.
const CLOCK: u32 = 985_248;
/// Player calls a second ("4× speed"): a 16th at 120 BPM is 25 of them.
const TICKS: u32 = 200;
/// Ticks between arpeggio steps — 50 Hz, the classic rate.
const ARP: u64 = 4;
/// How many SIDs.
const CHIPS: usize = 3;
/// Each chip's share of the left and right channel.
const PAN: [(f32, f32); CHIPS] = [(0.5, 0.5), (0.65, 0.35), (0.35, 0.65)];
/// The 808's level next to the chips (it sits in the middle).
const KIT: f32 = 0.35;

// The tracks, by index into `TRACKS`.
const PAD: u8 = 0;
const BELLS: u8 = 1;
const ACID: u8 = 2;
const DRUMS: u8 = 3;
const MALLETS: u8 = 4;
const ACID_HIGH: u8 = 6;
const THUD: u8 = 7;
/// Tracks left out of the mix: both bell leads (and so the mallets'
/// echo). Their voices stay wired — take a track off to bring it back.
const MUTED: [u8; 2] = [BELLS, MALLETS];
/// Not a track: the mallets again, later and quieter.
const ECHO: u8 = 100;
/// Ticks the echo comes after: a dotted eighth.
const ECHO_DELAY: u64 = 75;

// SID registers: voice `v`'s are at 7·v.
const FREQ: u8 = 0;
const PW: u8 = 2;
const CTRL: u8 = 4;
const AD: u8 = 5;
const SR: u8 = 6;
const FC_LO: u8 = 0x15;
const FC_HI: u8 = 0x16;
const RES_FILT: u8 = 0x17;
const MODE_VOL: u8 = 0x18;
// Control bits.
const GATE: u8 = 0x01;
const TEST: u8 = 0x08;
const TRI: u8 = 0x10;
const SAW: u8 = 0x20;
const PULSE: u8 = 0x40;

/// A voice: (chip, voice on that chip).
type Slot = (usize, u8);
const A1: Slot = (0, 0);
const A3: Slot = (0, 2);
const B1: Slot = (1, 0);
const B2: Slot = (1, 1);
const B3: Slot = (1, 2);
const C1: Slot = (2, 0);
const C2: Slot = (2, 1);
const C3: Slot = (2, 2);

/// The SID's frequency register for MIDI `key`.
fn freq(key: f64) -> u16 {
    let hz = 440.0 * 2f64.powf((key - 69.0) / 12.0);
    (hz * 16_777_216.0 / CLOCK as f64).round().min(65_535.0) as u16
}

/// Something with SID registers: the chips, or a test's notebook.
pub trait Regs {
    fn poke(&mut self, chip: usize, reg: u8, value: u8);
}

impl Regs for [Sid; CHIPS] {
    fn poke(&mut self, chip: usize, reg: u8, value: u8) {
        self[chip].write(reg, value);
    }
}

/// A note sounding, from tick `start` to `end`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    note: Note,
    start: u64,
    end: u64,
}

/// What a voice is playing: which note (for retriggering) and since when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Playing {
    track: u8,
    key: u8,
    start: u64,
}

/// A voice's sound: waveform, attack/decay, sustain/release.
#[derive(Debug, Clone, Copy)]
struct Patch {
    wave: u8,
    ad: u8,
    sr: u8,
}

/// The music routine: works out which notes sound and writes the SIDs'
/// registers for them, one tick at a time.
pub struct Player {
    tick: u64,
    /// Next note to start.
    next: usize,
    held: Vec<Held>,
    /// Echo notes waiting for their turn.
    later: Vec<Held>,
    voices: [[Option<Playing>; 3]; CHIPS],
    /// Drums struck this tick, for the 808.
    pub strikes: Vec<(Drum, u8)>,
    /// Play only this track (tests and balancing).
    solo: Option<u8>,
}

/// Ticks in one beat.
fn per_beat() -> f64 {
    TICKS as f64 * 60.0 / BPM
}

fn ticks(beats: f64) -> u64 {
    (beats * per_beat()).round() as u64
}

impl Player {
    pub fn new() -> Self {
        Player { tick: 0, next: 0, held: Vec::new(), later: Vec::new(), voices: [[None; 3]; CHIPS], strikes: Vec::new(), solo: None }
    }

    /// Ticks in the whole song.
    pub fn length() -> u64 {
        ticks(BEATS)
    }

    /// Advance one tick, writing this tick's registers.
    pub fn tick(&mut self, sid: &mut impl Regs) {
        if self.tick >= Self::length() {
            *self = Player { solo: self.solo, ..Player::new() };
        }
        let now = self.tick;
        if now == 0 {
            for chip in 0..CHIPS {
                // Volume up, low-pass, nothing filtered yet.
                sid.poke(chip, MODE_VOL, 0x1F);
                sid.poke(chip, RES_FILT, 0x00);
            }
        }
        while self.next < NOTES.len() && ticks(NOTES[self.next].at) <= now {
            let note = NOTES[self.next];
            self.next += 1;
            // Muted tracks stay quiet — unless soloed, to hear them anyway.
            let heard = match self.solo {
                Some(s) => s == note.track,
                None => !MUTED.contains(&note.track),
            };
            if !heard {
                continue;
            }
            if note.track == DRUMS {
                self.strikes.extend(Drum::on(note.key).map(|d| (d, note.vel)));
                continue;
            }
            let held = Held { note, start: now, end: ticks(note.at + note.len).max(now + 1) };
            self.held.push(held);
            if note.track == MALLETS {
                let echo = Note { track: ECHO, ..note };
                self.later.push(Held { note: echo, start: now + ECHO_DELAY, end: held.end + ECHO_DELAY });
            }
        }
        let (due, wait): (Vec<Held>, Vec<Held>) = self.later.iter().partition(|h| h.start <= now);
        self.later = wait;
        self.held.extend(due);
        self.held.retain(|h| h.end > now);

        self.acid(sid, A1, ACID, 1.0, Patch { wave: SAW, ad: 0x08, sr: 0xA0 }, now);
        // The second 303: quieter — a lower sustain, its filter opening
        // less far.
        self.acid(sid, B1, ACID_HIGH, 0.45, Patch { wave: SAW, ad: 0x06, sr: 0x50 }, now);
        let keys = |track: u8, shift: u8| -> Vec<(u8, u64)> { self.keys(track, shift) };
        let (thud, bells, mallets, pad, echo) = (keys(THUD, 0), keys(BELLS, 20), keys(MALLETS, 12), keys(PAD, 12), keys(ECHO, 12));
        self.tone(sid, A3, &thud, Patch { wave: TRI, ad: 0x09, sr: 0x00 }, now);
        self.tone(sid, B2, &bells, Patch { wave: PULSE, ad: 0x09, sr: 0x00 }, now);
        self.tone(sid, B3, &mallets, Patch { wave: TRI, ad: 0x0A, sr: 0x00 }, now);
        // The pad's lowest note on one voice, the rest on the next.
        let (low, rest) = pad.split_at(pad.len().min(1));
        self.tone(sid, C1, low, Patch { wave: PULSE, ad: 0x8A, sr: 0xA9 }, now);
        self.tone(sid, C2, rest, Patch { wave: PULSE, ad: 0x8A, sr: 0xA9 }, now);
        self.tone(sid, C3, &echo, Patch { wave: TRI, ad: 0x0A, sr: 0x00 }, now);
        self.tick += 1;
    }

    /// `track`'s sounding keys, `shift` semitones up, lowest first, each
    /// with the tick it started.
    fn keys(&self, track: u8, shift: u8) -> Vec<(u8, u64)> {
        let mut keys: Vec<(u8, u64)> = self.held.iter().filter(|h| h.note.track == track).map(|h| (h.note.key + shift, h.start)).collect();
        keys.sort_unstable();
        keys.dedup_by_key(|k| k.0);
        keys
    }

    /// The latest-started held note of `track`.
    fn latest(&self, track: u8) -> Option<Held> {
        self.held.iter().filter(|h| h.note.track == track).max_by_key(|h| h.start).copied()
    }

    /// Start a note on `slot` if it isn't already playing it: one tick of
    /// hard restart (gate off, oscillator held in test, envelope zeroed),
    /// the note itself from the next. True on the restart tick.
    fn retrigger(&mut self, sid: &mut impl Regs, (chip, v): Slot, playing: Playing) -> bool {
        if self.voices[chip][v as usize] == Some(playing) {
            return false;
        }
        self.voices[chip][v as usize] = Some(playing);
        sid.poke(chip, 7 * v + AD, 0x00);
        sid.poke(chip, 7 * v + SR, 0x00);
        sid.poke(chip, 7 * v + CTRL, TEST);
        true
    }

    fn release(&mut self, sid: &mut impl Regs, (chip, v): Slot, wave: u8) {
        if self.voices[chip][v as usize].take().is_some() {
            sid.poke(chip, 7 * v + CTRL, wave);
        }
    }

    fn sound(sid: &mut impl Regs, (chip, v): Slot, key: f64, patch: Patch, gate: bool) {
        let base = 7 * v;
        let f = freq(key);
        sid.poke(chip, base + FREQ, f as u8);
        sid.poke(chip, base + FREQ + 1, (f >> 8) as u8);
        sid.poke(chip, base + AD, patch.ad);
        sid.poke(chip, base + SR, patch.sr);
        sid.poke(chip, base + CTRL, patch.wave | if gate { GATE } else { 0 });
    }

    /// A 303 on `slot` through its chip's resonant low-pass, the cutoff
    /// snapping open `open` far with each note and closing again.
    fn acid(&mut self, sid: &mut impl Regs, slot: Slot, track: u8, open: f64, patch: Patch, now: u64) {
        let chip = slot.0;
        let Some(h) = self.latest(track) else {
            self.release(sid, slot, patch.wave);
            return;
        };
        if self.retrigger(sid, slot, Playing { track, key: h.note.key, start: h.start }) {
            return;
        }
        Self::sound(sid, slot, h.note.key as f64, patch, true);
        let since = (now - h.start) as f64;
        let vel = h.note.vel as f64 / 127.0;
        let fc = (0x0C0 as f64 + 0x500 as f64 * open * vel * (-since / 30.0).exp()) as u16;
        sid.poke(chip, FC_LO, (fc & 7) as u8);
        sid.poke(chip, FC_HI, (fc >> 3) as u8);
        sid.poke(chip, RES_FILT, 0xC0 | 1 << slot.1);
    }

    /// `keys` on `slot`: one held, several arpeggiated; retriggered when
    /// the first of them is new.
    fn tone(&mut self, sid: &mut impl Regs, slot: Slot, keys: &[(u8, u64)], patch: Patch, now: u64) {
        let Some(&(_, start)) = keys.iter().max_by_key(|k| k.1) else {
            self.release(sid, slot, patch.wave);
            return;
        };
        if self.retrigger(sid, slot, Playing { track: 0, key: 0, start }) {
            return;
        }
        let key = keys[((now / ARP) % keys.len() as u64) as usize].0;
        let (chip, v) = slot;
        if patch.wave == PULSE {
            // A slow pulse-width sweep.
            let pw = 0x400 + ((now as f64 / 40.0).sin() * 0x300 as f64) as i32;
            sid.poke(chip, 7 * v + PW, pw as u8);
            sid.poke(chip, 7 * v + PW + 1, (pw >> 8) as u8 & 0x0F);
        }
        Self::sound(sid, slot, key as f64, patch, true);
    }
}

/// Plays the song on three emulated SIDs, sample by sample, round and
/// round.
pub struct Synth {
    sids: [Sid; CHIPS],
    player: Player,
    /// Each chip's samples for the current tick.
    buffers: [Vec<i16>; CHIPS],
    kit: Kit,
    read: usize,
    /// Cycles owed to the chips, in 1/TICKS of a cycle.
    carry: u32,
}

impl Synth {
    pub fn new(sample_rate: u32) -> Self {
        let sids = std::array::from_fn(|_| {
            let mut sid = Sid::new(ChipModel::Mos6581);
            sid.set_sampling_parameters(SamplingMethod::Fast, CLOCK, sample_rate);
            // Volume up and a tenth of a second to settle, unheard: the
            // 6581 pops when its volume jumps, so it mustn't jump on air.
            sid.write(MODE_VOL, 0x1F);
            let mut scratch = [0i16; 1024];
            let mut left = CLOCK / 10;
            while left > 0 {
                left = sid.sample(left, &mut scratch, 1).1;
            }
            sid
        });
        Synth { sids, player: Player::new(), buffers: Default::default(), kit: Kit::new(sample_rate), read: 0, carry: 0 }
    }

    #[cfg(test)]
    fn solo(mut self, track: u8) -> Self {
        self.player.solo = Some(track);
        self
    }

    /// The next sample, left and right, -1..1.
    pub fn next_stereo(&mut self) -> (f32, f32) {
        while self.read >= self.buffers.iter().map(Vec::len).min().unwrap_or(0) {
            self.player.tick(&mut self.sids);
            for (drum, vel) in self.player.strikes.drain(..) {
                self.kit.strike(drum, vel);
            }
            // One tick's worth of chip time.
            self.carry += CLOCK;
            let cycles = self.carry / TICKS;
            self.carry %= TICKS;
            self.read = 0;
            let mut chunk = [0i16; 1024];
            for (sid, buffer) in self.sids.iter_mut().zip(&mut self.buffers) {
                buffer.clear();
                let mut left = cycles;
                while left > 0 {
                    let (n, rest) = sid.sample(left, &mut chunk, 1);
                    buffer.extend_from_slice(&chunk[..n]);
                    left = rest;
                }
            }
        }
        let (mut l, mut r) = (0.0, 0.0);
        for (buffer, (pl, pr)) in self.buffers.iter().zip(PAN) {
            let s = buffer[self.read] as f32 / 32_768.0;
            l += s * pl;
            r += s * pr;
        }
        let kit = self.kit.next() * KIT;
        self.read += 1;
        ((l * 2.0 + kit).tanh(), (r * 2.0 + kit).tanh())
    }

    /// The next sample, both channels together (tests measure this).
    #[cfg(test)]
    pub fn next(&mut self) -> f32 {
        let (l, r) = self.next_stereo();
        (l + r) / 2.0
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
    // Left and right as the first two channels; any others get both.
    let mut frame = move |out: &mut dyn FnMut(usize, f32)| {
        let (l, r) = synth.next_stereo();
        for c in 0..channels {
            out(c, if channels < 2 { (l + r) / 2.0 } else { [l, r].get(c).copied().unwrap_or((l + r) / 2.0) });
        }
    };
    let quiet = |_| {};
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for chunk in data.chunks_mut(channels) {
                    frame(&mut |c, s| chunk[c] = s);
                }
            },
            quiet,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                for chunk in data.chunks_mut(channels) {
                    frame(&mut |c, s| chunk[c] = (s * i16::MAX as f32) as i16);
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
    use super::super::orbital3::TRACKS;
    use super::*;

    const RATE: u32 = 22_050;
    const VOICED: [u8; 7] = [PAD, BELLS, ACID, DRUMS, MALLETS, ACID_HIGH, THUD];

    /// The registers as the player leaves them, per chip.
    struct Book([[u8; 32]; CHIPS]);

    impl Regs for Book {
        fn poke(&mut self, chip: usize, reg: u8, value: u8) {
            self.0[chip][reg as usize] = value;
        }
    }

    /// `secs` of `synth`, from `from` seconds in: the player alone runs up
    /// to there (no chip time to emulate), then hands the chips their
    /// registers and a moment to settle.
    fn render(mut synth: Synth, from: f64, secs: f64) -> Vec<f32> {
        let skip = ((from - 0.5).max(0.0) * TICKS as f64) as u64;
        let mut book = Book([[0; 32]; CHIPS]);
        for _ in 0..skip {
            synth.player.tick(&mut book);
        }
        synth.player.strikes.clear();
        for (sid, regs) in synth.sids.iter_mut().zip(&book.0) {
            for (reg, &value) in regs.iter().enumerate().take(0x19) {
                sid.write(reg as u8, value);
            }
        }
        let settle = ((from - skip as f64 / TICKS as f64) * RATE as f64) as usize;
        for _ in 0..settle {
            synth.next();
        }
        (0..(secs * RATE as f64) as usize).map(|_| synth.next()).collect()
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    }

    fn first_secs(track: u8) -> f64 {
        NOTES.iter().find(|n| n.track == track).unwrap().at * 60.0 / BPM
    }

    #[test]
    fn notes_are_in_order_and_in_the_song() {
        assert!(NOTES.windows(2).all(|w| w[0].at <= w[1].at));
        assert!(NOTES.iter().all(|n| n.at >= 0.0 && n.at < BEATS && n.len > 0.0 && (n.track as usize) < TRACKS.len()));
        for n in NOTES {
            assert!(VOICED.contains(&n.track), "track {} ({}) unvoiced", n.track, TRACKS[n.track as usize].0);
        }
    }

    #[test]
    fn a_sixteenth_is_a_whole_number_of_ticks() {
        assert_eq!(per_beat() / 4.0, 25.0);
        assert!(NOTES.iter().all(|n| (n.at * per_beat()).fract().abs() < 1e-6), "notes off the tick grid");
    }

    #[test]
    fn frequencies_are_pal_sid_values() {
        // f_reg = Hz · 2^24 / 985248: A-4 (440 Hz) is $1D45 on a PAL C64.
        assert_eq!(freq(69.0), 0x1D45);
        // An octave doubles it (within rounding).
        assert!((freq(81.0) as i32 - 2 * freq(69.0) as i32).abs() <= 1);
        assert!((2 * freq(57.0) as i32 - freq(69.0) as i32).abs() <= 1);
    }

    #[test]
    fn new_notes_hard_restart_then_gate() {
        let mut p = Player::new().only(ACID);
        let mut book = Book([[0; 32]; CHIPS]);
        let first = (first_secs(ACID) * TICKS as f64).round() as u64;
        for _ in 0..=first {
            p.tick(&mut book);
        }
        // The first 303 note: a tick of test bit with the envelope zeroed...
        assert_eq!((book.0[0][CTRL as usize], book.0[0][AD as usize]), (TEST, 0));
        p.tick(&mut book);
        // ...then the gate.
        assert_eq!(book.0[0][CTRL as usize] & GATE, GATE);
    }

    #[test]
    fn the_drums_go_to_the_808_not_the_chips() {
        let mut p = Player::new().only(DRUMS);
        let mut book = Book([[0; 32]; CHIPS]);
        let mut struck = Vec::new();
        for _ in 0..TICKS * 30 {
            p.tick(&mut book);
            struck.append(&mut p.strikes);
        }
        assert!(struck.iter().any(|&(d, _)| d == Drum::Kick), "{struck:?}");
        // No voice gated on any chip.
        assert!(book.0.iter().all(|regs| (0..3).all(|v| regs[7 * v + CTRL as usize] & GATE == 0)));
    }

    #[test]
    fn the_mallets_echo_a_dotted_eighth_later() {
        let mut p = Player::new().only(MALLETS);
        let mut book = Book([[0; 32]; CHIPS]);
        let first = (first_secs(MALLETS) * TICKS as f64).round() as u64;
        let gated = |b: &Book, (chip, v): Slot| b.0[chip][7 * v as usize + CTRL as usize] & GATE != 0;
        for _ in 0..first + 2 {
            p.tick(&mut book);
        }
        assert!(gated(&book, B3) && !gated(&book, C3));
        for _ in 0..ECHO_DELAY {
            p.tick(&mut book);
        }
        assert!(gated(&book, C3));
    }

    #[test]
    fn the_song_starts_over() {
        let mut p = Player::new();
        let mut book = Book([[0; 32]; CHIPS]);
        for _ in 0..Player::length() + 10 {
            p.tick(&mut book);
        }
        assert_eq!(p.tick, 10);
        assert!(p.next < 20, "at note {}", p.next);
    }

    #[test]
    fn the_bell_leads_are_out_of_the_mix() {
        let mut p = Player::new();
        let mut book = Book([[0; 32]; CHIPS]);
        for _ in 0..Player::length() {
            p.tick(&mut book);
            for (chip, v) in [B2, B3, C3] {
                assert_eq!(book.0[chip][7 * v as usize + CTRL as usize] & GATE, 0, "voice ({chip}, {v}) plays at tick {}", p.tick);
            }
        }
    }

    #[test]
    fn plays_cleanly() {
        // From the start, where it's drums and bass, and the busy middle.
        for from in [0.0, 70.0] {
            let s = render(Synth::new(RATE), from, 3.0);
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            assert!(rms(&s) > 0.02, "silent from {from}s: {}", rms(&s));
        }
    }

    #[test]
    fn no_pop_when_the_chips_come_on() {
        let s = render(Synth::new(RATE), 0.0, 0.02);
        let peak = s.iter().fold(0f32, |a, x| a.max(x.abs()));
        assert!(peak < 0.4, "a {peak} pop at the start");
    }

    #[test]
    fn every_track_sounds_when_it_plays() {
        for track in VOICED {
            let s = render(Synth::new(RATE).solo(track), first_secs(track), 2.0);
            assert!(rms(&s) > 0.01, "{} is silent: {}", TRACKS[track as usize].0, rms(&s));
        }
    }

    #[test]
    fn the_second_303_is_quieter_than_the_first() {
        let level = |track| rms(&render(Synth::new(RATE).solo(track), first_secs(track), 4.0));
        let (first, second) = (level(ACID), level(ACID_HIGH));
        assert!(second < first * 0.8, "first {first}, second {second}");
    }

    impl Player {
        fn only(mut self, track: u8) -> Self {
            self.solo = Some(track);
            self
        }
    }

    /// The whole song as a stereo WAV to listen to without the demo:
    /// `cargo test --release --example hills render_song -- --ignored`,
    /// then open `target/orbital3.wav`. Prints each track's level.
    #[test]
    #[ignore]
    fn render_song() {
        let sr = 44_100;
        let secs = Player::length() as f64 / TICKS as f64;
        for track in VOICED {
            let s = render(Synth::new(sr).solo(track), first_secs(track), 8.0);
            println!("{:>18}: rms {:.3} (8 s from its first note)", TRACKS[track as usize].0, rms(&s));
        }
        // The drums' first bars are just hats: the full groove too.
        let s = render(Synth::new(sr).solo(DRUMS), 40.0, 8.0);
        println!("{:>18}: rms {:.3} (8 s from 40 s)", TRACKS[DRUMS as usize].0, rms(&s));
        let mut synth = Synth::new(sr);
        let mut samples: Vec<i16> = Vec::new();
        for _ in 0..(secs * sr as f64) as usize {
            let (l, r) = synth.next_stereo();
            samples.extend([(l * 32_000.0) as i16, (r * 32_000.0) as i16]);
        }
        let mut wav = Vec::new();
        let data = samples.len() as u32 * 2;
        wav.extend(b"RIFF");
        wav.extend((36 + data).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(sr.to_le_bytes());
        wav.extend((sr * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
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
