//! Rolling 3D cube: 4 faces around the X-axis (front / top / back / bottom).
//! As the cube rolls, the bottom face slides up into the front position,
//! the front slides up to become the top, etc.
//!
//! All 4 sides use the same chromakopia banner for now — easier to read the
//! motion than 4 different textures. Each face gets a slightly different
//! palette so you can tell which is which.
//!
//! Run: `cargo run --release --example cube_rotate`

use chromakopia::animate::{
    framebuffer::{tempo, Cell, DEFAULT_TEXT_COLOR},
    Effect, FrameBuffer, Scene,
};
use chromakopia::{gradient, Color};

const BANNER: &str = "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

struct CubeRoll {
    /// 4 face textures: indices 0=front, 1=top, 2=back, 3=bottom at t=0.
    faces: [Vec<Vec<char>>; 4],
    palettes: [Vec<Color>; 4],
    src_w: usize,
    src_h: usize,
    /// Maximum angle (radians) of the swing. 2π = one full revolution each
    /// direction; π = half revolution (shows back); π/2 = quarter tilt.
    swing_amplitude: f64,
    /// Radians per frame of the SWING oscillation (not the rotation itself).
    /// Smaller = slower swing period.
    swing_freq: f64,
    /// Phase shift between adjacent letters.
    phase_step: f64,
    /// col → letter_index map. Cells in inter-letter gaps share the previous
    /// letter's phase, so the whole letter rotates as one cube.
    col_to_letter: Vec<usize>,
    /// Depth multiplier — bigger = deeper cube → more dramatic Y motion per
    /// degree of rotation, and stronger per-row shading.
    depth: f64,
    /// 0..1 — how much per-row Z-distance shading to apply. 0 disables.
    /// 0.5 means the rear edge is at 50% brightness vs the near edge.
    shade: f64,
}

impl CubeRoll {
    fn new(
        faces: [&str; 4],
        palettes: [Vec<Color>; 4],
        swing_amplitude: f64,
        swing_freq: f64,
        phase_step: f64,
        depth: f64,
        shade: f64,
    ) -> Self {
        let parsed: [Vec<Vec<char>>; 4] = std::array::from_fn(|i| {
            faces[i].lines().map(|l| l.chars().collect::<Vec<_>>()).collect()
        });
        let src_w = parsed.iter().flat_map(|f| f.iter()).map(|l| l.len()).max().unwrap_or(1);
        let src_h = parsed.iter().map(|f| f.len()).max().unwrap_or(1);
        // Pad all faces to (src_w, src_h).
        let mut faces = parsed;
        for f in faces.iter_mut() {
            f.resize(src_h, Vec::new());
            for l in f.iter_mut() { l.resize(src_w, ' '); }
        }
        // Three groups by column range, manually mapped to the rozzo
        // "chromakopia" letter boundaries (split_banner can't be trusted here
        // because the figlet's intra-letter 1-col gaps confuse the detector).
        //   cols 0..33  → "chro"
        //   cols 34..60 → "mak"
        //   cols 61..   → "opia"
        let col_to_letter: Vec<usize> = (0..src_w)
            .map(|c| if c < 34 { 0 } else if c < 61 { 1 } else { 2 })
            .collect();

        Self { faces, palettes, src_w, src_h, swing_amplitude, swing_freq, phase_step, col_to_letter, depth, shade }
    }
}

impl Effect for CubeRoll {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let oscillator = tempo(frame) * self.swing_freq;
        let center_x = (buf.width as i32) / 2;
        let center_y = (buf.height as i32) / 2;

        // Per-letter rotation. r = cube's vertical-axis half-radius (depth).
        let r = (self.src_h as f64 / 2.0) * self.depth;
        let half_w = self.src_w as i32 / 2;

        for src_x in 0..self.src_w {
            let screen_x = center_x - half_w + src_x as i32;
            if screen_x < 0 || screen_x as usize >= buf.width { continue; }

            // SWING oscillator: angle = A · sin(ω·t + group·phase). Sine gives
            // natural easing at the extremes (slows toward turnaround) and
            // direction reversal twice per cycle.
            let letter_idx = self.col_to_letter[src_x];
            let col_angle = self.swing_amplitude
                * (oscillator + (letter_idx as f64) * self.phase_step).sin();

            // Collect this column's visible faces, depth-sorted (back-to-front).
            let mut visible: Vec<(usize, f64, f64)> = (0..4)
                .filter_map(|i| {
                    let angle = col_angle + (i as f64) * std::f64::consts::FRAC_PI_2;
                    let cos_a = angle.cos();
                    if cos_a <= 0.001 { return None; }
                    Some((i, cos_a, angle.sin()))
                })
                .collect();
            visible.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            for (face_idx, cos_a, sin_a) in visible {
                let texture = &self.faces[face_idx];
                let palette = &self.palettes[face_idx];

                let face_center_y = center_y as f64 - sin_a * r;
                let squeezed_half_h = r * cos_a;
                let top_y = (face_center_y - squeezed_half_h).floor() as i32;
                let bot_y = (face_center_y + squeezed_half_h).ceil() as i32;

                for screen_y in top_y..=bot_y {
                    if screen_y < 0 || screen_y as usize >= buf.height { continue; }
                    let dy = screen_y as f64 - face_center_y;
                    let src_y_f = self.src_h as f64 / 2.0 + dy / cos_a;
                    let src_y = src_y_f.round() as i32;
                    if src_y < 0 || src_y as usize >= self.src_h { continue; }

                    let ch = texture[src_y as usize][src_x];
                    if ch == ' ' { continue; }
                    let base = palette[src_x % palette.len()];
                    // Per-row Z-shade: the row at sin_a > 0 with high dy is
                    // further from the camera → dimmer. Front edge brightest.
                    let dy_rel = (src_y as f64 - self.src_h as f64 / 2.0) / (self.src_h as f64 / 2.0);
                    let z_norm = dy_rel * sin_a; // +1 = far, -1 = near
                    let shade_factor = 1.0 - self.shade * (z_norm + 1.0) / 2.0;
                    let brightness = (cos_a * shade_factor).clamp(0.0, 1.0);
                    let dimmed = Color::lerp_rgb(Color::new(0, 0, 0), base, brightness);
                    buf.set(screen_x as usize, screen_y as usize, Cell::new(ch, dimmed));
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        // With deeper cube the rotation extends further vertically.
        let r = (self.src_h as f64 / 2.0) * self.depth;
        let h_extent = (self.src_h as f64 + 2.0 * r) as usize + 2;
        (self.src_w + 2, h_extent)
    }
}

#[tokio::main]
async fn main() {
    let white = vec![Color::new(255, 255, 255)];

    let cube = CubeRoll::new(
        [BANNER, BANNER, BANNER, BANNER],
        [white.clone(), white.clone(), white.clone(), white],
        std::f64::consts::FRAC_PI_2,      // ±90° swing amplitude
        0.100,                            // swing freq (rad/frame)
        0.5,                              // phase shift between adjacent letters
        1.0,                              // depth multiplier
        0.6,                              // Z-shade strength
    );
    let _ = gradient;

    let _ = DEFAULT_TEXT_COLOR;
    let scene = Scene::new().add(cube);
    scene.quit_key('q').run(20.0).await;
}
