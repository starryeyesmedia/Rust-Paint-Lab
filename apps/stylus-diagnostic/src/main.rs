#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    windows::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "stylus-diagnostic requires Windows 10 or later; no pen hardware is accessed on this platform."
    );
    std::process::exit(1);
}
