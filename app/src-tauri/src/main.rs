// Prevents an extra console window on Windows in release; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // With a command (`muster new …`, `muster list`), it's the command line; with none, the app.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() && !args[0].starts_with("-psn") {
        std::process::exit(muster_lib::cli::main(&args));
    }
    muster_lib::run()
}
