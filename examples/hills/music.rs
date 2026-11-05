//! The demo's music: the loops of "orbital 3"'s Session View (see
//! `orbital3.rs`, generated from the Ableton set), launched by the scroll
//! text the way you launch clips in Ableton — `{play drums full}`,
//! `{stop pad}`, `{stop all}` — each starting on the next bar. They play
//! on three emulated C64 SIDs (MOS 6581s by reSID, "3SID"), the way a C64
//! music routine does it: a player writes the chips' registers 200 times
//! a second. The 606 part plays on an 808 (`drums.rs`), and it all goes
//! through a hall (`reverb.rs`).
//!
//! | chip | voice 1 | voice 2 | voice 3 | filter |
//! |---|---|---|---|---|
//! | A, centre | 303 | marimba | thud bass | the 303's acid |
//! | B, left | second 303, quieter | mallets, soft | mallets, soft | the second 303's |
//! | C, right | pad | pad | pad | a slow, low pass |
//!
//! Chip C is one soft pad: the slow pad's notes and the bells' chords,
//! a note to a voice, slow in and slow out, through its own low-pass.
//!
//! What plays is worked out from the demo's script time alone (`Session`),
//! so skipping back and forth, or the show starting over, always lands on
//! the right loops at the right point in their bars.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use resid::{ChipModel, SamplingMethod, Sid};

use super::drums::{Drum, Kit};
use super::orbital3::{BPM, CLIPS, TRACKS};
use super::reverb::Reverb;
use super::script::{Code, Timeline};

/// One note of a loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    /// Beats from the start of its loop.
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

/// A loop from the Session View.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    /// Index into `TRACKS`.
    pub track: u8,
    /// Its name in Ableton (often none).
    pub name: &'static str,
    /// Beats before it comes round again.
    pub beats: f64,
    pub notes: &'static [Note],
}

/// What the scroll text calls each track, in track order.
const TRACK_NAMES: [&str; 9] = ["pad", "bells", "303", "drums", "mallets", "marimba", "303b", "bass", "toy"];
/// The cold open's music, before any code: (track, loop, seconds) — the
/// pad, from the start.
const OPENING: [(&str, Option<&str>, f64); 1] = [("pad", None, 0.0)];

/// A PAL C64's clock, cycles a second.
const CLOCK: u32 = 985_248;
/// Player calls a second ("4× speed"): a 16th at 120 BPM is 25 of them.
const TICKS: u32 = 200;
/// Ticks in a bar: loops start on one.
const BAR: u64 = 400;
/// Ticks between arpeggio steps — 50 Hz, the classic rate.
const ARP: u64 = 4;
/// How many SIDs.
const CHIPS: usize = 3;
/// Each chip's share of the left and right channel — the pad chip
/// kept back.
const PAN: [(f32, f32); CHIPS] = [(0.5, 0.5), (0.65, 0.35), (0.08, 0.14)];
/// The 808's level next to the chips (it sits in the middle).
const KIT: f32 = 0.35;
/// How much of each chip, and of the 808, goes into the hall — the pad
/// most, the bass hardly — and how loud the hall comes back.
const SEND: [f32; CHIPS] = [0.08, 0.5, 0.18];
const KIT_SEND: f32 = 0.2;
const WET: f32 = 1.2;
/// The hall: how long it rings (0–1), how dark (0–1), ms before it answers.
const HALL_ROOM: f32 = 0.98;
const HALL_DAMP: f32 = 0.25;
const HALL_PRE_MS: f32 = 70.0;
/// Each chip's filter, set before anything plays: the 6581 pops when its
/// filter routing changes, so it never does. The 303s' voice 1 on A and
/// B (their cutoffs move, their routing doesn't), the pad's three voices
/// on C with a little resonance.
const FILTERS: [u8; CHIPS] = [0xC1, 0xC1, 0x27];
const PAD_CUTOFF: u16 = 0x230;
/// Beats a bells chord holds as a pad, so the chords run into each other.
const CHORD_BEATS: f64 = 2.0;
/// The demo's time and the music's drifting further apart than this
/// (a skip, the show starting over): the music jumps to the demo.
const RESYNC: u64 = TICKS as u64 / 10;

// The tracks, by index into `TRACKS`.
const PAD: u8 = 0;
const BELLS: u8 = 1;
const ACID: u8 = 2;
const DRUMS: u8 = 3;
const MALLETS: u8 = 4;
const MARIMBA: u8 = 5;
const ACID_HIGH: u8 = 6;
const THUD: u8 = 7;

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
const A2: Slot = (0, 1);
const A3: Slot = (0, 2);
const B1: Slot = (1, 0);
const B2: Slot = (1, 1);
const B3: Slot = (1, 2);
/// The pad chip's three voices.
const PADS: [Slot; 3] = [(2, 0), (2, 1), (2, 2)];

/// The track the scroll text calls `name`.
pub fn track_named(name: &str) -> Option<u8> {
    TRACK_NAMES.iter().position(|&n| n.eq_ignore_ascii_case(name)).map(|i| i as u8)
}

/// The loop `{play <track> <which>}` means, as an index into `CLIPS`:
/// `which` is a loop's name or its number on the track (from 1); none is
/// the track's first.
pub fn clip_named(track: &str, which: Option<&str>) -> Result<usize, String> {
    let t = track_named(track).ok_or_else(|| format!("no track {track:?} (tracks: {})", TRACK_NAMES.join(", ")))?;
    let mut loops = CLIPS.iter().enumerate().filter(|(_, c)| c.track == t);
    let found = match which {
        None => loops.next(),
        Some(w) => match w.parse::<usize>() {
            Ok(n) => loops.nth(n.max(1) - 1),
            Err(_) => loops.find(|(_, c)| c.name.eq_ignore_ascii_case(w)),
        },
    };
    found.map(|(i, _)| i).ok_or_else(|| format!("{track} has no loop {:?}", which.unwrap_or("1")))
}

/// The SID's frequency register for MIDI `key`.
fn freq(key: f64) -> u16 {
    let hz = 440.0 * 2f64.powf((key - 69.0) / 12.0);
    (hz * 16_777_216.0 / CLOCK as f64).round().min(65_535.0) as u16
}

/// Ticks in one beat.
fn per_beat() -> f64 {
    TICKS as f64 * 60.0 / BPM
}

fn ticks(beats: f64) -> u64 {
    (beats * per_beat()).round() as u64
}

/// A launch or a stop, from a code or the opening.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Launch {
    Play(usize),
    Stop(Option<u8>),
}

/// Per track: the loop it plays and the tick it started.
type Playing = [Option<(usize, u64)>; TRACKS.len()];

/// The scroll text's loop launches, each on the bar after its code fires.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// (tick it takes effect, launch), in order.
    launches: Vec<(u64, Launch)>,
}

impl Session {
    /// The launches in `timeline`'s codes, after the opening's.
    pub fn new(timeline: &Timeline) -> Self {
        let opening = OPENING.iter().map(|&(t, w, secs)| {
            let at = (secs * TICKS as f64).ceil() as u64;
            (at.div_ceil(BAR) * BAR, Launch::Play(clip_named(t, w).expect("opening loop")))
        });
        let coded = timeline.cues.iter().filter_map(|cue| {
            let launch = match cue.code {
                Code::Play(clip) => Launch::Play(clip),
                Code::Stop(track) => Launch::Stop(track),
                _ => return None,
            };
            // The next bar line at or after the code.
            let at = (cue.t * TICKS as f64).ceil() as u64;
            Some((at.div_ceil(BAR) * BAR, launch))
        });
        let mut launches: Vec<(u64, Launch)> = opening.chain(coded).collect();
        launches.sort_by_key(|l| l.0);
        Session { launches }
    }

    /// Whether `track` has a loop going `secs` into the script (tests).
    #[cfg(test)]
    pub fn plays(&self, secs: f64, track: &str) -> bool {
        self.loop_at(secs, track).is_some()
    }

    /// The loop `track` plays `secs` into the script, as an index into
    /// `CLIPS` (tests).
    #[cfg(test)]
    pub fn loop_at(&self, secs: f64, track: &str) -> Option<usize> {
        self.at((secs * TICKS as f64) as u64)[track_named(track).unwrap() as usize].map(|p| p.0)
    }

    /// Just these loops, from the start (tests).
    #[cfg(test)]
    fn only(clips: &[usize]) -> Self {
        Session { launches: clips.iter().map(|&c| (0, Launch::Play(c))).collect() }
    }

    /// What plays at tick `now`.
    fn at(&self, now: u64) -> Playing {
        let mut playing: Playing = [None; TRACKS.len()];
        for &(at, launch) in self.launches.iter().take_while(|l| l.0 <= now) {
            match launch {
                // Launching what already plays carries on.
                Launch::Play(c) => {
                    let track = &mut playing[CLIPS[c].track as usize];
                    if track.is_none_or(|(p, _)| p != c) {
                        *track = Some((c, at));
                    }
                }
                Launch::Stop(Some(t)) => playing[t as usize] = None,
                Launch::Stop(None) => playing = [None; TRACKS.len()],
            }
        }
        playing
    }
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

/// A note sounding, from tick `start` to `end`, out of loop `clip`
/// launched at `launch`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    note: Note,
    start: u64,
    end: u64,
    clip: usize,
    launch: u64,
    /// Which of its track's voices it goes to, where they take turns.
    turn: u8,
}

/// What a voice is playing: which note (for retriggering) and since when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Voice {
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

/// The music routine: follows the demo's clock, plays the loops the
/// session has going, and writes the SIDs' registers, one tick at a time.
pub struct Player {
    session: Session,
    /// Script time in ticks; None before the first tick.
    now: Option<u64>,
    held: Vec<Held>,
    /// Mallet notes started, for taking turns between their two voices.
    mallets: u8,
    voices: [[Option<Voice>; 3]; CHIPS],
    /// Drums struck this tick, for the 808.
    pub strikes: Vec<(Drum, u8)>,
}

impl Player {
    pub fn new(session: Session) -> Self {
        Player { session, now: None, held: Vec::new(), mallets: 0, voices: [[None; 3]; CHIPS], strikes: Vec::new() }
    }

    /// Advance one tick, the demo `demo_secs` into its script, writing
    /// this tick's registers.
    pub fn tick(&mut self, sid: &mut impl Regs, demo_secs: f64) {
        let demo = (demo_secs.max(0.0) * TICKS as f64).round() as u64;
        let now = match self.now {
            Some(n) if n.abs_diff(demo) <= RESYNC => n,
            first_or_jump => {
                if first_or_jump.is_none() {
                    for (chip, &filter) in FILTERS.iter().enumerate() {
                        sid.poke(chip, MODE_VOL, 0x1F);
                        sid.poke(chip, RES_FILT, filter);
                    }
                }
                // Jumped: what rang stops; the loops pick up where the
                // demo is now.
                self.held.clear();
                for chip in 0..CHIPS {
                    for v in 0..3 {
                        self.release(sid, (chip, v), 0);
                    }
                }
                demo
            }
        };
        let playing = self.session.at(now);
        // Notes of loops that stopped, or were swapped, go.
        self.held.retain(|h| h.end > now && playing[h.note.track as usize] == Some((h.clip, h.launch)));
        for (c, launch) in playing.iter().flatten().copied() {
            let clip = CLIPS[c];
            let length = ticks(clip.beats).max(1);
            if now < launch {
                continue;
            }
            let phase = (now - launch) % length;
            for &note in clip.notes.iter().filter(|n| ticks(n.at) % length == phase) {
                self.hold(note, c, launch, now);
            }
        }
        self.voices(sid, now);
        self.now = Some(now + 1);
    }

    /// Hold `note` (of loop `clip`, launched at `launch`) from `now`.
    fn hold(&mut self, note: Note, clip: usize, launch: u64, now: u64) {
        if note.track == DRUMS {
            self.strikes.extend(Drum::on(note.key).map(|d| (d, note.vel)));
            return;
        }
        let mut turn = 0;
        if note.track == MALLETS {
            turn = self.mallets % 2;
            self.mallets = self.mallets.wrapping_add(1);
        }
        let mut end = now + ticks(note.len).max(1);
        if note.track == BELLS {
            end = end.max(now + ticks(CHORD_BEATS));
        }
        self.held.push(Held { note, start: now, end, clip, launch, turn });
    }

    /// Every voice's registers for this tick.
    fn voices(&mut self, sid: &mut impl Regs, now: u64) {
        self.acid(sid, A1, ACID, 1.0, Patch { wave: SAW, ad: 0x08, sr: 0xA0 }, now);
        // The second 303: quieter — a lower sustain, its filter opening
        // less far.
        self.acid(sid, B1, ACID_HIGH, 0.45, Patch { wave: SAW, ad: 0x06, sr: 0x50 }, now);
        let marimba = self.keys(|h| h.note.track == MARIMBA, 12);
        self.tone(sid, A2, &marimba, Patch { wave: TRI, ad: 0x08, sr: 0x00 }, now);
        let thud = self.keys(|h| h.note.track == THUD, 0);
        self.tone(sid, A3, &thud, Patch { wave: TRI, ad: 0x09, sr: 0x00 }, now);
        // The mallets, soft: triangles taking turns, each note ringing
        // out under the next.
        for (turn, slot) in [B2, B3].into_iter().enumerate() {
            let keys = self.keys(|h| h.note.track == MALLETS && h.turn == turn as u8, 12);
            self.pad(sid, &[slot], &keys, Patch { wave: TRI, ad: 0x68, sr: 0x9A }, now);
        }
        // The pad: the slow pad's notes and the bells' chords together,
        // a note to a voice.
        let mut chord = self.keys(|h| h.note.track == PAD, 12);
        chord.extend(self.keys(|h| h.note.track == BELLS, 24));
        chord.sort_unstable();
        chord.dedup_by_key(|k| k.0);
        self.pad(sid, &PADS, &chord, Patch { wave: PULSE, ad: 0xA0, sr: 0x9B }, now);
        // Its filter breathes, slowly.
        let fc = (PAD_CUTOFF as f64 + 0x90 as f64 * (now as f64 / 400.0).sin()) as u16;
        sid.poke(PADS[0].0, FC_LO, (fc & 7) as u8);
        sid.poke(PADS[0].0, FC_HI, (fc >> 3) as u8);
    }

    /// The sounding keys of the held notes `which` picks, `shift`
    /// semitones up, lowest first, each with the tick it started.
    fn keys(&self, which: impl Fn(&Held) -> bool, shift: u8) -> Vec<(u8, u64)> {
        let mut keys: Vec<(u8, u64)> = self.held.iter().filter(|h| which(h)).map(|h| (h.note.key + shift, h.start)).collect();
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
    fn retrigger(&mut self, sid: &mut impl Regs, (chip, v): Slot, voice: Voice) -> bool {
        if self.voices[chip][v as usize] == Some(voice) {
            return false;
        }
        self.voices[chip][v as usize] = Some(voice);
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
        if self.retrigger(sid, slot, Voice { track, key: h.note.key, start: h.start }) {
            return;
        }
        Self::sound(sid, slot, h.note.key as f64, patch, true);
        let since = (now - h.start) as f64;
        let vel = h.note.vel as f64 / 127.0;
        let fc = (0x0C0 as f64 + 0x500 as f64 * open * vel * (-since / 30.0).exp()) as u16;
        sid.poke(chip, FC_LO, (fc & 7) as u8);
        sid.poke(chip, FC_HI, (fc >> 3) as u8);
    }

    /// Soft voices: `keys` one to a slot, lowest first (any beyond the
    /// slots left out). A slot that goes from silence to a note fades in
    /// with the patch's attack — no hard restart, nothing snaps; one whose
    /// note changes just changes pitch; one left without a note fades out
    /// with the patch's release. Each slot's pulse width drifts on its own.
    fn pad(&mut self, sid: &mut impl Regs, slots: &[Slot], keys: &[(u8, u64)], patch: Patch, now: u64) {
        for (i, &(chip, v)) in slots.iter().enumerate() {
            let Some(&(key, start)) = keys.get(i) else {
                self.release(sid, (chip, v), patch.wave);
                continue;
            };
            let base = 7 * v;
            if self.voices[chip][v as usize].is_none() {
                sid.poke(chip, base + AD, patch.ad);
                sid.poke(chip, base + SR, patch.sr);
                sid.poke(chip, base + CTRL, patch.wave | GATE);
            }
            self.voices[chip][v as usize] = Some(Voice { track: 0, key, start });
            let f = freq(key as f64);
            sid.poke(chip, base + FREQ, f as u8);
            sid.poke(chip, base + FREQ + 1, (f >> 8) as u8);
            let pw = 0x600 + ((now as f64 / 90.0 + i as f64 * 2.1).sin() * 0x280 as f64) as i32;
            sid.poke(chip, base + PW, pw as u8);
            sid.poke(chip, base + PW + 1, (pw >> 8) as u8 & 0x0F);
        }
    }

    /// `keys` on `slot`: one held, several arpeggiated; retriggered when
    /// the newest of them is new.
    fn tone(&mut self, sid: &mut impl Regs, slot: Slot, keys: &[(u8, u64)], patch: Patch, now: u64) {
        let Some(&(_, start)) = keys.iter().max_by_key(|k| k.1) else {
            self.release(sid, slot, patch.wave);
            return;
        };
        if self.retrigger(sid, slot, Voice { track: 0, key: 0, start }) {
            return;
        }
        let key = keys[((now / ARP) % keys.len() as u64) as usize].0;
        Self::sound(sid, slot, key as f64, patch, true);
    }
}

/// Plays the session on three emulated SIDs and the 808, through the
/// hall, sample by sample, following the demo's clock.
pub struct Synth {
    sids: [Sid; CHIPS],
    player: Player,
    /// The demo's script time, as f64 bits, written each frame.
    clock: Arc<AtomicU64>,
    /// Each chip's samples for the current tick.
    buffers: [Vec<i16>; CHIPS],
    kit: Kit,
    hall: Reverb,
    read: usize,
    /// Cycles owed to the chips, in 1/TICKS of a cycle.
    carry: u32,
}

impl Synth {
    pub fn new(sample_rate: u32, session: Session, clock: Arc<AtomicU64>) -> Self {
        let sids = std::array::from_fn(|chip| {
            let mut sid = Sid::new(ChipModel::Mos6581);
            sid.set_sampling_parameters(SamplingMethod::Fast, CLOCK, sample_rate);
            // Volume up and filters set, and a tenth of a second to
            // settle, unheard: the 6581 pops when its volume or filter
            // routing jumps, so they mustn't jump on air.
            sid.write(MODE_VOL, 0x1F);
            sid.write(RES_FILT, FILTERS[chip]);
            sid.write(FC_LO, (PAD_CUTOFF & 7) as u8);
            sid.write(FC_HI, (PAD_CUTOFF >> 3) as u8);
            let mut scratch = [0i16; 1024];
            let mut left = CLOCK / 10;
            while left > 0 {
                left = sid.sample(left, &mut scratch, 1).1;
            }
            sid
        });
        Synth {
            sids,
            player: Player::new(session),
            clock,
            buffers: Default::default(),
            kit: Kit::new(sample_rate),
            hall: Reverb::new(sample_rate, HALL_ROOM, HALL_DAMP, HALL_PRE_MS),
            read: 0,
            carry: 0,
        }
    }

    /// The next sample, left and right, -1..1.
    pub fn next_stereo(&mut self) -> (f32, f32) {
        while self.read >= self.buffers.iter().map(Vec::len).min().unwrap_or(0) {
            let demo = f64::from_bits(self.clock.load(Ordering::Relaxed));
            self.player.tick(&mut self.sids, demo);
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
        let (mut l, mut r, mut send) = (0.0, 0.0, 0.0);
        for ((buffer, (pl, pr)), to_hall) in self.buffers.iter().zip(PAN).zip(SEND) {
            let s = buffer[self.read] as f32 / 32_768.0 * 2.0;
            l += s * pl;
            r += s * pr;
            send += s * to_hall;
        }
        let kit = self.kit.next() * KIT;
        send += kit * KIT_SEND;
        let (hall_l, hall_r) = self.hall.run(send);
        self.read += 1;
        ((l + kit + hall_l * WET).tanh(), (r + kit + hall_r * WET).tanh())
    }
}

/// Start the music on the default output device, following the demo's
/// `clock` (its script time as f64 bits) through `session`; it plays as
/// long as the returned stream lives. None when there is no sound to be
/// had (no device, an unusual sample format) — the demo just runs silent.
pub fn play(session: Session, clock: Arc<AtomicU64>) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;
    let mut synth = Synth::new(config.sample_rate.0, session, clock);
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
    use super::super::script;
    use super::*;

    const RATE: u32 = 22_050;

    /// The registers as the player leaves them, per chip.
    struct Book([[u8; 32]; CHIPS]);

    impl Book {
        fn new() -> Self {
            Book([[0; 32]; CHIPS])
        }

        fn gated(&self, (chip, v): Slot) -> bool {
            self.0[chip][7 * v as usize + CTRL as usize] & GATE != 0
        }
    }

    impl Regs for Book {
        fn poke(&mut self, chip: usize, reg: u8, value: u8) {
            self.0[chip][reg as usize] = value;
        }
    }

    /// The session `src` (a scroll text) launches on a screen 100 wide.
    fn session_of(src: &str) -> (Session, Timeline) {
        let placed = script::parse(src).unwrap().place(100).unwrap();
        let tl = Timeline::new(&placed, 100);
        (Session::new(&tl), tl)
    }

    fn clock() -> Arc<AtomicU64> {
        Arc::new(AtomicU64::new(0f64.to_bits()))
    }

    /// `secs` of `loops` playing from the start, from `from` seconds in.
    fn render(loops: &[usize], from: f64, secs: f64) -> Vec<f32> {
        let time = clock();
        let mut synth = Synth::new(RATE, Session::only(loops), time.clone());
        let mut out = Vec::new();
        for i in 0..((from + secs) * RATE as f64) as usize {
            time.store((i as f64 / RATE as f64).to_bits(), Ordering::Relaxed);
            let (l, r) = synth.next_stereo();
            if i as f64 >= from * RATE as f64 {
                out.push((l + r) / 2.0);
            }
        }
        out
    }

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    }

    fn first(track: &str) -> usize {
        clip_named(track, None).unwrap()
    }

    #[test]
    fn loops_are_tidy() {
        assert!(!CLIPS.is_empty());
        for (i, c) in CLIPS.iter().enumerate() {
            assert!(c.beats > 0.0 && (c.track as usize) < TRACKS.len(), "loop {i}");
            assert!(c.notes.iter().all(|n| n.track == c.track && n.at >= 0.0 && n.at < c.beats && n.len > 0.0), "loop {i}");
        }
    }

    #[test]
    fn loops_by_track_and_name_or_number() {
        let drums: Vec<usize> = (0..CLIPS.len()).filter(|&i| CLIPS[i].track == DRUMS).collect();
        assert_eq!(clip_named("drums", None), Ok(drums[0]));
        assert_eq!(clip_named("drums", Some("2")), Ok(drums[1]));
        assert_eq!(clip_named("DRUMS", Some("full")).map(|i| CLIPS[i].name), Ok("full"));
        assert!(clip_named("drums", Some("9")).is_err());
        assert!(clip_named("kazoo", None).is_err());
        assert_eq!(track_named("303b"), Some(ACID_HIGH));
    }

    #[test]
    fn frequencies_are_pal_sid_values() {
        // f_reg = Hz · 2^24 / 985248: A-4 (440 Hz) is $1D45 on a PAL C64.
        assert_eq!(freq(69.0), 0x1D45);
        assert!((freq(81.0) as i32 - 2 * freq(69.0) as i32).abs() <= 1);
    }

    #[test]
    fn codes_launch_on_the_next_bar() {
        let (session, tl) = session_of("ab{play bass}cdefgh{play drums full}ijklmnopqrstuvwxyz{stop bass}abcdefghijklmnopqrstuvwxyz{stop all}abc");
        let cue = |n: usize| tl.cues.iter().filter(|c| matches!(c.code, Code::Play(_) | Code::Stop(_))).nth(n).unwrap().t;
        let tick = |t: f64| (t * TICKS as f64).ceil() as u64;
        let bar_after = |t: f64| tick(t).div_ceil(BAR) * BAR;
        let bass = THUD as usize;
        // The opening pad from the start; the bass not until the bar after its code.
        assert!(session.at(0)[PAD as usize].is_some());
        let launch = bar_after(cue(0));
        assert!(launch >= tick(cue(0)) && launch % BAR == 0);
        assert!(session.at(launch - 1)[bass].is_none());
        assert_eq!(session.at(launch)[bass], Some((first("bass"), launch)));
        // Drums come in later, bass carries on; then the bass stops; then all.
        let drums = bar_after(cue(1));
        assert_eq!(session.at(drums)[DRUMS as usize].map(|p| CLIPS[p.0].name), Some("full"));
        assert_eq!(session.at(drums)[bass], Some((first("bass"), launch)));
        assert!(session.at(bar_after(cue(2)))[bass].is_none());
        assert!(session.at(bar_after(cue(3))).iter().all(Option::is_none));
    }

    #[test]
    fn a_loop_plays_its_notes_round_and_round() {
        let bass = first("bass");
        let mut p = Player::new(Session::only(&[bass]));
        let mut book = Book::new();
        let first_note = ticks(CLIPS[bass].notes[0].at);
        let length = ticks(CLIPS[bass].beats);
        let mut starts = Vec::new();
        for t in 0..length * 3 {
            p.tick(&mut book, t as f64 / TICKS as f64);
            if p.held.iter().any(|h| h.start == t && h.note == CLIPS[bass].notes[0]) {
                starts.push(t);
            }
        }
        assert_eq!(starts, [first_note, first_note + length, first_note + 2 * length]);
    }

    #[test]
    fn new_notes_hard_restart_then_gate() {
        let acid = first("303");
        let mut p = Player::new(Session::only(&[acid]));
        let mut book = Book::new();
        let at = ticks(CLIPS[acid].notes[0].at);
        for t in 0..=at {
            p.tick(&mut book, t as f64 / TICKS as f64);
        }
        // A tick of test bit with the envelope zeroed...
        assert_eq!((book.0[0][CTRL as usize], book.0[0][AD as usize]), (TEST, 0));
        p.tick(&mut book, (at + 1) as f64 / TICKS as f64);
        // ...then the gate.
        assert!(book.gated(A1));
    }

    #[test]
    fn skipping_jumps_the_music_to_the_demo() {
        let mut p = Player::new(Session::only(&[first("bass"), first("pad")]));
        let mut book = Book::new();
        for t in 0..300 {
            p.tick(&mut book, t as f64 / TICKS as f64);
        }
        // Small wobble: the music keeps its own steady count...
        p.tick(&mut book, 300.0 / TICKS as f64 + 0.03);
        assert_eq!(p.now, Some(301));
        // ...a skip: it jumps, and what rang is cut.
        p.tick(&mut book, 40.0);
        assert_eq!(p.now, Some(40 * TICKS as u64 + 1));
        assert!(p.held.iter().all(|h| h.start == 40 * TICKS as u64));
    }

    #[test]
    fn the_drums_go_to_the_808_not_the_chips() {
        let mut p = Player::new(Session::only(&[clip_named("drums", Some("full")).unwrap()]));
        let mut book = Book::new();
        let mut struck = Vec::new();
        for t in 0..BAR {
            p.tick(&mut book, t as f64 / TICKS as f64);
            struck.append(&mut p.strikes);
        }
        assert!(struck.iter().any(|&(d, _)| d == Drum::Kick), "{struck:?}");
        assert!(book.0.iter().all(|regs| (0..3).all(|v| regs[7 * v + CTRL as usize] & GATE == 0)));
    }

    #[test]
    fn the_pad_is_a_slow_soft_chord() {
        let mut p = Player::new(Session::only(&[first("pad"), first("bells")]));
        let mut book = Book::new();
        for t in 0..BAR {
            p.tick(&mut book, t as f64 / TICKS as f64);
        }
        let regs = &book.0[PADS[0].0];
        let gated: Vec<u16> =
            (0..3).filter(|v| regs[7 * v + CTRL as usize] & GATE != 0).map(|v| u16::from_le_bytes([regs[7 * v], regs[7 * v + 1]])).collect();
        assert_eq!(gated.len(), 3, "a chord on all three voices: {gated:?}");
        assert!(gated[0] != gated[1] && gated[1] != gated[2], "three different notes: {gated:?}");
        for v in 0..3 {
            // Attack at least half a second, release over two.
            assert!(regs[7 * v + AD as usize] >> 4 >= 0xA && regs[7 * v + SR as usize] & 0x0F >= 0xB);
        }
        assert_eq!(regs[RES_FILT as usize] & 0x07, 0x07);
    }

    #[test]
    fn filters_are_set_before_anything_plays() {
        let mut p = Player::new(Session::only(&[]));
        let mut book = Book::new();
        p.tick(&mut book, 0.0);
        for (chip, &filter) in FILTERS.iter().enumerate() {
            assert_eq!(book.0[chip][RES_FILT as usize], filter);
        }
    }

    #[test]
    fn plays_cleanly() {
        let all: Vec<usize> = (0..CLIPS.len()).filter(|&i| CLIPS[i].track != DRUMS || CLIPS[i].name == "full").collect();
        let s = render(&all, 0.0, 3.0);
        assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(rms(&s) > 0.02, "silent: {}", rms(&s));
    }

    #[test]
    fn no_pop_when_the_chips_come_on() {
        // Nothing playing: the chips switched on and settled are silent.
        let s = render(&[], 0.0, 0.5);
        let peak = s.iter().fold(0f32, |a, x| a.max(x.abs()));
        assert!(peak < 0.02, "a {peak} pop");
    }

    #[test]
    fn every_loop_sounds() {
        for (i, c) in CLIPS.iter().enumerate() {
            let s = render(&[i], 0.0, c.beats * 60.0 / BPM);
            assert!(rms(&s) > 0.005, "{} loop {i} ({}) is silent: {}", TRACK_NAMES[c.track as usize], c.name, rms(&s));
        }
    }

    #[test]
    fn the_second_303_is_quieter_than_the_first() {
        let level = |track| rms(&render(&[first(track)], 0.0, 4.0));
        let (one, two) = (level("303"), level("303b"));
        assert!(two < one * 0.8, "first {one}, second {two}");
    }

    /// The demo's soundtrack as a stereo WAV, codes and all, on a screen
    /// 160 wide: `cargo test --release --example hills render_song --
    /// --ignored`, then open `target/orbital3.wav`.
    #[test]
    #[ignore]
    fn render_song() {
        let sr = 44_100;
        let w = 160;
        let placed = script::parse(super::super::scroller::PITCH).unwrap().place(w).unwrap();
        let cold_open = super::super::intro::Intro::new(w, super::super::ROWS, super::super::SCROLLER_Y);
        let tl = Timeline::new(&placed, w).with_lead(cold_open.secs, &cold_open.marks);
        let time = clock();
        let mut synth = Synth::new(sr, Session::new(&tl), time.clone());
        let mut samples: Vec<i16> = Vec::new();
        for i in 0..(tl.total * sr as f64) as usize {
            time.store((i as f64 / sr as f64).to_bits(), Ordering::Relaxed);
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
        println!("wrote {} ({:.0} s)", path.display(), tl.total);
    }
}
