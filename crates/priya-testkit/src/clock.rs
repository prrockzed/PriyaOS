//! A [`Clock`] for tests: it starts at a fixed moment and moves only when
//! [`FakeClock::advance`] is called, so time-dependent code is
//! deterministic.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use priya_core::clock::Clock;
use time::OffsetDateTime;

#[derive(Debug)]
pub struct FakeClock {
    /// The wall-clock start and the monotonic start (a real `Instant`, since
    /// one can't be made from nothing).
    start: (OffsetDateTime, Instant),
    elapsed: Mutex<Duration>,
}

impl FakeClock {
    /// 2026-01-01T00:00:00Z: a fixed, readable starting point.
    pub const DEFAULT_START: i64 = 1_767_225_600;

    pub fn new() -> Self {
        Self::starting_at(
            OffsetDateTime::from_unix_timestamp(Self::DEFAULT_START)
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
        )
    }

    pub fn starting_at(now: OffsetDateTime) -> Self {
        Self {
            start: (now, Instant::now()),
            elapsed: Mutex::new(Duration::ZERO),
        }
    }

    /// Moves both clocks forward by `by`.
    pub fn advance(&self, by: Duration) {
        *self.lock() += by;
    }

    fn elapsed(&self) -> Duration {
        *self.lock()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Duration> {
        // A test that panicked while holding the lock leaves a valid
        // Duration behind, so keep going.
        self.elapsed.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        self.start.0 + self.elapsed()
    }

    fn monotonic(&self) -> Instant {
        self.start.1 + self.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: QA-04
    // suite: unit
    // acceptance: 0.4
    #[test]
    fn fake_clock_advances_deterministically() {
        let clock = FakeClock::new();
        let (wall, mono) = (clock.now(), clock.monotonic());
        assert_eq!(wall.unix_timestamp(), FakeClock::DEFAULT_START);
        assert_eq!(clock.now(), wall, "time doesn't pass on its own");
        assert_eq!(clock.monotonic(), mono);

        clock.advance(Duration::from_secs(90));
        assert_eq!(clock.now() - wall, time::Duration::seconds(90));
        assert_eq!(clock.monotonic() - mono, Duration::from_secs(90));

        let dyn_clock: &dyn Clock = &clock;
        assert_eq!(dyn_clock.now(), wall + time::Duration::seconds(90));
    }
}
