/// Framebuffer renderer demo — placeholder until Scene API is built.
///
/// Proves the two-loop renderer works: animation writes to grid, renderer diffs.
use chromakopia::animate::framebuffer::{Cell, Effect, FrameBuffer, run_effect};
use chromakopia::Color;
use std::time::Duration;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

/// Inline rainbow effect — just enough to prove the renderer works.
struct RainbowDemo {
    chars: Vec<Vec<char>>,
}

impl RainbowDemo {
    fn new(text: &str) -> Self {
        Self {
            chars: text.split('\n').map(|l| l.chars().collect()).collect(),
        }
    }
}

impl Effect for RainbowDemo {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let hue_offset = (frame * 5 % 360) as f64;
        let max_w = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let hue = (hue_offset + (x as f64 / max_w as f64) * 360.0) % 360.0;
                let color = Color::from_hsv(hue, 1.0, 1.0);
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');
    let lines: Vec<&str> = text.split('\n').collect();
    let height = lines.len();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    run_effect(RainbowDemo::new(text), width, height, Duration::from_secs(10), 1.0).await;
}
