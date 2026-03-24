use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, ScrollDirection, TimeRange};

const LINE1: &str = "MIT License (c) 2026 JP Wesselink";

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

const LINE3: &str = "github.com/jpwesselink/chromakopia  crates.io/crates/chromakopia";

#[tokio::main]
async fn main() {
    let banner = BANNER.trim_matches('\n');
    let banner_lines = banner.lines().count();
    let max_width = banner.lines().map(|l| l.len()).max().unwrap_or(0);

    // Center line1 and line3 to match banner width
    let pad1 = (max_width.saturating_sub(LINE1.len())) / 2;
    let pad3 = (max_width.saturating_sub(LINE3.len())) / 2;
    let line1_centered = format!("{:>width$}{}", "", LINE1, width = pad1);
    let line3_centered = format!("{:>width$}{}", "", LINE3, width = pad3);

    let full_text = format!("{}\n{}\n{}", line1_centered, banner, line3_centered);

    let fps = 30;
    let line1_scroll = animate::scroll_eased_gradient_effect(
        ScrollDirection::Left, Easing::ElasticOut, presets::mist(), fps * 2,
    );
    let banner_scroll = animate::scroll_eased_gradient_effect(
        ScrollDirection::Right, Easing::ElasticOut, presets::storm(), fps * 2,
    );
    let line3_scroll = animate::scroll_eased_gradient_effect(
        ScrollDirection::Left, Easing::ElasticOut, presets::mist(), fps * 2,
    );

    let composite = move |text: &str, frame: usize| -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let l1 = lines[0];
        let banner_text = lines[1..=banner_lines].join("\n");
        let l3 = lines[banner_lines + 1];

        // Stagger: line1 starts immediately, banner 0.5s later, line3 1s later
        let delay_banner = 15;
        let delay_line3 = 30;

        let l1_out = line1_scroll(l1, frame);
        let banner_out = if frame >= delay_banner {
            banner_scroll(&banner_text, frame - delay_banner)
        } else {
            " ".repeat(banner_text.lines().next().map_or(0, |l| l.len()))
                .lines()
                .cycle()
                .take(banner_lines)
                .collect::<Vec<_>>()
                .join("\n")
        };
        let l3_out = if frame >= delay_line3 {
            line3_scroll(l3, frame - delay_line3)
        } else {
            " ".repeat(l3.len())
        };

        format!("{}\n{}\n{}", l1_out, banner_out, l3_out)
    };

    animate::Sequence::new(&full_text)
        .effect(TimeRange::new(0.0, 6.0), fps as u64, composite)
        .run(1.0)
        .await;
}
