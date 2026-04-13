use chromakopia::animate::*;
use chromakopia::{pad, center, presets};


const BANNER: &str = "\
          oooo                                                       oooo                              o8o
          `888                                                       `888                              `\"'
 .ooooo.   888 .oo.   oooo d8b  .ooooo.  ooo. .oo.  .oo.    .oooo.    888  oooo   .ooooo.  oo.ooooo.  oooo   .oooo.
d88' `\"Y8  888P\"Y88b  `888\"\"8P d88' `88b `888P\"Y88bP\"Y88b  `P  )88b   888 .8P'   d88' `88b  888' `88b `888  `P  )88b
888        888   888   888     888   888  888   888   888   .oP\"888   888888.    888   888  888   888  888   .oP\"888
888   .o8  888   888   888     888   888  888   888   888  d8(  888   888 `88b.  888   888  888   888  888  d8(  888
`Y8bod8P' o888o o888o d888b    `Y8bod8P' o888o o888o o888o `Y888\"\"8o o888o o888o `Y8bod8P'  888bod8P' o888o `Y888\"\"8o
                                                                                            888
                                                                                           o888o";


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

#[tokio::main]
async fn main() {
    let bg = chromakopia::bg_color();
    let fg = chromakopia::fg_color();
    let fire = chromakopia::gradient(&["#ffffff", &bg.to_string(), "#ff69b4", "#00cccc", "#fffacd", "#8b4513", &bg.to_string(), "#ffffff"]).palette(256);

    let license_text = pad(&center(LICENSE));
    let banner_text = BANNER.lines()
        .map(|l| format!("    {}    ", l))
        .collect::<Vec<_>>()
        .join("\n");

    let term_width = chromakopia::terminal_width();
    let dycp_amp = 8.0;
    let banner_height = BANNER.lines().count() + 2;
    let license_height = license_text.lines().count();
    let banner_y_offset = ((license_height as i32 - banner_height as i32) / 2).max(0);
    let dycp_height = banner_height + dycp_amp as usize;

    let padded_banner = format!("\n{}\n", banner_text);
    let banner_width = padded_banner.lines().map(|l| l.len()).max().unwrap_or(0);
    let scroll_distance = term_width + banner_width;
    let scroll_secs = 13.0; // finish ~2s before 15s end
    let scroll_speed = scroll_distance as f64 / (scroll_secs * 30.0);

    let scene = Scene::new()
        // CHROMAKOPIA banner — DYCP, gold, scrolls in from right
        .overlay(FadeEnvelope::new(
            Dycp::new(&padded_banner)
                .amplitude(dycp_amp)
                .frequency(0.08)
                .speed(0.07)
                .palette(presets::mist().palette(256))
                .scroll(scroll_speed)
                .scroll_in(-(term_width as i64))
                .wave_delay((scroll_secs * 30.0 * 0.2) as usize)
                .shadow(1, 1, chromakopia::Color::new(40, 40, 40))
                .color(Chain::new()
                    .then(3.0, Transition::new(
                        Rainbow::new(),
                        Radar::new(),
                        3.0, Easing::EaseInOut,
                    ))
                    .then(3.0, Transition::new(
                        Radar::new(),
                        Plasma::new().palette(fire.clone()).seed(42.0),
                        3.0, Easing::EaseInOut,
                    ))
                    .then(3.0, Transition::new(
                        Plasma::new().palette(fire.clone()).seed(42.0),
                        Glow::new().palette(presets::mist().palette(256)),
                        3.0, Easing::EaseInOut,
                    ))
                    .then(3.0, Transition::new(
                        Glow::new().palette(presets::mist().palette(256)),
                        Neon::new(),
                        3.0, Easing::EaseInOut,
                    ))
                    .then(3.0, Neon::new())
                ),
        )
            .total(15.0)
            .fade_in(1.0, Easing::EaseOut)
            .fade_out(2.0, Easing::EaseInOut)
            .from_color(fg),
        dycp_height, banner_y_offset)
        // License text — delayed spread from top with plasma, starts as bg color
        .add({
            let wave_delay = (scroll_secs * 30.0 * 0.2) as usize;
            let spread_delay = wave_delay * 2;
            FadeEnvelope::new(
                DelayedStart::new(spread_delay,
                    Spread::new(&license_text)
                        .origin(SpreadOrigin::Top)
                        .easing(Easing::Elastic(0.25))
                        .duration(5.0)
                        .color(Plasma::new().palette(fire.clone()).seed(42.0).palette_ease(8.0)),
                ),
            )
                .total(15.0)
                .fade_in(1.0, Easing::EaseOut)
                .fade_out(2.0, Easing::EaseInOut)
                .from_color(bg)
                .fade_out_color(chromakopia::Color::new(255, 255, 255))
        })
    ;
    scene.run(15.0).await;
}
