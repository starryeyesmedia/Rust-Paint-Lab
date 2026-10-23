//! Route an existing synthetic fixture; no window, hardware, IO, or recording.
use paint_input::replay::Replay;
use paint_stroke::Router;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let replay: Replay = include_str!("../../input/tests/fixtures/pen.replay").parse()?;
    let mut router = Router::default();
    for output in replay.run() {
        if let Some(command) = router.route(&output) {
            println!("{command:?}");
        }
    }
    assert_eq!(router.active_count(), 0);
    Ok(())
}
