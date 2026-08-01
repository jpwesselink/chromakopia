//! CHROMAKOPIA letters drop in from above and bounce into place.
//!
//! Per-letter plasma using a shared seed: a single Plasma is rendered over
//! the buffer with all letters at their REST positions, then each letter's
//! cells are copied from the rest position to the current bouncing position.
//! Result: the plasma pattern travels with each letter as it bounces.

use chromakopia::animate::{
    framebuffer::{secs_to_frames, Cell, DEFAULT_TEXT_COLOR},
    AlphaIn, Easing, Effect, FrameBuffer, Glow, Neon, Plasma, Radar, Scene, Solid, Timeline,
    Wind,
};
use chromakopia::gradient;

const BANNER: &str = "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

const LICENSE: &str = "MIT License · Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.";


const STAGGER: f64 = 0.18;
const FALL_SECS: f64 = 2.4;       // longer = bounces have more frames to be seen
const TOP_MARGIN: i32 = 8;        // empty rows above the rest position — gives big bounces room
const TOTAL_SECS: f64 = 14.0;
const PLASMA_SEED: f64 = 7.0;
/// How much to amplify the bounce dips relative to vanilla BounceOut.
/// 1.0 = vanilla (small bounces), 2.0 = first bounce ~50% of fall distance,
/// 2.5 = even more dramatic. Values above ~2.5 will clip into TOP_MARGIN.
const BOUNCE_AMP: f64 = 2.0;
/// Global horizontal scroll: the whole banner slides from off-screen right
/// into its final position over `SLIDE_SECS`. Independent of per-letter bounces.
const SLIDE_SECS: f64 = 3.5;
/// Background scroll starts after the foreground banner has settled, so
/// the bg doesn't compete during the dramatic entrance.
const BG_APPEAR: f64 = 4.0;
/// Background color phase boundaries — bg cycles fade-in → glow → neon → zebra → rainbow.
const BG_AFTER_GLOW: f64 = BG_APPEAR + 3.0;   // 7.0  — glow phase ends
const BG_AFTER_NEON: f64 = BG_APPEAR + 5.0;   // 9.0  — neon phase ends
const BG_AFTER_ZEBRA: f64 = BG_APPEAR + 7.0;  // 11.0 — zebra phase ends
/// Closing arc: license slides in, holds for 3s, then wind takes both.
const LICENSE_APPEAR: f64 = 6.0;
const LICENSE_SLIDE_SECS: f64 = 2.0;
const WIND_START: f64 = 11.0; // 3s after the license has settled (8 + 3)
const WIND_SECS: f64 = 3.0;

/// Slice the banner into per-letter (start_column, art) pairs by detecting
/// columns where every row is whitespace.
fn split_banner(banner: &str) -> Vec<(usize, String)> {
    let lines: Vec<Vec<char>> = banner.lines().map(|l| l.chars().collect()).collect();
    let max_w = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let padded: Vec<Vec<char>> = lines
        .iter()
        .map(|l| {
            let mut v = l.clone();
            v.resize(max_w, ' ');
            v
        })
        .collect();

    let is_blank_col = |c: usize| padded.iter().all(|line| line[c] == ' ');

    let mut letters = Vec::new();
    let mut start: Option<usize> = None;
    for c in 0..max_w {
        match (is_blank_col(c), start) {
            (false, None) => start = Some(c),
            (true, Some(s)) => {
                let art = padded
                    .iter()
                    .map(|line| line[s..c].iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n");
                letters.push((s, art));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        let art = padded
            .iter()
            .map(|line| line[s..].iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        letters.push((s, art));
    }
    letters
}

/// One effect that owns all letters + the shared plasma. Renders by:
///   1) writing every letter's chars at REST position into a scratch buffer
///   2) coloring that scratch with a single Plasma render
///   3) copying each letter's cells from rest position to current bouncing position
///
/// Letters start at `start_x` (off-screen right) and `start_y = -glyph_h`,
/// then bounce-slide diagonally to (rest_x, rest_y). Same easing on both axes.
struct BouncingBanner {
    /// (rest_x_in_buffer, glyph_art) per letter.
    letters: Vec<(usize, Vec<Vec<char>>)>,
    rest_y: i32,
    fall_frames: usize,
    stagger_frames: usize,
    bounce_amp: f64,
    /// Global horizontal slide: at frame 0 the whole banner is offset by
    /// `slide_distance` cells to the right, then eases to 0 over `slide_frames`.
    slide_distance: i32,
    slide_frames: usize,
    slide_easing: Easing,
    plasma: Plasma,
    /// 0.0 = opaque plasma stamp (default), >0 = alpha-only mode where the
    /// chromakopia letter shape DIMS the underlying buffer cell rather than
    /// drawing itself. 1.0 = full black at letter positions.
    outline_dim: f64,
    /// Halo: cells within `halo_radius` cells of any letter cell get their
    /// existing color dimmed by `halo_dim` (toward black). 0 disables.
    /// The halo only visibly affects cells that already have content
    /// (e.g. license cells underneath/around the banner) — it doesn't draw
    /// anything new.
    halo_radius: i32,
    halo_dim: f64,
}

/// Scale a multi-line glyph by repeating each char `scale_x` times across
/// and each line `scale_y` times down. Pads to `target_width` columns
/// (centered) so all scaled letters line up the same width.
fn scale_letter(art: &str, scale_x: usize, scale_y: usize, target_width: usize) -> String {
    let scaled_lines: Vec<String> = art
        .lines()
        .flat_map(|line| {
            let scaled: String = line
                .chars()
                .flat_map(|ch| std::iter::repeat(ch).take(scale_x))
                .collect();
            std::iter::repeat(scaled).take(scale_y).collect::<Vec<_>>()
        })
        .collect();
    let max_w = scaled_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let lpad = if target_width > max_w { (target_width - max_w) / 2 } else { 0 };
    scaled_lines
        .iter()
        .map(|l| {
            let cur = l.chars().count();
            let rpad = target_width.saturating_sub(lpad + cur);
            format!("{}{}{}", " ".repeat(lpad), l, " ".repeat(rpad))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Vertical sine-wave displacement: each column shifts UP/DOWN by an amount
/// driven by `sin(x * frequency + t * speed)`. Each character bobs up/down
/// as it passes a fixed buffer column.
struct VSineWave {
    inner: Box<dyn Effect>,
    amplitude: f64,
    frequency: f64,
    speed: f64,
}

impl VSineWave {
    fn new(inner: impl Effect) -> Self {
        Self {
            inner: Box::new(inner),
            amplitude: 4.0,
            frequency: 0.15,
            speed: 0.05,
        }
    }
    fn amplitude(mut self, v: f64) -> Self { self.amplitude = v; self }
    fn frequency(mut self, v: f64) -> Self { self.frequency = v; self }
    fn speed(mut self, v: f64) -> Self { self.speed = v; self }
}

impl Effect for VSineWave {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut scratch = FrameBuffer::new(buf.width, buf.height);
        self.inner.render(&mut scratch, frame);
        let t = frame as f64 * self.speed;
        for x in 0..scratch.width {
            let wave = (x as f64 * self.frequency + t).sin();
            let dy = (wave * self.amplitude).round() as i32;
            for y in 0..scratch.height {
                let cell = scratch.get(x, y);
                if cell.ch == ' ' { continue; }
                let dest_y = y as i32 + dy;
                if dest_y < 0 || dest_y as usize >= buf.height { continue; }
                if buf.get(x, dest_y as usize).ch == ' ' {
                    buf.set(x, dest_y as usize, cell);
                }
            }
        }
    }
}

/// Horizontal background scroll: source content slides RIGHT → LEFT through
/// the buffer over time. Only writes to empty cells.
struct HBackgroundScroll {
    chars: Vec<Vec<char>>,
    /// Cols per frame. >0 = content scrolls left.
    speed: f64,
    color: chromakopia::Color,
    /// Top row of the source within the buffer.
    y_offset: i32,
}

impl HBackgroundScroll {
    fn new(text: &str, speed: f64, color: chromakopia::Color, y_offset: i32) -> Self {
        let mut chars: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let max_w = chars.iter().map(|l| l.len()).max().unwrap_or(0);
        for l in chars.iter_mut() { l.resize(max_w, ' '); }
        Self { chars, speed, color, y_offset }
    }
}

impl Effect for HBackgroundScroll {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let source_w = self.chars.first().map(|l| l.len()).unwrap_or(0) as i32;
        let source_h = self.chars.len() as i32;
        let buf_w = buf.width as i32;
        let offset = (frame as f64 * self.speed).round() as i32;

        for src_y in 0..source_h {
            let dst_y = self.y_offset + src_y;
            if dst_y < 0 || dst_y as usize >= buf.height { continue; }
            for buf_x in 0..buf.width {
                // At frame 0: source col 0 enters at buffer right edge.
                let src_x = buf_x as i32 - buf_w + 1 + offset;
                if src_x < 0 || src_x >= source_w { continue; }
                let ch = self.chars[src_y as usize][src_x as usize];
                if ch == ' ' { continue; }
                if buf.get(buf_x, dst_y as usize).ch == ' ' {
                    buf.set(buf_x, dst_y as usize, Cell::new(ch, self.color));
                }
            }
        }
    }
}

/// Compound animation:
///   • OUTER: the whole text block bounces from above the buffer to its
///     target row as a single rigid unit (block_easing applied to one t).
///   • INNER: at the same time, each line independently slides in horizontally
///     from off-screen left or right (alternating), staggered line-by-line.
///
/// Multi-phase color transitions using a SINGLE inner-effect frame counter,
/// so the same effect spans phase boundaries without the visible "jump" you
/// get when stacking separate Transition tracks (each of which resets the
/// inner effect's frame to 0 at track-window start).
///
/// `phases` is a list of `(start_frame, effect)` — effects activate at their
/// start_frame, and a `fade_frames`-long crossfade blends the previous phase
/// into the active one. All inner effects are called with the GLOBAL frame.
struct PhaseTransitions {
    phases: Vec<(usize, Box<dyn Effect>)>,
    fade_frames: usize,
}

impl Effect for PhaseTransitions {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        if self.phases.is_empty() { return; }
        // Find the latest phase whose start_frame <= frame.
        let mut active_idx = 0;
        for (i, (start, _)) in self.phases.iter().enumerate() {
            if frame >= *start { active_idx = i; }
        }
        let (start, ref active_effect) = self.phases[active_idx];
        // Render active phase at GLOBAL frame.
        active_effect.render(buf, frame);

        // Crossfade with the previous phase during the fade window.
        if active_idx > 0 && frame < start + self.fade_frames {
            let t = (frame - start) as f64 / self.fade_frames.max(1) as f64;
            let prev_effect = &self.phases[active_idx - 1].1;
            // Copy current buf into scratch so prev_effect renders against
            // the same underlying content (cell chars).
            let mut prev_buf = FrameBuffer::new(buf.width, buf.height);
            for y in 0..buf.height {
                for x in 0..buf.width {
                    let mut cell = buf.get(x, y);
                    // Restore default color so prev_effect colors from a clean slate.
                    cell.color = chromakopia::animate::framebuffer::DEFAULT_TEXT_COLOR;
                    prev_buf.set(x, y, cell);
                }
            }
            prev_effect.render(&mut prev_buf, frame);
            // Lerp prev → active.
            for y in 0..buf.height {
                for x in 0..buf.width {
                    if buf.get(x, y).ch == ' ' { continue; }
                    let prev_c = prev_buf.get(x, y).color;
                    let cur_c = buf.get(x, y).color;
                    buf.set_color(x, y, chromakopia::Color::lerp_rgb(prev_c, cur_c, t));
                }
            }
        }
    }
}

/// The two axes are independent — the block keeps bouncing vertically while
/// each line continues sliding into its column position.
struct CompoundLicenseBounce {
    chars: Vec<Vec<char>>,
    target_x: i32,
    target_y: i32,
    block_duration: usize,
    block_easing: Easing,
    line_duration: usize,
    line_stagger: usize,
    line_easing: Easing,
    color: chromakopia::Color,
    alternate_sides: bool,
    /// Plasma applied to the license cells AFTER they're written.
    /// Done internally so it doesn't leak onto BG cells.
    plasma: Option<Plasma>,
    /// Halo dimming around license cells. Same semantics as BouncingBanner's halo.
    halo_radius: i32,
    halo_dim: f64,
}

impl CompoundLicenseBounce {
    fn new(
        text: &str,
        target_x: i32,
        target_y: i32,
        block_seconds: f64,
        block_easing: Easing,
        line_seconds: f64,
        line_stagger: usize,
        line_easing: Easing,
        color: chromakopia::Color,
        alternate_sides: bool,
    ) -> Self {
        Self {
            chars: text.split('\n').map(|l| l.chars().collect()).collect(),
            target_x,
            target_y,
            block_duration: secs_to_frames(block_seconds),
            block_easing,
            line_duration: secs_to_frames(line_seconds),
            line_stagger,
            line_easing,
            color,
            alternate_sides,
            plasma: None,
            halo_radius: 0,
            halo_dim: 0.0,
        }
    }

    fn plasma(mut self, p: Plasma) -> Self { self.plasma = Some(p); self }
    fn halo(mut self, radius: i32, dim: f64) -> Self {
        self.halo_radius = radius;
        self.halo_dim = dim;
        self
    }
}

impl Effect for CompoundLicenseBounce {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Outer: whole block bounces vertically.
        let bt = (frame as f64 / self.block_duration.max(1) as f64).min(1.0);
        let block_eased = self.block_easing.apply(bt);
        let block_start_y = -(self.chars.len() as i32);
        let block_y = (block_start_y as f64
            + (self.target_y - block_start_y) as f64 * block_eased)
            .round() as i32;

        // First pass: compute every license cell position (with horizontal slide).
        let mut positions: Vec<(i32, i32)> = Vec::new();
        let mut line_state: Vec<(i32, i32)> = Vec::with_capacity(self.chars.len());
        for (idx, line) in self.chars.iter().enumerate() {
            let cy = block_y + idx as i32;
            let line_frame = frame.saturating_sub(idx * self.line_stagger);
            let lt = (line_frame as f64 / self.line_duration.max(1) as f64).min(1.0);
            let line_eased = self.line_easing.apply(lt);
            let from_right = self.alternate_sides && (idx % 2 == 1);
            let start_x: i32 = if from_right {
                buf.width as i32
            } else {
                -(line.len() as i32)
            };
            let cx = (start_x as f64 + (self.target_x - start_x) as f64 * line_eased).round() as i32;
            line_state.push((cx, cy));
            for (dx, &ch) in line.iter().enumerate() {
                if ch == ' ' { continue; }
                let x = cx + dx as i32;
                let y = cy;
                positions.push((x, y));
            }
        }

        // Halo pass: dim non-license cells around license positions.
        if self.halo_dim > 0.0 && self.halo_radius > 0 {
            let r = self.halo_radius;
            let dim = self.halo_dim;
            let mut letter_set: std::collections::HashSet<(i32, i32)> =
                std::collections::HashSet::with_capacity(positions.len());
            for p in &positions { letter_set.insert(*p); }
            let mut already_dimmed: std::collections::HashSet<(i32, i32)> =
                std::collections::HashSet::new();
            for (lx, ly) in &positions {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx == 0 && dy == 0 { continue; }
                        let hx = lx + dx;
                        let hy = ly + dy;
                        if hx < 0 || hx as usize >= buf.width { continue; }
                        if hy < 0 || hy as usize >= buf.height { continue; }
                        if letter_set.contains(&(hx, hy)) { continue; }
                        if already_dimmed.contains(&(hx, hy)) { continue; }
                        let existing = buf.get(hx as usize, hy as usize);
                        if existing.ch == ' ' { continue; }
                        let dimmed = chromakopia::Color::lerp_rgb(
                            existing.color,
                            chromakopia::Color::new(0, 0, 0),
                            dim,
                        );
                        buf.set(hx as usize, hy as usize, Cell::new(existing.ch, dimmed));
                        already_dimmed.insert((hx, hy));
                    }
                }
            }
        }

        // Write license cells.
        for ((cx, cy), line) in line_state.iter().zip(self.chars.iter()) {
            let cy = *cy;
            if cy < 0 || cy as usize >= buf.height { continue; }
            for (dx, &ch) in line.iter().enumerate() {
                if ch == ' ' { continue; }
                let x = cx + dx as i32;
                if x < 0 || x as usize >= buf.width { continue; }
                buf.set(x as usize, cy as usize, Cell::new(ch, self.color));
            }
        }

        // Apply plasma to license cells only — render into a scratch with
        // just the license cells, then copy plasma'd cells back.
        if let Some(plasma) = &self.plasma {
            let mut scratch = FrameBuffer::new(buf.width, buf.height);
            for (x, y) in &positions {
                if *x < 0 || *x as usize >= buf.width { continue; }
                if *y < 0 || *y as usize >= buf.height { continue; }
                scratch.set(*x as usize, *y as usize, buf.get(*x as usize, *y as usize));
            }
            plasma.render(&mut scratch, frame);
            for (x, y) in &positions {
                if *x < 0 || *x as usize >= buf.width { continue; }
                if *y < 0 || *y as usize >= buf.height { continue; }
                buf.set(*x as usize, *y as usize, scratch.get(*x as usize, *y as usize));
            }
        }
    }
}

/// BounceOut with the bounce dips amplified. First-bounce height ≈ 25% × amp
/// of total fall distance; subsequent bounces scale similarly.
fn dynamic_bounce(t: f64, amp: f64) -> f64 {
    let raw = Easing::BounceOut.apply(t);
    // The first piece of BounceOut (t < 1/2.75) is the initial fall — leave it
    // alone so the letter doesn't overshoot at startup. After that, amplify
    // the deviation from 1.0 (the rest position).
    if t < 1.0 / 2.75 {
        raw
    } else {
        let dip = 1.0 - raw;
        1.0 - dip * amp
    }
}

impl Effect for BouncingBanner {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // 1) Build the rest buffer: every letter laid out at its final position.
        let mut rest = FrameBuffer::new(buf.width, buf.height);
        for (rx, art) in &self.letters {
            for (dy, line) in art.iter().enumerate() {
                let y = self.rest_y + dy as i32;
                if y < 0 || y as usize >= rest.height { continue; }
                for (dx, &ch) in line.iter().enumerate() {
                    if ch == ' ' { continue; }
                    let x = rx + dx;
                    if x >= rest.width { continue; }
                    rest.set(x, y as usize, Cell::new(ch, DEFAULT_TEXT_COLOR));
                }
            }
        }

        // 2) Color the rest buffer with one plasma pass — single seed, one pattern.
        self.plasma.render(&mut rest, frame);

        // Global horizontal slide: applied to ALL letters, drives the banner
        // from off-screen-right to its final x positions.
        let slide_t = (frame as f64 / self.slide_frames.max(1) as f64).min(1.0);
        let slide_eased = self.slide_easing.apply(slide_t);
        let slide_offset =
            ((1.0 - slide_eased) * self.slide_distance as f64).round() as i32;

        // First pass: compute the set of all letter-cell positions in the
        // buffer at this frame. Used both for halo dimming and so the halo
        // pass doesn't dim cells that are about to be overwritten by letters.
        let mut letter_positions: Vec<(i32, i32)> = Vec::new();
        let mut bounce_state: Vec<(i32, i32)> = Vec::with_capacity(self.letters.len());
        for (i, (rx, art)) in self.letters.iter().enumerate() {
            let local = frame.saturating_sub(i * self.stagger_frames);
            let t = (local as f64 / self.fall_frames.max(1) as f64).min(1.0);
            let eased = dynamic_bounce(t, self.bounce_amp);
            let glyph_h = art.len() as i32;
            let start_y = -glyph_h;
            let cy = (start_y as f64 + (self.rest_y as f64 - start_y as f64) * eased).round() as i32;
            let cx = *rx as i32 + slide_offset;
            bounce_state.push((cx, cy));
            for (dy, line) in art.iter().enumerate() {
                for (dx, &ch) in line.iter().enumerate() {
                    if ch == ' ' { continue; }
                    let dst_x = cx + dx as i32;
                    let dst_y = cy + dy as i32;
                    letter_positions.push((dst_x, dst_y));
                }
            }
        }

        // Halo pass: for each letter cell, dim non-letter cells within
        // halo_radius that already have content. Skipped if halo_dim is 0.
        if self.halo_dim > 0.0 && self.halo_radius > 0 {
            let r = self.halo_radius;
            let dim = self.halo_dim;
            let mut letter_set: std::collections::HashSet<(i32, i32)> =
                std::collections::HashSet::with_capacity(letter_positions.len());
            for p in &letter_positions { letter_set.insert(*p); }

            let mut already_dimmed: std::collections::HashSet<(i32, i32)> =
                std::collections::HashSet::new();
            for (lx, ly) in &letter_positions {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx == 0 && dy == 0 { continue; }
                        let hx = lx + dx;
                        let hy = ly + dy;
                        if hx < 0 || hx as usize >= buf.width { continue; }
                        if hy < 0 || hy as usize >= buf.height { continue; }
                        if letter_set.contains(&(hx, hy)) { continue; }
                        if already_dimmed.contains(&(hx, hy)) { continue; }
                        let existing = buf.get(hx as usize, hy as usize);
                        if existing.ch == ' ' { continue; }
                        let dimmed = chromakopia::Color::lerp_rgb(
                            existing.color,
                            chromakopia::Color::new(0, 0, 0),
                            dim,
                        );
                        buf.set(hx as usize, hy as usize, Cell::new(existing.ch, dimmed));
                        already_dimmed.insert((hx, hy));
                    }
                }
            }
        }

        // Second pass: actually write the letter cells (or apply outline_dim
        // if in alpha-only mode). Reuses the bounce state computed above.
        for ((rx, art), (cx, cy)) in self.letters.iter().zip(bounce_state.iter()) {
            let cx = *cx;
            let cy = *cy;

            for (dy, line) in art.iter().enumerate() {
                let src_y = self.rest_y + dy as i32;
                let dst_y = cy + dy as i32;
                if dst_y < 0 || dst_y as usize >= buf.height { continue; }
                if src_y < 0 || src_y as usize >= rest.height { continue; }

                for (dx, &ch) in line.iter().enumerate() {
                    if ch == ' ' { continue; }
                    let dst_x = cx + dx as i32;
                    if dst_x < 0 || dst_x as usize >= buf.width { continue; }
                    let src_x = rx + dx;
                    if src_x >= rest.width { continue; }

                    if self.outline_dim > 0.0 {
                        // Alpha-only mode: the letter shape dims the underlying
                        // buffer cell instead of replacing it. The chromakopia
                        // type becomes a ghost: license shows through, dimmed.
                        let existing = buf.get(dst_x as usize, dst_y as usize);
                        if existing.ch != ' ' {
                            let dimmed = chromakopia::Color::lerp_rgb(
                                existing.color,
                                chromakopia::Color::new(0, 0, 0),
                                self.outline_dim,
                            );
                            buf.set(dst_x as usize, dst_y as usize,
                                Cell::new(existing.ch, dimmed));
                        }
                    } else {
                        // Default: opaque plasma stamp. Source x stays anchored
                        // to rest_x so the plasma pattern travels with the
                        // letter as it slides.
                        let cell = rest.get(src_x, src_y as usize);
                        buf.set(dst_x as usize, dst_y as usize, cell);
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let fire = gradient(&["#1a1a1a", "#ff69b4", "#00cccc", "#fffacd", "#8b4513", "#1a1a1a"])
        .palette(256);
    // Vivid synthwave palette for the BG plasma phase — deep purple through
    // magenta, cyan, and back. Distinct from the fire palette used on the fg.
    let synth = gradient(&[
        "#1a0a3e", "#7b2d8e", "#ff00aa", "#00ffff", "#ff00ff", "#7b2d8e", "#1a0a3e",
    ]).palette(256);

    let letters_raw = split_banner(BANNER);
    let banner_w = BANNER.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let banner_h = BANNER.lines().count();

    // Background letters from the same rozzo font as the foreground, scaled
    // 4× and stacked HORIZONTALLY (side-by-side) so they scroll right→left.
    const HUGE_SCALE: usize = 4;
    let bg_source: Vec<String> = letters_raw
        .iter()
        .chain(letters_raw.iter())
        .map(|(_, art)| art.clone())
        .collect();
    // Each letter scaled to its own natural width (target_width = its width × scale).
    let scaled_letters: Vec<String> = bg_source
        .iter()
        .map(|art| {
            let w = art.lines().map(|l| l.chars().count()).max().unwrap_or(0) * HUGE_SCALE;
            scale_letter(art, HUGE_SCALE, HUGE_SCALE, w)
        })
        .collect();
    // Horizontal concat row-by-row, with `sep_cols` of space between letters.
    let bg_h = scaled_letters
        .iter()
        .map(|l| l.lines().count())
        .max()
        .unwrap_or(0);
    let sep_cols = 6;
    let mut h_lines: Vec<String> = vec![String::new(); bg_h];
    for (i, letter) in scaled_letters.iter().enumerate() {
        let lines: Vec<&str> = letter.lines().collect();
        let letter_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        for row in 0..bg_h {
            if i > 0 { h_lines[row].push_str(&" ".repeat(sep_cols)); }
            if row < lines.len() {
                let line = lines[row];
                let pad = letter_w.saturating_sub(line.chars().count());
                h_lines[row].push_str(line);
                h_lines[row].push_str(&" ".repeat(pad));
            } else {
                h_lines[row].push_str(&" ".repeat(letter_w));
            }
        }
    }
    let horizontal_source = h_lines.join("\n");
    let h_source_width = h_lines.first().map(|l| l.chars().count()).unwrap_or(0);

    // Lead-in: chunky horizontal bars made of the SAME chars the doh letters
    // use — `:` (interior fill) and cycling CHROMAKOPIA letters as solid
    // bars. Same fabric as the letters → bars and letters read as one
    // coherent surface, with the letters revealing themselves as the
    // patterns slowly resolve into shape.
    let license_w = LICENSE.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let license_h = LICENSE.lines().count();

    let term_w = chromakopia::terminal_width().max(banner_w.max(license_w));
    let left_pad = (term_w - banner_w) / 2;
    let license_pad = (term_w - license_w) / 2;

    // Buffer fits: top margin + banner + 2-row gap + license; AND at least
    // tall enough for the bg letters + sine amplitude on either side so the
    // background type doesn't clip when the wave swings.
    let banner_block_h = TOP_MARGIN as usize + banner_h;
    let fg_h = banner_block_h + 2 + license_h;
    let bg_amplitude_slack: usize = 12; // a bit more than VSineWave amplitude
    let bg_needed_h = bg_h + 2 * bg_amplitude_slack;
    let buf_h = fg_h.max(bg_needed_h);
    let rest_y = TOP_MARGIN;
    let license_target_y = banner_block_h as i32 + 2;

    // Convert (banner_col, art_string) into (target_x, glyph_grid).
    let letters: Vec<(usize, Vec<Vec<char>>)> = letters_raw
        .into_iter()
        .map(|(banner_col, art)| {
            let target_x = left_pad + banner_col;
            let chars = art.split('\n').map(|l| l.chars().collect()).collect();
            (target_x, chars)
        })
        .collect();

    let banner = BouncingBanner {
        letters,
        rest_y,
        fall_frames: secs_to_frames(FALL_SECS),
        stagger_frames: secs_to_frames(STAGGER),
        bounce_amp: BOUNCE_AMP,
        slide_distance: term_w as i32,
        slide_frames: secs_to_frames(SLIDE_SECS),
        slide_easing: Easing::EaseOut,
        plasma: Plasma::new().palette(fire.clone()).seed(PLASMA_SEED),
        outline_dim: 0.0,    // 0 = normal opaque plasma stamps
        halo_radius: 1,      // single-cell halo around each letter
        halo_dim: 0.55,      // 0.0 = no dim, 1.0 = full black; 0.4–0.6 reads as a soft shadow
    };

    // Pace the horizontal background scroll. Single-pass = ONE CHROMAKOPIA
    // worth of source columns; speed picked so a single pass takes scroll_secs.
    let single_pass_cols = h_source_width as f64 / 2.0;
    let scroll_secs = TOTAL_SECS - 1.0;
    let scroll_speed_cols_per_frame = (single_pass_cols + term_w as f64)
        / (scroll_secs * chromakopia::animate::framebuffer::FPS);

    let seed_buf = FrameBuffer::new(term_w, buf_h);
    // Vertically center the bg letters in the buffer.
    let bg_y_offset = ((buf_h as i32 - bg_h as i32) / 2).max(0);
    let timeline = Timeline::from_buffer(seed_buf, TOTAL_SECS)
        // 1) Horizontal background scroll wrapped in VSineWave — each column
        //    gets a vertical sine displacement, so characters bob up/down as
        //    they pass by from right to left. Starts after the banner has
        //    settled so it doesn't compete during the entrance.
        .at(BG_APPEAR..TOTAL_SECS, VSineWave::new(HBackgroundScroll::new(
            &horizontal_source,
            scroll_speed_cols_per_frame,
            chromakopia::Color::new(85, 85, 105),
            bg_y_offset,
        ))
        .amplitude(10.0)   // bigger up/down swing
        .frequency(0.04)   // longer wavelength → smoother arc
        .speed(0.10))
        // 1) Color phases: Solid black → Glow → Neon → synth-plasma → Radar.
        //    Single PhaseTransitions effect = continuous frame counter across
        //    all phases, so the same effect (e.g. Neon) spans a phase boundary
        //    without resetting its animation and "jumping" visually.
        .at(BG_APPEAR..TOTAL_SECS, PhaseTransitions {
            phases: vec![
                (0, Box::new(Solid(chromakopia::Color::new(0, 0, 0)))),
                (secs_to_frames(1.0), Box::new(Glow::new())),
                (secs_to_frames(BG_AFTER_GLOW - BG_APPEAR), Box::new(Neon::new())),
                (secs_to_frames(BG_AFTER_NEON - BG_APPEAR), Box::new(Plasma::new().palette(synth).seed(42.0))),
                (secs_to_frames(BG_AFTER_ZEBRA - BG_APPEAR), Box::new(Radar::new())),
            ],
            fade_frames: secs_to_frames(1.0),
        })
        // 2) License: compound animation with built-in plasma + halo.
        //    Halo dims BG cells around license letters by 1 cell.
        //    Plasma colors only license cells (cool palette, same seed).
        .at(LICENSE_APPEAR..TOTAL_SECS, CompoundLicenseBounce::new(
            LICENSE,
            license_pad as i32,
            license_target_y,
            LICENSE_SLIDE_SECS,
            Easing::BounceOut,
            1.0,
            4,
            Easing::EaseOut,
            chromakopia::Color::new(180, 180, 180),
            true,
        ).plasma(Plasma::new().palette(fire).seed(PLASMA_SEED))
         .halo(1, 0.55))
        // 3) Bouncing chromakopia banner — halo dims surrounding cells
        //    (license + BG), then writes its own plasma-fire cells on top.
        .at(0.0..TOTAL_SECS, banner)
        // 5) Early alpha fade-in.
        .at(0.0..2.0, AlphaIn::new(2.0).easing(Easing::EaseOut))
        // 6) Wind LAST so it sees banner + license + background, and
        //    dissolves them together.
        .at(WIND_START..TOTAL_SECS, Wind::new()
            .duration(WIND_SECS)
            .angle_deg(162.0)
            .strength(36.0)
            .flutter(0.7, 0.22, 2.2)
            .damping(1.4)
            .fade_tail(1.2));

    let scene = Scene::new().add(timeline);
    scene.quit_key('q').run(TOTAL_SECS).await;
}
