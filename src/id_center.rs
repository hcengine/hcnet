use std::sync::atomic::{AtomicU64, Ordering};

pub struct IdCenter;

static NOW: AtomicU64 = AtomicU64::new(1);

impl IdCenter {
    pub fn next_server_id() -> u64 {
        static MAX_ID: u64 = u64::MAX >> 33;
        loop {
            let next = NOW.fetch_add(1, Ordering::Relaxed);
            if next > MAX_ID {
                NOW.store(1, Ordering::Relaxed);
                continue;
            }
            return next;
        }
    }
}
