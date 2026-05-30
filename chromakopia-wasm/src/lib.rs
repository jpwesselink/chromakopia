use wasm_bindgen::prelude::*;
use chromakopia::animate::*;
use chromakopia::animate::framebuffer::{Cell, secs_to_frames, DEFAULT_TEXT_COLOR, FPS};
use chromakopia::presets;
use chromakopia::{gradient, Color, ProgressBar};

/// Install a panic hook that forwards Rust panics to console.error with the
/// actual panic message + stack. Called automatically by wasm-bindgen on
/// module load via `#[wasm_bindgen(start)]`.
#[wasm_bindgen(start)]
pub fn _wasm_init() {
    console_error_panic_hook::set_once();
}

fn fire() -> Vec<Color> {
    chromakopia::gradient(&[
        "#1a1a1a", "#ff69b4", "#00cccc", "#fffacd", "#8b4513", "#1a1a1a",
    ])
    .palette(256)
}

const LOGO_ART: &str = "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

const TAGLINE: &str = "beautiful terminal animations - for rust";

// Bigger figlet "big" art — used only in the DYCP scroller (width doesn't matter, it scrolls)
const CHROMA_BIG: &str = "         888
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888";

const KOPIA_BIG: &str = "\
888                       ,e,
888 ee  e88 88e  888 88e   \"   ,\"Y88b
888 P  d888 888b 888 888b 888 \"8\" 888
888 b  Y888 888P 888 888P 888 ,ee 888
888 8b  \"88 88\"  888 88\"  888 \"88 888
                 888
                 888";

// ─── letters_drop showcase: constants, helpers, structs ────────────────────

const LICENSE: &str = "MIT License · Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.";

const SHOWCASE_STAGGER: f64 = 0.18;
const SHOWCASE_FALL_SECS: f64 = 2.4;
const SHOWCASE_TOP_MARGIN: i32 = 8;
const SHOWCASE_TOTAL_SECS: f64 = 14.0;
const SHOWCASE_PLASMA_SEED: f64 = 7.0;
const SHOWCASE_BOUNCE_AMP: f64 = 2.0;
const SHOWCASE_SLIDE_SECS: f64 = 3.5;
const SHOWCASE_BG_APPEAR: f64 = 4.0;
const SHOWCASE_BG_AFTER_GLOW: f64 = SHOWCASE_BG_APPEAR + 3.0;
const SHOWCASE_BG_AFTER_NEON: f64 = SHOWCASE_BG_APPEAR + 5.0;
const SHOWCASE_BG_AFTER_ZEBRA: f64 = SHOWCASE_BG_APPEAR + 7.0;
const SHOWCASE_LICENSE_APPEAR: f64 = 6.0;
const SHOWCASE_LICENSE_SLIDE_SECS: f64 = 2.0;
const SHOWCASE_WIND_START: f64 = 11.0;
const SHOWCASE_WIND_SECS: f64 = 3.0;

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

fn dynamic_bounce(t: f64, amp: f64) -> f64 {
    let raw = Easing::BounceOut.apply(t);
    if t < 1.0 / 2.75 {
        raw
    } else {
        let dip = 1.0 - raw;
        1.0 - dip * amp
    }
}

struct VSineWave {
    inner: Box<dyn Effect>,
    amplitude: f64,
    frequency: f64,
    speed: f64,
}

impl VSineWave {
    fn new(inner: impl Effect) -> Self {
        Self { inner: Box::new(inner), amplitude: 4.0, frequency: 0.15, speed: 0.05 }
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

struct HBackgroundScroll {
    chars: Vec<Vec<char>>,
    speed: f64,
    color: Color,
    y_offset: i32,
}

impl HBackgroundScroll {
    fn new(text: &str, speed: f64, color: Color, y_offset: i32) -> Self {
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

struct PhaseTransitions {
    phases: Vec<(usize, Box<dyn Effect>)>,
    fade_frames: usize,
}

impl Effect for PhaseTransitions {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        if self.phases.is_empty() { return; }
        let mut active_idx = 0;
        for (i, (start, _)) in self.phases.iter().enumerate() {
            if frame >= *start { active_idx = i; }
        }
        let (start, ref active_effect) = self.phases[active_idx];
        active_effect.render(buf, frame);
        if active_idx > 0 && frame < start + self.fade_frames {
            let t = (frame - start) as f64 / self.fade_frames.max(1) as f64;
            let prev_effect = &self.phases[active_idx - 1].1;
            let mut prev_buf = FrameBuffer::new(buf.width, buf.height);
            for y in 0..buf.height {
                for x in 0..buf.width {
                    let mut cell = buf.get(x, y);
                    cell.color = DEFAULT_TEXT_COLOR;
                    prev_buf.set(x, y, cell);
                }
            }
            prev_effect.render(&mut prev_buf, frame);
            for y in 0..buf.height {
                for x in 0..buf.width {
                    if buf.get(x, y).ch == ' ' { continue; }
                    let prev_c = prev_buf.get(x, y).color;
                    let cur_c = buf.get(x, y).color;
                    buf.set_color(x, y, Color::lerp_rgb(prev_c, cur_c, t));
                }
            }
        }
    }
}

struct BouncingBanner {
    letters: Vec<(usize, Vec<Vec<char>>)>,
    rest_y: i32,
    fall_frames: usize,
    stagger_frames: usize,
    bounce_amp: f64,
    slide_distance: i32,
    slide_frames: usize,
    slide_easing: Easing,
    plasma: Plasma,
    outline_dim: f64,
    halo_radius: i32,
    halo_dim: f64,
}

impl Effect for BouncingBanner {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
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
        self.plasma.render(&mut rest, frame);

        let slide_t = (frame as f64 / self.slide_frames.max(1) as f64).min(1.0);
        let slide_eased = self.slide_easing.apply(slide_t);
        let slide_offset = ((1.0 - slide_eased) * self.slide_distance as f64).round() as i32;

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
                    letter_positions.push((cx + dx as i32, cy + dy as i32));
                }
            }
        }

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
                        let dimmed = Color::lerp_rgb(existing.color, Color::new(0, 0, 0), dim);
                        buf.set(hx as usize, hy as usize, Cell::new(existing.ch, dimmed));
                        already_dimmed.insert((hx, hy));
                    }
                }
            }
        }

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
                        let existing = buf.get(dst_x as usize, dst_y as usize);
                        if existing.ch != ' ' {
                            let dimmed = Color::lerp_rgb(existing.color, Color::new(0, 0, 0), self.outline_dim);
                            buf.set(dst_x as usize, dst_y as usize, Cell::new(existing.ch, dimmed));
                        }
                    } else {
                        let cell = rest.get(src_x, src_y as usize);
                        buf.set(dst_x as usize, dst_y as usize, cell);
                    }
                }
            }
        }
    }
}

struct CompoundLicenseBounce {
    chars: Vec<Vec<char>>,
    target_x: i32,
    target_y: i32,
    block_duration: usize,
    block_easing: Easing,
    line_duration: usize,
    line_stagger: usize,
    line_easing: Easing,
    color: Color,
    alternate_sides: bool,
    plasma: Option<Plasma>,
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
        color: Color,
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
        let bt = (frame as f64 / self.block_duration.max(1) as f64).min(1.0);
        let block_eased = self.block_easing.apply(bt);
        let block_start_y = -(self.chars.len() as i32);
        let block_y = (block_start_y as f64 + (self.target_y - block_start_y) as f64 * block_eased).round() as i32;

        let mut positions: Vec<(i32, i32)> = Vec::new();
        let mut line_state: Vec<(i32, i32)> = Vec::with_capacity(self.chars.len());
        for (idx, line) in self.chars.iter().enumerate() {
            let cy = block_y + idx as i32;
            let line_frame = frame.saturating_sub(idx * self.line_stagger);
            let lt = (line_frame as f64 / self.line_duration.max(1) as f64).min(1.0);
            let line_eased = self.line_easing.apply(lt);
            let from_right = self.alternate_sides && (idx % 2 == 1);
            let start_x: i32 = if from_right { buf.width as i32 } else { -(line.len() as i32) };
            let cx = (start_x as f64 + (self.target_x - start_x) as f64 * line_eased).round() as i32;
            line_state.push((cx, cy));
            for (dx, &ch) in line.iter().enumerate() {
                if ch == ' ' { continue; }
                positions.push((cx + dx as i32, cy));
            }
        }

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
                        let dimmed = Color::lerp_rgb(existing.color, Color::new(0, 0, 0), dim);
                        buf.set(hx as usize, hy as usize, Cell::new(existing.ch, dimmed));
                        already_dimmed.insert((hx, hy));
                    }
                }
            }
        }

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

fn make_scene_letters_drop(w: usize) -> (Box<dyn Effect>, usize) {
    let _ = FPS; // imported but only referenced indirectly via secs_to_frames
    // Banner palette — green → cyan → blue, matching the hero title gradient.
    let fire = gradient(&["#003322", "#00ff88", "#00ffcc", "#00bbff", "#003322"]).palette(256);
    // BG plasma palette — same hues, deeper so it reads as background.
    let synth = gradient(&[
        "#001a14", "#00aa55", "#00cccc", "#0077bb", "#001a14",
    ]).palette(256);

    let letters_raw = split_banner(LOGO_ART);
    let banner_w = LOGO_ART.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let banner_h = LOGO_ART.lines().count();

    const HUGE_SCALE: usize = 4;
    let bg_source: Vec<String> = letters_raw
        .iter()
        .chain(letters_raw.iter())
        .map(|(_, art)| art.clone())
        .collect();
    let scaled_letters: Vec<String> = bg_source
        .iter()
        .map(|art| {
            let lw = art.lines().map(|l| l.chars().count()).max().unwrap_or(0) * HUGE_SCALE;
            scale_letter(art, HUGE_SCALE, HUGE_SCALE, lw)
        })
        .collect();
    let bg_h = scaled_letters.iter().map(|l| l.lines().count()).max().unwrap_or(0);
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

    let license_w = LICENSE.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let license_h = LICENSE.lines().count();

    let term_w = w.max(banner_w.max(license_w));
    let left_pad = (term_w - banner_w) / 2;
    let license_pad = (term_w - license_w) / 2;

    let banner_block_h = SHOWCASE_TOP_MARGIN as usize + banner_h;
    let fg_h = banner_block_h + 2 + license_h;
    let bg_amplitude_slack: usize = 12;
    let bg_needed_h = bg_h + 2 * bg_amplitude_slack;
    let buf_h = fg_h.max(bg_needed_h);
    let rest_y = SHOWCASE_TOP_MARGIN;
    let license_target_y = banner_block_h as i32 + 2;

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
        fall_frames: secs_to_frames(SHOWCASE_FALL_SECS),
        stagger_frames: secs_to_frames(SHOWCASE_STAGGER),
        bounce_amp: SHOWCASE_BOUNCE_AMP,
        slide_distance: term_w as i32,
        slide_frames: secs_to_frames(SHOWCASE_SLIDE_SECS),
        slide_easing: Easing::EaseOut,
        plasma: Plasma::new().palette(fire.clone()).seed(SHOWCASE_PLASMA_SEED),
        outline_dim: 0.0,
        halo_radius: 1,
        halo_dim: 0.55,
    };

    let single_pass_cols = h_source_width as f64 / 2.0;
    let scroll_secs = SHOWCASE_TOTAL_SECS - 1.0;
    let scroll_speed_cols_per_frame = (single_pass_cols + term_w as f64)
        / (scroll_secs * chromakopia::animate::framebuffer::FPS);

    let seed_buf = FrameBuffer::new(term_w, buf_h);
    let bg_y_offset = ((buf_h as i32 - bg_h as i32) / 2).max(0);
    // The chromakopia banner runs FOREVER (way past the cycle), so the hero
    // shows it indefinitely. BG + wind run their 14s arc, then go away
    // (their tracks end) — only the banner keeps animating after.
    const FOREVER: f64 = 9999.0;
    let timeline = Timeline::from_buffer(seed_buf, FOREVER)
        // BG scroll — only runs during the entry+wind window.
        .at(SHOWCASE_BG_APPEAR..SHOWCASE_TOTAL_SECS, VSineWave::new(HBackgroundScroll::new(
            &horizontal_source,
            scroll_speed_cols_per_frame,
            Color::new(85, 85, 105),
            bg_y_offset,
        ))
        .amplitude(10.0)
        .frequency(0.04)
        .speed(0.10))
        // BG color phases.
        .at(SHOWCASE_BG_APPEAR..SHOWCASE_TOTAL_SECS, PhaseTransitions {
            phases: vec![
                (0, Box::new(Solid(Color::new(0, 0, 0)))),
                (secs_to_frames(1.0), Box::new(Glow::new())),
                (secs_to_frames(SHOWCASE_BG_AFTER_GLOW - SHOWCASE_BG_APPEAR), Box::new(Neon::new())),
                (secs_to_frames(SHOWCASE_BG_AFTER_NEON - SHOWCASE_BG_APPEAR), Box::new(Plasma::new().palette(synth).seed(42.0))),
                (secs_to_frames(SHOWCASE_BG_AFTER_ZEBRA - SHOWCASE_BG_APPEAR), Box::new(Radar::new())),
            ],
            fade_frames: secs_to_frames(1.0),
        })
        // Wind BEFORE banner — dissolves only the bg cells, never reaches the banner.
        .at(SHOWCASE_WIND_START..SHOWCASE_TOTAL_SECS, Wind::new()
            .duration(SHOWCASE_WIND_SECS)
            .angle_deg(162.0)
            .strength(36.0)
            .flutter(0.7, 0.22, 2.2)
            .damping(1.4)
            .fade_tail(1.2))
        // Banner LAST so it survives the wind. Runs forever — keeps animating
        // its plasma after the bg has been blown away.
        .at(0.0..FOREVER, banner)
        // Early alpha fade-in over the banner.
        .at(0.0..2.0, AlphaIn::new(2.0).easing(Easing::EaseOut));

    let scene = Scene::new().add(timeline);
    let h = scene.height();
    (Box::new(scene), h)
}

// ─── end letters_drop showcase ─────────────────────────────────────────────

fn center(text: &str, width: usize) -> String {
    let max_len = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let pad = if max_len < width { (width - max_len) / 2 } else { 0 };
    text.lines()
        .map(|line| format!("{}{}", " ".repeat(pad), line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn make_scene_0_rainbow(w: usize) -> (Box<dyn Effect>, usize) {
    let text = center(LOGO_ART, w);
    let scene = Scene::new().add(
        FadeEnvelope::new(Rainbow::on(&text))
            .total(30.0)
            .fade_in(1.5, Easing::EaseOut)
            .fade_out(2.0, Easing::EaseInOut)
            .from_color(Color::new(0, 0, 0)),
    );
    let h = scene.height();
    (Box::new(scene), h)
}

fn make_scene_1_plasma(w: usize) -> (Box<dyn Effect>, usize) {
    let fire = fire();
    let text = center(LOGO_ART, w);
    let scene = Scene::new().add(
        FadeEnvelope::new(
            Plasma::new()
                .palette(fire.clone())
                .seed(42.0)
                .on(&text),
        )
        .total(30.0)
        .fade_in(1.5, Easing::EaseOut)
        .fade_out(2.0, Easing::EaseInOut)
        .from_color(Color::new(0, 0, 0)),
    );
    let h = scene.height();
    (Box::new(scene), h)
}

fn make_scene_2_glow(w: usize) -> (Box<dyn Effect>, usize) {
    let mist = presets::mist().palette(256);
    let text = center(LOGO_ART, w);
    let scene = Scene::new().add(
        FadeEnvelope::new(
            Glow::new().palette(mist.clone()).on(&text),
        )
        .total(30.0)
        .fade_in(1.5, Easing::EaseOut)
        .fade_out(2.0, Easing::EaseInOut)
        .from_color(Color::new(0, 0, 0)),
    );
    let h = scene.height();
    (Box::new(scene), h)
}

fn dycp_line(text: &str, amp: f64, phase: f64, start_offset: i64, ease_frames: usize, color: impl Effect) -> Dycp {
    Dycp::new(text)
        .amplitude(amp)
        .frequency(0.09)
        .speed(0.08)
        .phase_offset(phase)
        .scroll_in(start_offset)
        .ease_in(ease_frames)
        .color(color)
}

fn make_scene_6_dycp(w: usize) -> (Box<dyn Effect>, usize) {
    let start = -(w as i64);

    let storm = presets::storm().palette(256);
    let mist  = presets::mist().palette(256);

    let amp1 = 5.0_f64;
    let amp2 = 6.0_f64;
    let amp3 = 4.0_f64;

    // Text heights (line count)
    let h_chroma = CHROMA_BIG.lines().count();
    let h_kopia  = KOPIA_BIG.lines().count();
    let h_tag    = 1_usize;

    // Layout: 1 blank line between each block
    let y1 = 0_i32;
    let y2 = (h_chroma + 1) as i32;           // after chroma + 1 gap
    let y3 = (h_chroma + 1 + h_kopia + 1) as i32; // after kopia + 1 gap

    // Overlay height = text height + amplitude (bounce room)
    let oh1 = h_chroma + amp1 as usize + 1;
    let oh2 = h_kopia  + amp2 as usize + 1;
    let oh3 = h_tag    + amp3 as usize + 1;

    // Total scene height = layout rows + max possible bounce overshoot
    let total_h = y3 as usize + h_tag + amp3 as usize + 1;

    let fld_amp = 2.0;
    let ease1 = (2.5 * 30.0) as usize;
    let ease2 = (11.0 * 30.0) as usize;
    let ease3 = (5.0 * 30.0) as usize;

    // Each DYCP wrapped in FLD (freq=0 → whole block moves as one unit)
    // FLD delays until DYCP has eased in, then ramps up fast
    let line1 = FadeEnvelope::new(Fld::new(dycp_line(
        CHROMA_BIG, amp1, 0.0, start, ease1,
        Rainbow::new(),
    )).amplitude(fld_amp).frequency(0.0).speed(0.12).phase(0.0)
      .delay(ease1).ramp(30)
    ).total(30.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(Color::new(0, 0, 0));

    let line2 = FadeEnvelope::new(Fld::new(dycp_line(
        KOPIA_BIG, amp2, std::f64::consts::PI * 0.7, start, ease2,
        Plasma::new().palette(storm.clone()).seed(42.0),
    )).amplitude(fld_amp).frequency(0.0).speed(0.10).phase(std::f64::consts::PI * 0.6)
      .delay(ease2).ramp(30)
    ).total(30.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(Color::new(0, 0, 0));

    let line3 = FadeEnvelope::new(Fld::new(dycp_line(
        TAGLINE, amp3, std::f64::consts::PI * 1.4, start, ease3,
        Glow::new().palette(mist.clone()),
    )).amplitude(fld_amp).frequency(0.0).speed(0.14).phase(std::f64::consts::PI * 1.3)
      .delay(ease3).ramp(30)
    ).total(30.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(Color::new(0, 0, 0));

    // Extra room in overlay heights for FLD displacement
    let fld_extra = fld_amp as usize + 1;
    let oh1 = oh1 + fld_extra * 2;
    let oh2 = oh2 + fld_extra * 2;
    let oh3 = oh3 + fld_extra * 2;
    let total_h = total_h + fld_extra * 2;

    let scene = Scene::new()
        .overlay(line1, oh1, y1)
        .overlay(line2, oh2, y2)
        .overlay(line3, oh3, y3);

    let _ = w;
    (Box::new(scene), total_h)
}

/// A single animated progress bar — 1-row Effect with dynamic progress.
struct AnimatedBar {
    label: String,
    bar_width: usize,
    bar_chars: String,
    speed: f64,
    phase: f64,
    offset: f64,
    color: Box<dyn Effect>,
    center_pad: usize,
}

impl Effect for AnimatedBar {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 * 0.03;
        let progress = ((t * self.speed + self.phase).sin() * 0.5 + 0.5) * 0.85 + self.offset;
        let pb = ProgressBar::new(100).width(self.bar_width).chars(&self.bar_chars);
        let bar_text = pb.text(progress.min(1.0));
        let text = format!("{}{}", self.label, bar_text);

        let label_color = Color::new(100, 100, 100);

        for (i, ch) in text.chars().enumerate() {
            let x = self.center_pad + i;
            if x < buf.width {
                buf.set(x, 0, Cell::new(ch, Color::new(204, 204, 204)));
            }
        }

        // Color effect only on the bar portion
        self.color.render(buf, frame);

        // Restore label to static color (overwrite effect coloring)
        for (i, ch) in self.label.chars().enumerate() {
            let x = self.center_pad + i;
            if x < buf.width {
                buf.set(x, 0, Cell::new(ch, label_color));
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.center_pad + self.label.len() + self.bar_width, 1)
    }
}

fn make_scene_7_progress(w: usize) -> (Box<dyn Effect>, usize) {
    let bar_w = 35_usize;
    let storm = presets::storm().palette(256);
    let mist = presets::mist().palette(256);

    let configs: Vec<(&str, &str, f64, f64, f64, Box<dyn Effect>)> = vec![
        ("  downloading crates  ", "█▓░", 1.2, 0.0,   0.10, Box::new(Rainbow::new())),
        ("  compiling (42/58)   ", "━╸ ", 0.8, 1.0,   0.05, Box::new(Plasma::new().palette(storm.clone()).seed(7.0))),
        ("  linking modules     ", "=>-", 1.5, 2.2,   0.0,  Box::new(Glow::new().palette(mist.clone()))),
        ("  running tests       ", "##.", 0.6, 3.5,   0.15, Box::new(Radar::new())),
        ("  optimizing release  ", "█░ ", 1.0, 4.8,   0.0,  Box::new(Plasma::new().palette(presets::flughafen().palette(256)).seed(13.0))),
    ];

    let total_text_w = configs[0].0.len() + bar_w;
    let pad = if total_text_w < w { (w - total_text_w) / 2 } else { 0 };

    let mut scene = Scene::new();
    for (label, chars, speed, phase, offset, color) in configs {
        scene = scene.add(FadeEnvelope::new(AnimatedBar {
            label: label.to_string(),
            bar_width: bar_w,
            bar_chars: chars.to_string(),
            speed,
            phase,
            offset,
            color,
            center_pad: pad,
        })
        .total(30.0)
        .fade_in(1.5, Easing::EaseOut)
        .fade_out(2.0, Easing::EaseInOut)
        .from_color(Color::new(0, 0, 0)))
        .blank();
    }
    let h = scene.height();
    (Box::new(scene), h)
}

const ABOUT_TEXT: &str = "\
                  a tiny rust crate that turns
                  your terminal into a canvas

              gradients   ·   scenes   ·   plasmas
              particles   ·   ASCII    ·   progress

                composable  ·  zero deps  ·  CI-safe";

fn make_scene_9_about(w: usize) -> (Box<dyn Effect>, usize) {
    let storm = presets::storm().palette(256);
    let body = format!("{}\n\n{}", LOGO_ART, ABOUT_TEXT);
    let text = center(&body, w);

    let plasma_text = Plasma::new()
        .palette(storm)
        .seed(31.0)
        .on(&text);

    let wind = Wind::wrap(plasma_text)
        .delay(20.0)        // hold long enough to read
        .duration(5.0)
        .angle_deg(162.0)   // wind blows left, tear follows wind
        .strength(36.0)
        .flutter(0.7, 0.22, 2.2)
        .damping(1.4)
        .fade_tail(1.0);

    let scene = Scene::new().add(
        FadeEnvelope::new(wind)
            .total(30.0)
            .fade_in(1.5, Easing::EaseOut)
            .fade_out(0.0, Easing::Linear)  // wind handles the exit
            .from_color(Color::new(0, 0, 0)),
    );
    let h = scene.height();
    (Box::new(scene), h)
}

fn make_scene_8_wind(w: usize) -> (Box<dyn Effect>, usize) {
    // Plasma-coloured banner that holds for a few seconds, flutters,
    // tears from the upwind edge and gets blown off-screen.
    let fire = fire();
    let text = center(LOGO_ART, w);
    let plasma_text = Plasma::new()
        .palette(fire)
        .seed(91.0)
        .on(&text);

    let wind = Wind::wrap(plasma_text)
        .delay(4.0)
        .duration(4.5)
        .angle_deg(162.0) // wind blows left, slightly down — tear follows wind (right → left)
        .strength(36.0)
        .flutter(0.7, 0.22, 2.2)
        .damping(1.4)
        .fade_tail(1.0);

    let scene = Scene::new().add(
        FadeEnvelope::new(wind)
            .total(30.0)
            .fade_in(1.5, Easing::EaseOut)
            .fade_out(2.0, Easing::EaseInOut)
            .from_color(Color::new(0, 0, 0)),
    );
    let h = scene.height();
    (Box::new(scene), h)
}

const DEMO_NAMES: &[&str] = &[
    "Showcase",
    "Rainbow",
    "Plasma Fire",
    "Mist Glow",
    "DYCP",
    "Progress Bars",
    "Wind",
    "About",
];

#[wasm_bindgen]
pub fn demo_count() -> u32 {
    DEMO_NAMES.len() as u32
}

#[wasm_bindgen]
pub fn demo_name(id: u32) -> String {
    DEMO_NAMES.get(id as usize).unwrap_or(&"Unknown").to_string()
}

/// A WASM-friendly renderer: holds a scene and a reusable FrameBuffer.
#[wasm_bindgen]
pub struct Renderer {
    effect: Box<dyn Effect>,
    buf: chromakopia::animate::FrameBuffer,
    w: usize,
    h: usize,
}

#[wasm_bindgen]
impl Renderer {
    /// Render frame `n` and return the ANSI string.
    pub fn frame(&mut self, n: u32) -> String {
        self.buf.clear();
        self.effect.render(&mut self.buf, n as usize);
        self.buf.to_ansi_string()
    }

    pub fn width(&self) -> u32 { self.w as u32 }
    pub fn height(&self) -> u32 { self.h as u32 }
}

/// Create a renderer for a demo scene. `width` controls the buffer width.
#[wasm_bindgen]
pub fn make_renderer(id: u32, width: u32) -> Renderer {
    let w = width as usize;
    let (effect, h) = match id % DEMO_NAMES.len() as u32 {
        0 => make_scene_letters_drop(w),
        1 => make_scene_0_rainbow(w),
        2 => make_scene_1_plasma(w),
        3 => make_scene_2_glow(w),
        4 => make_scene_6_dycp(w),
        5 => make_scene_7_progress(w),
        6 => make_scene_8_wind(w),
        7 => make_scene_9_about(w),
        _ => make_scene_letters_drop(w),
    };
    let buf = chromakopia::animate::FrameBuffer::new(w, h);
    Renderer { effect, buf, w, h }
}
