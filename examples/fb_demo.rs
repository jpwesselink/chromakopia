use chromakopia::animate::*;
use chromakopia::{pad, presets};
use std::time::Duration;

const BANNER: &str = r#"   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|"#;

const LICENSE: &str = "\
MIT License

Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.";

const CREDIT: &str = "(c) 2026 JP Wesselink";

#[tokio::main]
async fn main() {
    let bg = chromakopia::bg_color();
    let fg = chromakopia::fg_color();
    let bg_hex = bg.to_string();
    let storm = presets::storm().palette(256);
    let fire = chromakopia::gradient(&[&bg_hex, "#ff2975", "#f222ff", "#8c1eff", "#00d9ff", "#ff2975", &bg_hex]).palette(256);
    let fps = 30;
    let total = fps * 15;

    let full = pad(&format!("{}\n\n{}", BANNER, LICENSE));
    let lines: Vec<&str> = full.lines().collect();

    let banner_height = BANNER.lines().count();
    let banner_text: String = lines[..banner_height].join("\n");
    let license_start = banner_height + 1;
    let license_text: String = lines[license_start..].join("\n");

    Scene::new()
        // Banner — scroll + plasma composite with fade envelope
        .block(&banner_text, FadeEnvelope::new(
            Composite::new(
                Scroll::new(&banner_text, storm.clone(), ScrollDirection::Left, Easing::Elastic(0.15), fps * 3, 0),
                Plasma::new(&banner_text, storm.clone(), 42.0),
            ),
            fg, fps, fps * 2, total, Easing::EaseOut, Easing::EaseInOut,
        ))
        .line(Line::blank())
        // License — scroll + plasma composite with fade envelope
        .block(&license_text, FadeEnvelope::new(
            Composite::new(
                Scroll::new(&license_text, fire.clone(), ScrollDirection::Left, Easing::Elastic(0.25), fps * 2, 2),
                Plasma::new(&license_text, fire.clone(), 42.0),
            ),
            fg, fps, fps * 2, total, Easing::EaseOut, Easing::EaseInOut,
        ))
        .run(Duration::from_secs(15))
        .await;
}
