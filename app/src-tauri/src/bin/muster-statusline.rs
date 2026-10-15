//! The status line's reader (0113, `bin/statusline-usage`): `muster accounts record --from
//! statusline` in a binary of its own. It runs on every status-line render, so it links none of
//! the app's frameworks (build.rs drops the ones it doesn't use): dyld loading AppKit and WebKit
//! for the app's binary cost more than the 5 ms budget.
fn main() {
    std::process::exit(muster_lib::cli::statusline())
}
