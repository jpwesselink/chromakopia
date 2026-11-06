//! The demo's music: the loops of "orbital 3"'s Session View (see
//! `orbital3.rs`, generated from the Ableton set), launched by the scroll
//! text the way you launch clips in Ableton — `{play drums full}`,
//! `{play mallets volume:20}` (20 %), `{play 303 cutoff:30 res:90 over:4}`
//! (the 303s' filter knobs — cutoff, res, env, decay — turned over 4
//! bars; knobs always turn, over a bar unless told), `{stop pad}`, `{stop all}` — each
//! starting on the next bar. They play on emulated C64 SIDs (MOS 6581s by
//! reSID), one chip to a track so each has its own level and filter, the
//! way a C64 music routine does it: a player writes the chips' registers
//! 200 times a second. The 606 part plays on an 808 (`drums.rs`), and it
//! all goes through a hall (`reverb.rs`).
//!
//! | chip | track | voices | filter |
//! |---|---|---|---|
//! | A, centre | 303 | 1 | its acid |
//! | B, left | 303b, quieter | 1 | its acid |
//! | C, right | pad and the bells' chords | 3 | a slow, low pass |
//! | D, left | mallets, soft, taking turns | 2 | — |
//! | E, right | marimba | 1 | — |
//! | F, centre | thud bass | 1 | — |
//!
//! Chip C is one soft pad: the slow pad's notes and the bells' chords,
//! a note to a voice, slow in and slow out, through its own low-pass.
//!
//! What plays is worked out from the demo's script time alone (`Session`),
//! so skipping back and forth, or the show starting over, always lands on
//! the right loops at the right point in their bars.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
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
const CHIPS: usize = 6;
/// Each chip's share of the left and right channel — the pad chip
/// kept back.
const PAN: [(f32, f32); CHIPS] = [(0.5, 0.5), (0.65, 0.35), (0.08, 0.14), (0.65, 0.35), (0.45, 0.55), (0.5, 0.5)];
/// The 808's level next to the chips (it sits in the middle).
const KIT: f32 = 0.35;
/// How much of each chip, and of the 808, goes into the hall — the pad
/// most, the bass hardly — and how loud the hall comes back.
const SEND: [f32; CHIPS] = [0.08, 0.15, 0.18, 0.5, 0.2, 0.05];
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
const FILTERS: [u8; CHIPS] = [0xC1, 0xC1, 0x27, 0x00, 0x00, 0x00];
/// Samples a level change takes to settle, so volume changes don't click.
const LEVEL_GLIDE: f32 = 2_000.0;
const PAD_CUTOFF: u16 = 0x230;
/// Beats a bells chord holds as a pad, so the chords run into each other.
const CHORD_BEATS: f64 = 2.0;
/// The demo's time and the music's further apart than this (a skip, the
/// show starting over): the music jumps to the demo, cutting what rang.
const RESYNC: u64 = TICKS as u64;
/// Ticks the music may drift from the demo before it's nudged back — a
/// tick held or a tick doubled, unheard. (The demo counts frames, the
/// music samples; a dropped frame or two shouldn't cut the music short.)
const DRIFT: u64 = 2;

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
const B1: Slot = (1, 0);
/// The pad chip's three voices.
const PADS: [Slot; 3] = [(2, 0), (2, 1), (2, 2)];
/// The mallets' two voices, taking turns.
const MALLET_SLOTS: [Slot; 2] = [(3, 0), (3, 1)];
const MARIMBA_SLOT: Slot = (4, 0);
const THUD_SLOT: Slot = (5, 0);

/// The chip `track` plays on (None: the 808's drums).
fn chip_of(track: u8) -> Option<usize> {
    match track {
        ACID => Some(0),
        ACID_HIGH => Some(1),
        PAD | BELLS => Some(2),
        MALLETS => Some(3),
        MARIMBA => Some(4),
        THUD => Some(5),
        _ => None,
    }
}

/// What the scroll text calls track `t`.
pub fn track_name(t: u8) -> &'static str {
    TRACK_NAMES[t as usize]
}

/// Loop `clip` as the scroll text names it: its track and its name, or
/// its number on the track.
pub fn clip_label(clip: usize) -> String {
    let c = CLIPS[clip];
    let number = CLIPS[..clip].iter().filter(|o| o.track == c.track).count() + 1;
    let which = if c.name.is_empty() { number.to_string() } else { c.name.to_string() };
    format!("{} {which}", TRACK_NAMES[c.track as usize])
}

/// Seconds in a bar.
pub fn bar_secs() -> f64 {
    BAR as f64 / TICKS as f64
}

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

/// What a `{play}` turns, all 0–100: the track's volume, and for the
/// 303s their filter's knobs — cutoff, resonance, how far each note's
/// envelope opens it, how fast that closes again — and over how many bars
/// (None: leave it where it is; a bar).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Knobs {
    pub volume: Option<u8>,
    pub cutoff: Option<u8>,
    pub res: Option<u8>,
    pub env: Option<u8>,
    pub decay: Option<u8>,
    pub over: Option<u8>,
}

impl Knobs {
    /// Whether any of the 303 filter's knobs is set.
    pub fn filtered(&self) -> bool {
        self.cutoff.or(self.res).or(self.env).or(self.decay).is_some()
    }
}

/// Where a track's knobs stand, 0–100 each (fractions while they turn).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dial {
    volume: f64,
    cutoff: f64,
    res: f64,
    env: f64,
    decay: f64,
}

impl Dial {
    /// `track` as it sounds untouched.
    fn of(track: u8) -> Self {
        // The 303s untouched are soft and gentle — little resonance, a
        // mild, rounder envelope; the scroll text turns them up to acid.
        let env = if track == ACID_HIGH { 20.0 } else { 35.0 };
        Dial { volume: 100.0, cutoff: 15.0, res: 30.0, env, decay: 60.0 }
    }

    /// This, with the knobs `knobs` names turned to their settings.
    fn turned(self, knobs: Knobs) -> Self {
        let to = |k: Option<u8>, v: f64| k.map_or(v, f64::from);
        Dial {
            volume: to(knobs.volume, self.volume),
            cutoff: to(knobs.cutoff, self.cutoff),
            res: to(knobs.res, self.res),
            env: to(knobs.env, self.env),
            decay: to(knobs.decay, self.decay),
        }
    }

    fn towards(self, to: Dial, p: f64) -> Self {
        let mix = |a: f64, b: f64| a + (b - a) * p;
        Dial {
            volume: mix(self.volume, to.volume),
            cutoff: mix(self.cutoff, to.cutoff),
            res: mix(self.res, to.res),
            env: mix(self.env, to.env),
            decay: mix(self.decay, to.decay),
        }
    }
}

/// A track's knobs turning from `from` to `to`, over the ticks from
/// `start` to `start + len`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Turn {
    from: Dial,
    to: Dial,
    start: u64,
    len: u64,
}

impl Turn {
    fn still(dial: Dial) -> Self {
        Turn { from: dial, to: dial, start: 0, len: 0 }
    }

    /// Where the knobs stand at tick `now`: easing in and out.
    fn at(&self, now: u64) -> Dial {
        if self.len == 0 || now >= self.start + self.len {
            return self.to;
        }
        let p = now.saturating_sub(self.start) as f64 / self.len as f64;
        self.from.towards(self.to, p * p * (3.0 - 2.0 * p))
    }
}

/// Whether `track` is a 303, with the filter knobs.
pub fn has_filter_knobs(track: u8) -> bool {
    track == ACID || track == ACID_HIGH
}

/// A launch or a stop, from a code or the opening.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Launch {
    /// A loop, with its knobs.
    Play(usize, Knobs),
    Stop(Option<u8>),
    /// Stop the track if it's playing this loop.
    StopLoop(usize),
}

/// What a track plays: the loop and the tick it started.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Track {
    clip: usize,
    launch: u64,
}

/// Per track, what it plays.
type Playing = [Option<Track>; TRACKS.len()];

/// The scroll text's loop launches, each on the bar after its code fires.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// (tick it takes effect, launch), in order.
    launches: Vec<(u64, Launch)>,
    /// (tick its code fired, tick it takes effect, tracks it touches) for
    /// each coded launch: between the two it's waiting for its bar.
    waits: Vec<(u64, u64, Vec<u8>)>,
}

/// Which tracks are muted, a bit per track (index into `TRACKS`): shared
/// by the keys that toggle them, the strip that shows them and the music
/// that leaves them out.
pub type Mutes = Arc<AtomicU16>;

/// The tracks in the sequencer strip, in order — those with loops. Keys
/// 1, 2, 3… toggle them.
pub fn strip_tracks() -> Vec<u8> {
    (0..TRACKS.len() as u8).filter(|&t| CLIPS.iter().any(|c| c.track == t)).collect()
}

/// One track in the sequencer strip.
#[derive(Debug, Clone, PartialEq)]
pub struct StripTrack {
    pub name: &'static str,
    /// The loop it plays — its name, or its number on the track — and how
    /// far through it (0–1).
    pub playing: Option<(String, f64)>,
    /// A launch or stop has fired and waits for the next bar.
    pub waiting: bool,
    /// The 303s' cutoff knob, 0–100.
    pub cutoff: Option<f64>,
    /// Muted from the keyboard.
    pub muted: bool,
}

/// The sequencer strip: the transport, and every track with loops.
#[derive(Debug, Clone, PartialEq)]
pub struct Strip {
    /// Bar, beat and sixteenth, from 1, as Live counts.
    pub position: (u64, u64, u64),
    pub tracks: Vec<StripTrack>,
}

impl Session {
    /// The launches in `timeline`'s codes, after the opening's.
    pub fn new(timeline: &Timeline) -> Self {
        let opening = OPENING.iter().map(|&(t, w, secs)| {
            let at = (secs * TICKS as f64).ceil() as u64;
            (at.div_ceil(BAR) * BAR, Launch::Play(clip_named(t, w).expect("opening loop"), Knobs::default()))
        });
        let mut waits = Vec::new();
        let coded: Vec<(u64, Launch)> = timeline
            .cues
            .iter()
            .filter_map(|cue| {
                let (launch, tracks) = match cue.code {
                    Code::Play(clip, knobs) => (Launch::Play(clip, knobs), vec![CLIPS[clip].track]),
                    Code::Stop(Some(track)) => (Launch::Stop(Some(track)), vec![track]),
                    Code::Stop(None) => (Launch::Stop(None), (0..TRACKS.len() as u8).collect()),
                    Code::StopLoop(clip) => (Launch::StopLoop(clip), vec![CLIPS[clip].track]),
                    _ => return None,
                };
                // The next bar line at or after the code.
                let fired = (cue.t * TICKS as f64).ceil() as u64;
                let at = fired.div_ceil(BAR) * BAR;
                waits.push((fired, at, tracks));
                Some((at, launch))
            })
            .collect();
        let mut launches: Vec<(u64, Launch)> = opening.chain(coded).collect();
        launches.sort_by_key(|l| l.0);
        Session { launches, waits }
    }

    /// The sequencer strip `secs` into the script, with `mutes` (a bit per
    /// track).
    pub fn strip(&self, secs: f64, mutes: u16) -> Strip {
        let now = (secs.max(0.0) * TICKS as f64) as u64;
        let (playing, dials) = self.at(now);
        let per_beat = per_beat() as u64;
        let position = (now / BAR + 1, now % BAR / per_beat + 1, now % per_beat / (per_beat / 4) + 1);
        let tracks = strip_tracks()
            .into_iter()
            .map(|t| t as usize)
            .map(|t| {
                let playing = playing[t].filter(|p| p.launch <= now).map(|p| {
                    let clip = CLIPS[p.clip];
                    let number = CLIPS[..p.clip].iter().filter(|c| c.track == clip.track).count() + 1;
                    let name = if clip.name.is_empty() { number.to_string() } else { clip.name.to_string() };
                    let length = ticks(clip.beats).max(1);
                    (name, ((now - p.launch) % length) as f64 / length as f64)
                });
                StripTrack {
                    name: TRACK_NAMES[t],
                    playing,
                    waiting: self.waits.iter().any(|(fired, at, ts)| *fired <= now && now < *at && ts.contains(&(t as u8))),
                    cutoff: has_filter_knobs(t as u8).then_some(dials[t].cutoff),
                    muted: mutes & 1 << t != 0,
                }
            })
            .collect();
        Strip { position, tracks }
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
        self.at((secs * TICKS as f64) as u64).0[track_named(track).unwrap() as usize].map(|p| p.clip)
    }

    /// `track`'s volume (%) `secs` into the script (tests).
    #[cfg(test)]
    pub fn volume_at(&self, secs: f64, track: &str) -> Option<u8> {
        let (playing, dials) = self.at((secs * TICKS as f64) as u64);
        let t = track_named(track).unwrap() as usize;
        playing[t].map(|_| dials[t].volume.round() as u8)
    }

    /// Just these loops, from the start (tests).
    #[cfg(test)]
    fn only(clips: &[usize]) -> Self {
        Session { launches: clips.iter().map(|&c| (0, Launch::Play(c, Knobs::default()))).collect(), waits: Vec::new() }
    }

    /// These loops at these volumes, from the start (tests).
    #[cfg(test)]
    fn at_volumes(clips: &[(usize, u8)]) -> Self {
        Session { launches: clips.iter().map(|&(c, v)| (0, Launch::Play(c, Knobs { volume: Some(v), ..Knobs::default() }))).collect(), waits: Vec::new() }
    }

    /// These loops with these knobs, from the start (tests).
    #[cfg(test)]
    fn with_knobs(clips: &[(usize, Knobs)]) -> Self {
        Session { launches: clips.iter().map(|&(c, k)| (0, Launch::Play(c, k))).collect(), waits: Vec::new() }
    }

    /// What plays at tick `now`, and where every track's knobs stand.
    fn at(&self, now: u64) -> (Playing, [Dial; TRACKS.len()]) {
        let mut playing: Playing = [None; TRACKS.len()];
        let mut turns: [Turn; TRACKS.len()] = std::array::from_fn(|t| Turn::still(Dial::of(t as u8)));
        for &(at, launch) in self.launches.iter().take_while(|l| l.0 <= now) {
            match launch {
                Launch::Play(clip, knobs) => {
                    let t = CLIPS[clip].track as usize;
                    // Knobs belong to the track, and turn — from where they
                    // stand, over `over` bars (a bar unless told) — while it
                    // plays; a track setting off starts with them set.
                    let to = turns[t].to.turned(knobs);
                    turns[t] = match playing[t] {
                        Some(_) => Turn { from: turns[t].at(at), to, start: at, len: knobs.over.unwrap_or(1) as u64 * BAR },
                        None => Turn::still(to),
                    };
                    // Launching what already plays carries on.
                    if playing[t].is_none_or(|p| p.clip != clip) {
                        playing[t] = Some(Track { clip, launch: at });
                    }
                }
                Launch::Stop(Some(t)) => playing[t as usize] = None,
                Launch::StopLoop(clip) => {
                    let track = &mut playing[CLIPS[clip].track as usize];
                    if track.is_some_and(|t| t.clip == clip) {
                        *track = None;
                    }
                }
                Launch::Stop(None) => playing = [None; TRACKS.len()],
            }
        }
        (playing, std::array::from_fn(|t| turns[t].at(now)))
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
    /// Each chip's level, and the 808's, 0–1, as the session has them.
    pub levels: [f32; CHIPS],
    pub kit_level: f32,
    /// Where each track's knobs stand, as the session has them.
    dials: [Dial; TRACKS.len()],
    /// Tracks muted from the keyboard: their loops run on, unheard.
    pub mutes: Mutes,
    /// Times it jumped to the demo, cutting what rang (the first tick counts).
    pub jumps: u32,
}

impl Player {
    pub fn new(session: Session) -> Self {
        Player {
            session,
            now: None,
            held: Vec::new(),
            mallets: 0,
            voices: [[None; 3]; CHIPS],
            strikes: Vec::new(),
            levels: [1.0; CHIPS],
            kit_level: 1.0,
            dials: std::array::from_fn(|t| Dial::of(t as u8)),
            mutes: Mutes::default(),
            jumps: 0,
        }
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
                self.jumps += 1;
                self.held.clear();
                for chip in 0..CHIPS {
                    for v in 0..3 {
                        self.release(sid, (chip, v), 0);
                    }
                }
                demo
            }
        };
        // Drifted a little: nudge back, a tick at a time, unheard.
        let steps = if now > demo + DRIFT {
            0
        } else if now + DRIFT < demo {
            2
        } else {
            1
        };
        for k in 0..steps {
            self.step(sid, now + k);
        }
        self.now = Some(now + steps);
    }

    /// One tick of music at tick `now`: the notes due, the voices' registers.
    fn step(&mut self, sid: &mut impl Regs, now: u64) {
        let (playing, dials) = self.session.at(now);
        self.dials = dials;
        // Notes of loops that stopped, or were swapped, go.
        self.held.retain(|h| h.end > now && playing[h.note.track as usize].is_some_and(|t| (t.clip, t.launch) == (h.clip, h.launch)));
        // Levels: each chip at its track's volume (the pad chip at the
        // louder of the pad's and the bells').
        self.levels = [0.0; CHIPS];
        let mutes = self.mutes.load(Ordering::Relaxed);
        let muted = |track: usize| mutes & 1 << track != 0;
        self.kit_level = if muted(DRUMS as usize) { 0.0 } else { self.kit_level };
        // A muted track's notes go at once (its loop runs on, unheard).
        self.held.retain(|h| !muted(h.note.track as usize));
        for (track, t) in playing.iter().enumerate() {
            if t.is_none() || muted(track) {
                continue;
            }
            let level = (dials[track].volume / 100.0) as f32;
            match chip_of(track as u8) {
                Some(chip) => self.levels[chip] = self.levels[chip].max(level),
                None => self.kit_level = level,
            }
        }
        for (c, launch) in playing.iter().flatten().map(|t| (t.clip, t.launch)) {
            let clip = CLIPS[c];
            let length = ticks(clip.beats).max(1);
            if now < launch {
                continue;
            }
            let phase = (now - launch) % length;
            for &note in clip.notes.iter().filter(|n| ticks(n.at) % length == phase && !muted(n.track as usize)) {
                self.hold(note, c, launch, now);
            }
        }
        self.voices(sid, now);
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
        self.acid(sid, A1, ACID, Patch { wave: SAW, ad: 0x08, sr: 0xA0 }, now);
        // The second 303: quieter — a lower sustain, its filter opening
        // less far.
        self.acid(sid, B1, ACID_HIGH, Patch { wave: SAW, ad: 0x06, sr: 0x50 }, now);
        let marimba = self.keys(|h| h.note.track == MARIMBA, 12);
        self.tone(sid, MARIMBA_SLOT, &marimba, Patch { wave: TRI, ad: 0x08, sr: 0x00 }, now);
        let thud = self.keys(|h| h.note.track == THUD, 0);
        self.tone(sid, THUD_SLOT, &thud, Patch { wave: TRI, ad: 0x09, sr: 0x00 }, now);
        // The mallets, soft: triangles taking turns, each note ringing
        // out under the next.
        for (turn, slot) in MALLET_SLOTS.into_iter().enumerate() {
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

    /// Let every sounding voice go, each keeping its waveform so it rings
    /// out on its own release (a pause: the tails, the echoes and the hall
    /// carry on); the notes are kept, to sound again on going on.
    pub fn let_go(&mut self, sid: &mut impl Regs) {
        let wave = |(chip, v): Slot| match (chip, v) {
            (0, 0) | (1, 0) => SAW,
            (2, _) => PULSE,
            _ => TRI,
        };
        for chip in 0..CHIPS {
            for v in 0..3u8 {
                let slot = (chip, v);
                self.release(sid, slot, wave(slot));
            }
        }
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
    /// snapping open with each note and closing again, as its knobs stand.
    fn acid(&mut self, sid: &mut impl Regs, slot: Slot, track: u8, patch: Patch, now: u64) {
        let chip = slot.0;
        let Some(h) = self.latest(track) else {
            self.release(sid, slot, patch.wave);
            return;
        };
        if self.retrigger(sid, slot, Voice { track, key: h.note.key, start: h.start }) {
            return;
        }
        Self::sound(sid, slot, h.note.key as f64, patch, true);
        let dial = self.dials[track as usize];
        // The knobs, 0–100, onto the chip: the cutoff's floor across its
        // 11 bits, the envelope's reach, its fall in ticks, resonance in 4 bits.
        let floor = dial.cutoff * 0x7FF as f64 / 100.0;
        let reach = dial.env / 100.0 * 0x500 as f64;
        let fall = 3.0 + dial.decay * 0.54;
        let res = (dial.res * 15.0 / 100.0).round() as u8;
        let since = (now - h.start) as f64;
        let vel = h.note.vel as f64 / 127.0;
        let fc = (floor + reach * vel * (-since / fall).exp()).min(0x7FF as f64) as u16;
        sid.poke(chip, FC_LO, (fc & 7) as u8);
        sid.poke(chip, FC_HI, (fc >> 3) as u8);
        // Resonance only: the routing (voice 1 through the filter) never moves.
        sid.poke(chip, RES_FILT, res << 4 | 1 << slot.1);
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
    /// Each chip's level and the 808's as heard, gliding to the player's.
    levels: [f32; CHIPS],
    kit_level: f32,
    read: usize,
    /// Cycles owed to the chips, in 1/TICKS of a cycle.
    carry: u32,
    /// The demo is paused: the sequencer holds, what sounds rings out.
    pub paused: Arc<AtomicBool>,
    was_paused: bool,
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
            levels: [1.0; CHIPS],
            kit_level: 1.0,
            read: 0,
            carry: 0,
            paused: Arc::new(AtomicBool::new(false)),
            was_paused: false,
        }
    }

    /// The next sample, left and right, -1..1.
    pub fn next_stereo(&mut self) -> (f32, f32) {
        while self.read >= self.buffers.iter().map(Vec::len).min().unwrap_or(0) {
            let paused = self.paused.load(Ordering::Relaxed);
            if paused {
                // Held: no ticks, no new notes; what sounded rings out.
                if !self.was_paused {
                    self.player.let_go(&mut self.sids);
                }
            } else {
                let demo = f64::from_bits(self.clock.load(Ordering::Relaxed));
                self.player.tick(&mut self.sids, demo);
            }
            self.was_paused = paused;
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
        for (chip, buffer) in self.buffers.iter().enumerate() {
            self.levels[chip] += (self.player.levels[chip] - self.levels[chip]) / LEVEL_GLIDE;
            let s = buffer[self.read] as f32 / 32_768.0 * 2.0 * self.levels[chip];
            l += s * PAN[chip].0;
            r += s * PAN[chip].1;
            send += s * SEND[chip];
        }
        self.kit_level += (self.player.kit_level - self.kit_level) / LEVEL_GLIDE;
        let kit = self.kit.next() * KIT * self.kit_level;
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
pub fn play(session: Session, clock: Arc<AtomicU64>, mutes: Mutes, paused: Arc<AtomicBool>) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config: cpal::StreamConfig = supported.config();
    let channels = config.channels as usize;
    let mut synth = Synth::new(config.sample_rate.0, session, clock);
    synth.player.mutes = mutes;
    synth.paused = paused;
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
        let cue = |n: usize| tl.cues.iter().filter(|c| matches!(c.code, Code::Play(..) | Code::Stop(_))).nth(n).unwrap().t;
        let tick = |t: f64| (t * TICKS as f64).ceil() as u64;
        let bar_after = |t: f64| tick(t).div_ceil(BAR) * BAR;
        let bass = THUD as usize;
        // The opening pad from the start; the bass not until the bar after its code.
        assert!(session.at(0).0[PAD as usize].is_some());
        let launch = bar_after(cue(0));
        assert!(launch >= tick(cue(0)) && launch % BAR == 0);
        assert!(session.at(launch - 1).0[bass].is_none());
        assert_eq!(session.at(launch).0[bass].map(|t| (t.clip, t.launch)), Some((first("bass"), launch)));
        // Drums come in later, bass carries on; then the bass stops; then all.
        let drums = bar_after(cue(1));
        assert_eq!(session.at(drums).0[DRUMS as usize].map(|p| CLIPS[p.clip].name), Some("full"));
        assert_eq!(session.at(drums).0[bass].map(|t| (t.clip, t.launch)), Some((first("bass"), launch)));
        assert!(session.at(bar_after(cue(2))).0[bass].is_none());
        assert!(session.at(bar_after(cue(3))).0.iter().all(Option::is_none));
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
        // Small wobble: no jump — nothing cut — it catches up a tick...
        let rang = p.held.clone();
        p.tick(&mut book, 300.0 / TICKS as f64 + 0.03);
        assert_eq!(p.now, Some(302));
        assert!(rang.iter().all(|h| h.end <= 302 || p.held.contains(h)), "notes cut by a wobble");
        // ...a skip: it jumps, and what rang is cut.
        p.tick(&mut book, 40.0);
        assert_eq!(p.now, Some(40 * TICKS as u64 + 1));
        assert!(p.held.iter().all(|h| h.start == 40 * TICKS as u64));
    }

    #[test]
    fn a_demo_running_slow_never_makes_the_music_jump() {
        // The demo dropping frames — its clock 3 % slow — against steady
        // music: nudged along, never a jump, never more than a few ticks out.
        let mut p = Player::new(Session::only(&[first("bass"), first("pad"), clip_named("drums", Some("full")).unwrap()]));
        let mut book = Book::new();
        for t in 0..TICKS as u64 * 20 {
            let demo = t as f64 * 0.97 / TICKS as f64;
            p.tick(&mut book, demo);
            let now = p.now.unwrap();
            let target = (demo * TICKS as f64).round() as u64;
            assert!(now.abs_diff(target) <= DRIFT + 2, "{now} vs {target} at {t}");
        }
        // Only the very first tick counts as a jump: nothing was ever cut.
        assert_eq!(p.jumps, 1, "the music jumped, cutting what rang");
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
    fn volume_turns_a_track_down_and_a_relaunch_only_changes_it() {
        let (session, tl) = session_of("ab{play mallets volume:20}cdefghijklmnopqrstuvwxyz{play mallets volume:100 over:0}abc");
        let cue = |n: usize| tl.cues.iter().filter(|c| matches!(c.code, Code::Play(..))).nth(n).unwrap().t;
        assert_eq!(session.volume_at(cue(0) + 2.1, "mallets"), Some(20));
        // Played again: back to full, the loop carrying on from where it was.
        let later = cue(1) + 2.1;
        assert_eq!(session.volume_at(later, "mallets"), Some(100));
        assert_eq!(session.at((later * TICKS as f64) as u64).0[MALLETS as usize].map(|t| t.launch), session.at(((cue(0) + 2.1) * TICKS as f64) as u64).0[MALLETS as usize].map(|t| t.launch));
        // And it sounds that much quieter.
        let level = |volume| {
            let time = clock();
            let mut synth = Synth::new(RATE, Session::at_volumes(&[(first("mallets"), volume)]), time.clone());
            let s: Vec<f32> = (0..RATE as usize * 8).map(|i| {
                time.store((i as f64 / RATE as f64).to_bits(), Ordering::Relaxed);
                let (l, r) = synth.next_stereo();
                (l + r) / 2.0
            }).collect();
            rms(&s[RATE as usize..])
        };
        let (full, low) = (level(100), level(20));
        assert!((0.12..0.3).contains(&(low / full)), "20 % came out at {:.0} %", 100.0 * low / full);
    }

    #[test]
    fn stopping_a_loop_stops_its_track_only_if_it_plays_that_loop() {
        let two = clip_named("303", Some("2")).unwrap();
        // Playing loop 2: stopped.
        let (session, tl) = session_of("ab{play 303 2}cdefghijklmnopqrstuvwxyz{stop 303 2}abcdefghijklmnopqrstuvwxyz");
        let last = tl.cues.iter().filter(|c| matches!(c.code, Code::StopLoop(_))).next().unwrap().t;
        assert_eq!(session.loop_at(last - 0.1, "303"), Some(two));
        assert_eq!(session.loop_at(last + 2.1, "303"), None);
        // Playing loop 1: carries on.
        let (session, tl) = session_of("ab{play 303}cdefghijklmnopqrstuvwxyz{stop 303 2}abcdefghijklmnopqrstuvwxyz");
        let last = tl.cues.iter().filter(|c| matches!(c.code, Code::StopLoop(_))).next().unwrap().t;
        assert_eq!(session.loop_at(last + 2.1, "303"), Some(first("303")));
        assert!(script::parse("{stop 303 9}").is_err());
    }

    #[test]
    fn the_303_knobs_turn_its_filter() {
        // Parsed from the scroll text...
        let code = script::parse("{play 303 cutoff:30 res:90 env:20 decay:70}").unwrap().events[0].1;
        let knobs = Knobs { cutoff: Some(30), res: Some(90), env: Some(20), decay: Some(70), ..Knobs::default() };
        assert_eq!(code, Code::Play(first("303"), knobs));
        for bad in ["{play pad cutoff:30}", "{play 303 wobble:3}", "{play 303 cutoff:300}"] {
            assert!(script::parse(bad).is_err(), "{bad}");
        }
        // ...onto the chip: a higher cutoff sits higher once each note's
        // envelope has closed; resonance in the top nibble.
        let filter = |knobs: Knobs| {
            let acid = first("303");
            let mut p = Player::new(Session::with_knobs(&[(acid, knobs)]));
            let mut book = Book::new();
            for t in 0..BAR {
                p.tick(&mut book, t as f64 / TICKS as f64);
            }
            let regs = book.0[0];
            ((regs[FC_HI as usize] as u16) << 3 | regs[FC_LO as usize] as u16 & 7, regs[RES_FILT as usize] >> 4)
        };
        let closed = Knobs { cutoff: Some(5), env: Some(0), ..Knobs::default() };
        let open = Knobs { cutoff: Some(60), env: Some(0), res: Some(90), ..Knobs::default() };
        let (lo, hi) = (filter(closed), filter(open));
        assert!(hi.0 > lo.0 + 500, "cutoff {lo:?} vs {hi:?}");
        assert_eq!((lo.1, hi.1), (5, 14));
        // Untouched knobs: soft and gentle, little resonance.
        assert_eq!(filter(Knobs::default()).1, 5);
    }

    #[test]
    fn knobs_turn_gradually_over_their_bars() {
        let (session, tl) = session_of("ab{play 303}cdefghijklmnopqrstuvwxyz{play 303 cutoff:80 over:4}abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz{play 303 res:20}abcdefghijklmnopqrstuvwxyzabc");
        let cues: Vec<f64> = tl.cues.iter().filter(|c| matches!(c.code, Code::Play(..))).map(|c| c.t).collect();
        let tick = |t: f64| (t * TICKS as f64).ceil() as u64;
        let turn = tick(cues[1]).div_ceil(BAR) * BAR;
        let cutoff = |at: u64| session.at(at).1[ACID as usize].cutoff;
        let untouched = Dial::of(ACID).cutoff;
        // Where it was as the turn starts; half-way at half-way; there after 4 bars.
        assert!((cutoff(turn) - untouched).abs() < 1e-9);
        assert!((cutoff(turn + 2 * BAR) - (untouched + 80.0) / 2.0).abs() < 1e-9);
        assert!((cutoff(turn + 4 * BAR) - 80.0).abs() < 1e-9);
        // Every step on the way a small one.
        assert!((turn..turn + 4 * BAR).all(|t| (cutoff(t + 1) - cutoff(t)).abs() < 0.2));
        // Another knob later: the cutoff stays where it was turned to, the
        // resonance turns over a bar.
        let later = tick(cues[2]).div_ceil(BAR) * BAR;
        let dial = |at: u64| session.at(at).1[ACID as usize];
        assert!((dial(later + BAR).cutoff - 80.0).abs() < 1e-9);
        assert!((dial(later).res - Dial::of(ACID).res).abs() < 1e-9 && (dial(later + BAR).res - 20.0).abs() < 1e-9);
    }

    #[test]
    fn the_strip_shows_what_plays_what_waits_and_where_we_are() {
        let (session, tl) = session_of("ab{play bass}cdefghijklmnopqrstuvwxyz{play 303 cutoff:60 over:0}abc");
        let cue = |n: usize| tl.cues.iter().filter(|c| matches!(c.code, Code::Play(..))).nth(n).unwrap().t;
        let track = |strip: &Strip, name: &str| strip.tracks.iter().find(|t| t.name == name).cloned().unwrap();
        // The opening pad plays from the start, its first loop.
        let start = session.strip(0.1, 0);
        assert_eq!(start.position, (1, 1, 1));
        assert_eq!(track(&start, "pad").playing.map(|p| p.0), Some("1".to_string()));
        // The bass's code has fired: it waits for the bar, then plays.
        let fired = session.strip(cue(0) + 0.01, 0);
        assert!(track(&fired, "bass").waiting && track(&fired, "bass").playing.is_none());
        let bar = (cue(0) * TICKS as f64).ceil() as u64;
        let bar = bar.div_ceil(BAR) * BAR;
        let on = session.strip(bar as f64 / TICKS as f64, 0);
        assert!(!track(&on, "bass").waiting);
        assert_eq!(track(&on, "bass").playing.map(|p| p.1), Some(0.0));
        assert_eq!(on.position.1, 1, "a launch lands on beat 1");
        // The 303's cutoff knob shows.
        let later = session.strip(cue(1) + 2.5, 0);
        assert!((track(&later, "303").cutoff.unwrap() - 60.0).abs() < 1e-9);
        // The toy track has no loops: not in the strip.
        assert!(later.tracks.iter().all(|t| t.name != "toy"));
    }

    #[test]
    fn a_muted_track_goes_quiet_and_comes_back_in_time() {
        let bass = first("bass");
        let mut p = Player::new(Session::only(&[bass]));
        let mut book = Book::new();
        let mute = |p: &Player, on: bool| p.mutes.store(if on { 1 << THUD } else { 0 }, Ordering::Relaxed);
        mute(&p, true);
        let length = ticks(CLIPS[bass].beats);
        for t in 0..length {
            p.tick(&mut book, t as f64 / TICKS as f64);
            assert!(p.held.iter().all(|h| h.note.track != THUD) && p.levels[5] == 0.0, "the bass plays muted at {t}");
        }
        // Unmuted: back on its next note, in its loop's time.
        mute(&p, false);
        let next = CLIPS[bass].notes[0];
        for t in length..length + ticks(next.at) + 1 {
            p.tick(&mut book, t as f64 / TICKS as f64);
        }
        assert!(p.held.iter().any(|h| h.note == next && h.start == length + ticks(next.at)));
        // The strip numbers eight tracks for the keys, and shows the mute.
        assert_eq!(strip_tracks().len(), 8);
        let strip = Session::only(&[bass]).strip(1.0, 1 << THUD);
        assert!(strip.tracks.iter().find(|t| t.name == "bass").unwrap().muted);
    }

    #[test]
    fn a_pause_holds_the_sequencer_and_lets_the_tails_ring_out() {
        let time = clock();
        let all: Vec<usize> = (0..CLIPS.len()).filter(|&i| CLIPS[i].track != DRUMS || CLIPS[i].name == "full").collect();
        let mut synth = Synth::new(RATE, Session::only(&all), time.clone());
        let play = |synth: &mut Synth, from: usize, n: usize| -> Vec<f32> {
            (from..from + n)
                .map(|i| {
                    time.store((i as f64 / RATE as f64).to_bits(), Ordering::Relaxed);
                    let (l, r) = synth.next_stereo();
                    (l + r) / 2.0
                })
                .collect()
        };
        let second = RATE as usize;
        play(&mut synth, 0, 3 * second);
        let now = synth.player.now;
        // Paused, the demo's clock stopped: the music doesn't move on...
        synth.paused.store(true, Ordering::Relaxed);
        time.store((3.0f64).to_bits(), Ordering::Relaxed);
        let tail: Vec<f32> = (0..3 * second)
            .map(|_| {
                let (l, r) = synth.next_stereo();
                (l + r) / 2.0
            })
            .collect();
        assert_eq!(synth.player.now, now, "the sequencer moved on in a pause");
        // ...but it doesn't cut out: the first moments still sound, dying
        // away, and in the end it's quiet.
        let (first, last) = (rms(&tail[..second / 4]), rms(&tail[2 * second..]));
        assert!(first > 0.01, "cut dead: {first}");
        assert!(last < first / 4.0, "rings on and on: {first} then {last}");
        // On again: from where it was.
        synth.paused.store(false, Ordering::Relaxed);
        play(&mut synth, 3 * second, second / 10);
        assert!(synth.player.now.unwrap() - now.unwrap() < TICKS as u64 / 5);
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
