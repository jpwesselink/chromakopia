use shimmer::{gradient, presets};

fn main() {
    println!("{}", gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello, gradient world!"));
    println!("{}", gradient(&["cyan", "pink"]).apply("Shimmer is beautiful"));
    println!("{}", gradient(&["#833ab4", "#fd1d1d", "#fcb045"]).hsv().apply("HSV interpolation!"));
    println!();

    println!("=== Presets ===");
    let sample = "The quick brown fox jumps over the lazy dog";
    println!("atlas:     {}", presets::atlas().apply(sample));
    println!("cristal:   {}", presets::cristal().apply(sample));
    println!("teen:      {}", presets::teen().apply(sample));
    println!("mind:      {}", presets::mind().apply(sample));
    println!("morning:   {}", presets::morning().apply(sample));
    println!("vice:      {}", presets::vice().apply(sample));
    println!("passion:   {}", presets::passion().apply(sample));
    println!("fruit:     {}", presets::fruit().apply(sample));
    println!("instagram: {}", presets::instagram().apply(sample));
    println!("retro:     {}", presets::retro().apply(sample));
    println!("summer:    {}", presets::summer().apply(sample));
    println!("rainbow:   {}", presets::rainbow().apply(sample));
    println!("pastel:        {}", presets::pastel().apply(sample));
    println!("dark_n_stormy: {}", presets::dark_n_stormy().apply(sample));
    println!("mist:          {}", presets::mist().apply(sample));
    println!("relic:         {}", presets::relic().apply(sample));
    println!("flughafen:     {}", presets::flughafen().apply(sample));
    println!();

    let art = r#"
       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|
"#
    .trim_start_matches('\n');

    println!("=== Multiline ===");
    println!("{}", presets::rainbow().multiline(art));
}
