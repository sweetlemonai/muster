//! Prints the app's state for the real queues as JSON (for screenshots): `cargo run --example dump`.
fn main() {
    let base = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("playground");
    let queues = muster_lib::projects::active(&base, &muster_lib::projects::Config::default());
    let s = muster_lib::queue::read_state(&queues, &mut muster_lib::queue::Cache::default());
    println!("{}", serde_json::to_string(&s).unwrap());
}
