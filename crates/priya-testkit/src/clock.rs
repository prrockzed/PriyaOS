//! A [`Clock`] for tests: it starts at a fixed moment and moves only when
//! [`FakeClock::advance`] is called, so time-dependent code is
//! deterministic.
//!
//! priya-core's own `#[cfg(test)]` unit tests can't use it: a dev-dependency
//! on priya-testkit builds a second copy of priya-core, whose `Clock` is a
//! different trait. Use it from priya-core's `tests/` folder, or from any
//! other crate.

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
    pub const DEFAULT_START: OffsetDateTime =
        match OffsetDateTime::from_unix_timestamp(1_767_225_600) {
            Ok(start) => start,
            Err(_) => panic!("the default start is a valid timestamp"),
        };

    /// A clock at [`DEFAULT_START`](Self::DEFAULT_START).
    pub fn new() -> Self {
        Self::starting_at(Self::DEFAULT_START)
    }

    /// A clock whose wall time starts at `now`.
    pub fn starting_at(now: OffsetDateTime) -> Self {
        Self {
            start: (now, Instant::now()),
            elapsed: Mutex::new(Duration::ZERO),
        }
    }

    /// Moves both clocks forward by `by`. Panics if the total would overflow
    /// (centuries), which no test needs.
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
    // regression: 0.4
    #[test]
    fn fake_clock_advances_deterministically() {
        let clock = FakeClock::new();
        let (wall, mono) = (clock.now(), clock.monotonic());
        assert_eq!(wall, FakeClock::DEFAULT_START);
        assert_eq!(clock.now(), wall, "time doesn't pass on its own");
        assert_eq!(clock.monotonic(), mono);

        clock.advance(Duration::from_secs(90));
        assert_eq!(clock.now() - wall, time::Duration::seconds(90));
        assert_eq!(clock.monotonic() - mono, Duration::from_secs(90));

        let dyn_clock: &dyn Clock = &clock;
        assert_eq!(dyn_clock.now(), wall + time::Duration::seconds(90));
    }
}
