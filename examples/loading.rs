/// Simulates a CLI tool that does work while showing animated loading states.
use shimmer::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 1. Rainbow spinner while "connecting"
    let anim = animate::rainbow("⠋ Connecting to database...", 1.0);
    fake_work(1500).await;
    anim.stop();
    eprintln!("✓ Connected");

    // 2. Pulse while "migrating"
    let anim = animate::pulse("⠋ Running migrations...", 1.5);
    fake_work(2000).await;
    anim.stop();
    eprintln!("✓ Migrations complete");

    // 3. Radar while "scanning"
    let anim = animate::radar("Scanning dependencies for vulnerabilities...", 1.0);
    fake_work(2500).await;
    anim.stop();
    eprintln!("✓ No vulnerabilities found");

    // 4. Karaoke for a final message
    let anim = animate::karaoke("All systems operational — ready to serve.", 1.5);
    fake_work(3000).await;
    anim.stop();

    eprintln!();
    eprintln!(
        "{}",
        shimmer::presets::rainbow().apply("🚀 Server started on http://localhost:3000")
    );
}

async fn fake_work(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
