use std::time::Instant;

pub fn now() -> String {
    let now = Instant::now();
    let elapsed = now.elapsed();
    let elapsed_secs = elapsed.as_secs();
    let elapsed_nanos = elapsed.subsec_nanos();
    format!("{}.{:09}", elapsed_secs, elapsed_nanos)
}
