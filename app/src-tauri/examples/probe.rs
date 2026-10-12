//! Prints what the app would show for the real queues: `cargo run --example probe`.
fn main() {
    let base = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("playground");
    let queues = muster_lib::queue::discover(&base);
    let t = std::time::Instant::now();
    let mut cache = muster_lib::queue::Cache::default();
    let s = muster_lib::queue::read_state(&queues, &mut cache);
    let cold = t.elapsed();
    let t = std::time::Instant::now();
    let _ = muster_lib::queue::read_state(&queues, &mut cache);
    let warm = t.elapsed();
    for q in &s.queues {
        println!("{} {:?}", q.name, q.counts);
    }
    let waiting: Vec<_> = s
        .tasks
        .iter()
        .filter(|t| t.waiting_for_owner)
        .map(|t| format!("{}/{}", t.queue, t.id))
        .collect();
    println!(
        "tasks {} · waiting for owner {:?} · problems {} · activity {}",
        s.tasks.len(),
        waiting,
        s.problems.len(),
        s.activity.len()
    );
    for p in &s.problems {
        println!("  problem: {p}");
    }
    println!("read: cold {cold:?}, warm {warm:?}");
}
