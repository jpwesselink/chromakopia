use chromakopia::{animate, presets};
use chromakopia::animate::{ScrollDirection, TimeRange};

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

const TAGLINE: &str =
    "MIT License (c) 2026 JP Wesselink https://github.com/jpwesselink/chromakopia";

#[tokio::main]
async fn main() {
    let banner = BANNER.trim_matches('\n');
    let banner_lines = banner.lines().count();
    // Combine banner + blank line + tagline as one text block
    let full_text = format!("{}\n\n{}", banner, TAGLINE);

    let banner_scroll = animate::scroll_gradient_effect(ScrollDirection::Left, presets::storm(), 60);
    let tagline_scroll = animate::scroll_gradient_effect(ScrollDirection::Right, presets::mist(), 45);

    let composite = move |text: &str, frame: usize| -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let banner_text = lines[..banner_lines].join("\n");
        let tagline_text = lines[banner_lines + 1..].join("\n");

        let banner_out = banner_scroll(&banner_text, frame);

        // Tagline starts 30 frames (1s) after the banner
        let tagline_delay = 30;
        let tagline_out = if frame >= tagline_delay {
            tagline_scroll(&tagline_text, frame - tagline_delay)
        } else {
            " ".repeat(tagline_text.len())
        };

        format!("{}\n\n{}", banner_out, tagline_out)
    };

    animate::Sequence::new(&full_text)
        .effect(TimeRange::new(0.0, 6.0), 30, composite)
        .run(1.0)
        .await;
}
