use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Custom epoch: 2026-01-01 00:00:00 UTC (1767225600000 ms)
pub const SNOWFLAKE_EPOCH_MS: i64 = 1_767_225_600_000;

const WORKER_ID_BITS: u8 = 10;
const SEQUENCE_BITS: u8 = 12;

const MAX_WORKER_ID: i64 = (1 << WORKER_ID_BITS) - 1;
const SEQUENCE_MASK: i64 = (1 << SEQUENCE_BITS) - 1;

const WORKER_ID_SHIFT: u8 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u8 = SEQUENCE_BITS + WORKER_ID_BITS;

#[derive(Debug)]
struct SnowflakeState {
    last_timestamp: i64,
    sequence: i64,
}

/// High-throughput monotonic Snowflake sequence generator for message IDs.
#[derive(Debug)]
pub struct SnowflakeGenerator {
    worker_id: i64,
    state: Mutex<SnowflakeState>,
}

impl SnowflakeGenerator {
    pub fn new(worker_id: i64) -> Self {
        let valid_worker = worker_id & MAX_WORKER_ID;
        Self {
            worker_id: valid_worker,
            state: Mutex::new(SnowflakeState {
                last_timestamp: 0,
                sequence: 0,
            }),
        }
    }

    /// Generate next 64-bit monotonically increasing sequence ID.
    pub fn next_id(&self) -> i64 {
        let mut state = self.state.lock().expect("Snowflake mutex lock poisoned");
        let mut now = current_time_ms();

        if now < state.last_timestamp {
            // Clock moved backwards, spin until it catches up
            while now < state.last_timestamp {
                std::hint::spin_loop();
                now = current_time_ms();
            }
        }

        if now == state.last_timestamp {
            state.sequence = (state.sequence + 1) & SEQUENCE_MASK;
            if state.sequence == 0 {
                // Sequence exhausted within the current millisecond; wait for next ms
                while now <= state.last_timestamp {
                    std::hint::spin_loop();
                    now = current_time_ms();
                }
                state.last_timestamp = now;
            }
        } else {
            state.last_timestamp = now;
            state.sequence = 0;
        }

        let delta = (state.last_timestamp - SNOWFLAKE_EPOCH_MS).max(0);
        (delta << TIMESTAMP_SHIFT) | (self.worker_id << WORKER_ID_SHIFT) | state.sequence
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX EPOCH")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonicity() {
        let generator = SnowflakeGenerator::new(1);
        let mut prev = 0;
        for _ in 0..10_000 {
            let id = generator.next_id();
            assert!(
                id > prev,
                "ID must be strictly greater than previous: {id} <= {prev}"
            );
            prev = id;
        }
    }
}
