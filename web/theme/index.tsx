import { Layout as BaseLayout } from '@rspress/core/theme-original';
import { TerminalDemo } from '../components/TerminalDemo';

export * from '@rspress/core/theme-original';

const codeExample = `use chromakopia::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Animate text in place — spawn, fade out, wait for cleanup.
    let anim = Rainbow::on("hello, world!").spawn();
    tokio::time::sleep(Duration::from_secs(3)).await;
    anim.fade_out(1.0);
    anim.wait().await;

    // indicatif-style progress bars with a custom palette + template.
    let mut bar = ProgressBar::new(1000)
        .width(40)
        .chars("━╸ ")
        .template("{spinner} {bar} {pos}/{len} {msg}")
        .filled_color(Color::new(0, 255, 136))
        .empty_color(Color::new(40, 40, 40));
    bar.set_position(420);
    bar.set_message("downloading...");
    println!("{}", bar.render_template());

    // Compose effects: a DYCP wave, rippled by FLD, plasma-coloured.
    let fire = gradient(&["#1a1a1a", "#ff69b4", "#fffacd"]).palette(256);
    Scene::new()
        .add(
            Fld::new(Dycp::new("chromakopia").color(Plasma::new().palette(fire)))
                .amplitude(2.0)
                .delay(30)
                .ramp(20),
        )
        .run(8.0).await;
}`;

const CodeExample = () => (
  <div style={{
    maxWidth: '720px',
    margin: '0 auto',
    padding: '0 1.5rem 3rem',
  }}>
    <pre style={{
      background: '#0f1a0f',
      border: '1px solid #1a3a1a',
      padding: '1.5rem',
      overflow: 'auto',
      fontSize: '14px',
      lineHeight: 1.6,
      color: '#88bb88',
      fontFamily: "'JetBrains Mono', monospace",
    }}>
      <code>{codeExample}</code>
    </pre>
  </div>
);

export const Layout = () => (
  <BaseLayout
    beforeHero={
      <div className="terminal-hero-bg">
        <TerminalDemo bare />
      </div>
    }
    afterFeatures={
      <>
        <CodeExample />
      </>
    }
  />
);
