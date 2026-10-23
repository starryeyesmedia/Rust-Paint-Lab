//! Explicit, local-only fixture runner. The library itself performs no IO.
use paint_input::replay::Replay;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: replay <fixture.replay>")?;
    let file = std::fs::File::open(path)?;
    // Bound reads before allocating a caller-supplied file in memory.
    use std::io::Read;
    let mut text = String::new();
    file.take((paint_input::replay::MAX_FIXTURE_BYTES + 1) as u64)
        .read_to_string(&mut text)?;
    let replay: Replay = text.parse()?;
    for output in replay.run() {
        println!("{output:?}");
    }
    Ok(())
}
