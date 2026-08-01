//! Timeline demo — the chromakopia banner scrolls in, plasma flows over it,
//! the MIT license slides in from below, both hold for a few seconds, then
//! the wind blows them both away like dust.
//!
//! Track order matters here. From top to bottom:
//!   1. Scroll  — banner content (active the whole demo so it stays at rest)
//!   2. SlideDown — license content (active from t=8 onward, pins at rest after slide)
//!   3. Plasma  — colors every non-space cell (banner + license)
//!   4. AlphaIn — fades everything from black during the first 2s
//!   5. Wind    — reads the populated buffer and dissolves it at the end
//!
//! Wind being LAST means it sees everything; wind earlier in the order would
//! only see whatever tracks were before it.

use chromakopia::animate::{
    framebuffer::{secs_to_frames, Cell, DEFAULT_TEXT_COLOR},
    AlphaIn, Easing, Effect, FrameBuffer, Plasma, Scene, Scroll, ScrollDirection, Timeline, Wind,
};
use chromakopia::{gradient, Color};

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
furnished to do so, subject to the following conditions:";

const TOTAL_SECS: f64 = 16.0;
const TOP_MARGIN: usize = 2;
const LICENSE_APPEAR: f64 = 8.0;
const LICENSE_SLIDE_SECS: f64 = 2.0;
const WIND_START: f64 = 13.0; // 3s after the license has settled
const WIND_SECS: f64 = 3.0;

/// Run a Plasma but only color cells in the given row range. Cells outside
/// the range are left untouched. The plasma sampling still uses the cell's
/// absolute (x, y) so two PlasmaInRows with the same seed and different
/// palettes share the underlying field — just mapped to different colors.
struct PlasmaInRows {
    plasma: Plasma,
    rows: std::ops::Range<i32>,
}

impl Effect for PlasmaInRows {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let start = self.rows.start.max(0) as usize;
        let end = (self.rows.end.max(0) as usize).min(buf.height);
        if start >= end { return; }

        // Render plasma into a scratch buffer that only has chars in our rows;
        // plasma skips empty cells, so other rows stay untouched.
        let mut scratch = FrameBuffer::new(buf.width, buf.height);
        for y in start..end {
            for x in 0..buf.width {
                scratch.set(x, y, buf.get(x, y));
            }
        }
        self.plasma.render(&mut scratch, frame);
        for y in start..end {
            for x in 0..buf.width {
                buf.set(x, y, scratch.get(x, y));
            }
        }
    }
}

/// Slide a multi-line text from above the buffer down to a target row.
struct SlideDown {
    chars: Vec<Vec<char>>,
    target_x: usize,
    target_y: i32,
    duration: usize,
    easing: Easing,
    color: Color,
}

impl SlideDown {
    fn new(text: &str, target_x: usize, target_y: i32, seconds: f64, easing: Easing, color: Color) -> Self {
        Self {
            chars: text.split('\n').map(|l| l.chars().collect()).collect(),
            target_x,
            target_y,
            duration: secs_to_frames(seconds),
            easing,
            color,
        }
    }
}

impl Effect for SlideDown {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = (frame as f64 / self.duration.max(1) as f64).min(1.0);
        let eased = self.easing.apply(t);
        let h = self.chars.len() as i32;
        let start_y = -h;
        let cy = (start_y as f64 + (self.target_y - start_y) as f64 * eased).round() as i32;
        for (dy, line) in self.chars.iter().enumerate() {
            let y = cy + dy as i32;
            if y < 0 || y as usize >= buf.height {
                continue;
            }
            for (dx, &ch) in line.iter().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = self.target_x + dx;
                if x >= buf.width {
                    continue;
                }
                buf.set(x, y as usize, Cell::new(ch, self.color));
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let fire = gradient(&["#1a1a1a", "#ff69b4", "#00cccc", "#fffacd", "#8b4513", "#1a1a1a"])
        .palette(256);
    // Cool palette for the license — same plasma seed gives the same field,
    // this maps the field to teal/blue/violet instead.
    let cool = gradient(&["#0a1a2a", "#00cccc", "#5fa8d3", "#9b8cff", "#0a1a2a"])
        .palette(256);
    let plasma_seed = 31.0;

    let banner_w = BANNER.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let banner_h = BANNER.lines().count();
    let license_w = LICENSE.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let license_h = LICENSE.lines().count();

    let term_w = chromakopia::terminal_width().max(banner_w.max(license_w));
    let license_pad = (term_w - license_w) / 2;

    // Center the banner horizontally and add top margin by prepending blank
    // rows. Scroll places chars at their (col, row) position in its source
    // text, so padded text → centered+offset rest position.
    let banner_pad = (term_w - banner_w) / 2;
    let banner_centered = std::iter::repeat(String::new())
        .take(TOP_MARGIN)
        .chain(BANNER.lines().map(|l| format!("{}{}", " ".repeat(banner_pad), l)))
        .collect::<Vec<_>>()
        .join("\n");

    // Buffer: top margin + banner + 2-row gap + license.
    let buf_h = TOP_MARGIN + banner_h + 2 + license_h;
    let license_target_y = (TOP_MARGIN + banner_h + 2) as i32;

    let seed_buf = FrameBuffer::new(term_w, buf_h);
    let timeline = Timeline::from_buffer(seed_buf, TOTAL_SECS)
        // 1) Banner content — scrolls in from left during 0..2s, holds at
        //    rest (centered) for the rest of the demo.
        .at(0.0..TOTAL_SECS, Scroll::new(&banner_centered)
            .direction(ScrollDirection::Left)
            .easing(Easing::EaseOut)
            .duration(2.0))
        // 2) License content — slides in from above starting at t=8, settles
        //    at t=10, holds at rest until end.
        .at(LICENSE_APPEAR..TOTAL_SECS, SlideDown::new(
            LICENSE,
            license_pad,
            license_target_y,
            LICENSE_SLIDE_SECS,
            Easing::EaseOut,
            Color::new(180, 180, 180),
        ))
        // 3) Plasma — same seed across both regions (so the field is unified)
        //    but different palettes (so banner is fire-coloured, license cool).
        .at(0.0..TOTAL_SECS, PlasmaInRows {
            plasma: Plasma::new().palette(fire).seed(plasma_seed),
            rows: TOP_MARGIN as i32..(TOP_MARGIN + banner_h) as i32,
        })
        .at(0.0..TOTAL_SECS, PlasmaInRows {
            plasma: Plasma::new().palette(cool).seed(plasma_seed),
            rows: license_target_y..(license_target_y + license_h as i32),
        })
        // 4) Early alpha fade-in.
        .at(0.0..2.0, AlphaIn::new(2.0).easing(Easing::EaseOut))
        // 5) Wind dissolves whatever's in the buffer at this point — both
        //    banner and license — over the final WIND_SECS.
        .at(WIND_START..TOTAL_SECS, Wind::new()
            .duration(WIND_SECS)
            .angle_deg(162.0)
            .strength(36.0)
            .flutter(0.7, 0.22, 2.2)
            .damping(1.4)
            .fade_tail(1.2));

    let _ = DEFAULT_TEXT_COLOR; // import kept for future tweaks
    let scene = Scene::new().add(timeline);
    scene.quit_key('q').run(TOTAL_SECS).await;
}
