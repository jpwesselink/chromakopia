/// Demoscene plasma on CHROMAKOPIA banner with a glow-mist license line.
/// Each section has its own fade envelope, all handled inside the composite effect.
use chromakopia::{animate, presets, bg_color};
use chromakopia::animate::TimeRange;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
              MIT License (c) 2026 JP Wesselink — crates.io/crates/chromakopia
"#;

#[tokio::main]
async fn main() {
    let text = format!("\n{}", BANNER.trim_matches('\n'));

    // Detect terminal background color before animation starts
    let bg = bg_color();

    let plasma_fn = animate::plasma_gradient_effect(presets::storm());
    let glow_fn = animate::glow_effect(presets::dark_n_stormy());

    let composite = move |text: &str, frame: usize| -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let split = lines.len() - 1;
        let t = frame as f64 / 30.0;

        // Banner: plasma, fade in 0-2s, fade out 8-10s
        let banner = lines[..split].join("\n");
        let banner_out = plasma_fn(&banner, frame);
        let banner_opacity = if t < 2.0 {
            smoothstep(t / 2.0)
        } else if t < 8.0 {
            1.0
        } else if t < 10.0 {
            1.0 - smoothstep((t - 8.0) / 2.0)
        } else {
            0.0
        };
        let banner_faded = fade_toward(bg, &banner_out, banner_opacity);

        // License: glow dark_n_stormy, fade in 3-5s, fade out 9-11s
        let license_out = glow_fn(lines[split], frame);
        let license_opacity = if t < 3.0 {
            0.0
        } else if t < 5.0 {
            smoothstep((t - 3.0) / 2.0)
        } else if t < 9.0 {
            1.0
        } else if t < 11.0 {
            1.0 - smoothstep((t - 9.0) / 2.0)
        } else {
            0.0
        };
        let license_faded = fade_toward(bg, &license_out, license_opacity);

        format!("{}\n{}", banner_faded, license_faded)
    };

    animate::Sequence::new(&text)
        .effect(TimeRange::new(0.0, 12.0), 30, composite)
        .run(1.0)
        .await;
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Lerp all truecolor values toward `target` color by (1 - opacity).
fn fade_toward(target: chromakopia::Color, s: &str, opacity: f64) -> String {
    let tr = target.r as f64;
    let tg = target.g as f64;
    let tb = target.b as f64;
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;
            let seq_start = i;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            if i < bytes.len() {
                let seq = &s[seq_start..i];
                if seq.starts_with("38;2;") {
                    let parts: Vec<&str> = seq[5..].split(';').collect();
                    if parts.len() == 3 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            parts[0].parse::<f64>(),
                            parts[1].parse::<f64>(),
                            parts[2].parse::<f64>(),
                        ) {
                            let r = (tr + (r - tr) * opacity) as u8;
                            let g = (tg + (g - tg) * opacity) as u8;
                            let b = (tb + (b - tb) * opacity) as u8;
                            result.push_str(&format!("\x1B[38;2;{};{};{}m", r, g, b));
                            i += 1;
                            continue;
                        }
                    }
                }
                result.push_str(&s[start..=i]);
                i += 1;
            }
        } else {
            let byte = bytes[i];
            let char_len = if byte < 0x80 { 1 }
                else if byte < 0xE0 { 2 }
                else if byte < 0xF0 { 3 }
                else { 4 };
            let end = (i + char_len).min(bytes.len());
            result.push_str(&s[i..end]);
            i = end;
        }
    }
    result
}
