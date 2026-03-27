#[cfg(debug_assertions)]
fn main() {
    if let Err(err) = app_lib::benchmark::run_cli(std::env::args()) {
        eprintln!("benchmark failed: {err:#}");
        std::process::exit(1);
    }
}

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!("ocr_bench is only available in debug builds.");
    std::process::exit(1);
}
