import { Layout as BaseLayout } from '@rspress/core/theme-original';
import { TerminalDemo } from '../components/TerminalDemo';

export * from '@rspress/core/theme-original';

const codeExample = `use chromakopia::prelude::*;

// Animate text — zero config
let anim = Rainbow::on("hello, world!").spawn();
tokio::time::sleep(Duration::from_secs(3)).await;
anim.fade_out(1.0);
anim.wait().await;

// indicatif-style progress bars with templates
let mut bar = ProgressBar::new(1000)
    .width(40)
    .chars("━╸ ")
    .template("{spinner} {bar} {pos}/{len} {msg}")
    .filled_color(Color::new(0, 255, 136))
    .empty_color(Color::new(40, 40, 40));

bar.set_position(420);
bar.set_message("downloading...");
print!("\\r{}", bar.render_template());

// ...or pipe through any effect
print!("\\r{}", Rainbow::on(&bar.text(0.7)).frame(tick));

// Demoscene: DYCP + FLD + Scene composition
Scene::new()
    .add(Plasma::on("fire").palette(storm))
    .overlay(Fld::new(
        Dycp::new("chromakopia").color(Rainbow::new())
    ).amplitude(2.0).delay(90).ramp(30), height, 0)
    .run(10.0).await;`;

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
