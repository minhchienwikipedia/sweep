use std::{
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    thread,
    time::Duration,
};

/// Runs `work`, calling `tick` every `ms` milliseconds on another thread until
/// it finishes, and once more at the end so the final value is always reported.
pub fn every<T>(ms: u64, tick: impl Fn() + Sync, work: impl FnOnce() -> T) -> T {
    let done = AtomicBool::new(false);
    let out = thread::scope(|s| {
        s.spawn(|| {
            while !done.load(Relaxed) {
                tick();
                thread::sleep(Duration::from_millis(ms));
            }
        });
        let out = work();
        done.store(true, Relaxed);
        out
    });
    tick();
    out
}
