//! Time as a dependency, so tests can control it (`FakeClock` arrives with
//! priya-testkit in 0.4). Code reads time only through [`Clock`].

use std::fmt::Debug;
use std::time::Instant;

use time::OffsetDateTime;

pub trait Clock: Debug + Send + Sync {
    /// Wall-clock time in UTC, for timestamps people read. It can jump.
    fn now(&self) -> OffsetDateTime;

    /// Monotonic time, for durations and timeouts. It never goes backwards.
    fn monotonic(&self) -> Instant;
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn monotonic(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: infra
    // suite: unit
    // acceptance: 0.2
    // regression: 0.2
    #[test]
    fn system_clock_is_monotonic_enough() {
        let clock: &dyn Clock = &SystemClock;
        let mut last = clock.monotonic();
        for _ in 0..1000 {
            let next = clock.monotonic();
            assert!(next >= last);
            last = next;
        }

        // 2024-01-01T00:00:00Z: a clock earlier than this is broken.
        let floor = OffsetDateTime::from_unix_timestamp(1_704_067_200).unwrap();
        let now = clock.now();
        assert!(now > floor, "wall clock reads {now}");
        assert_eq!(now.offset(), time::UtcOffset::UTC);
    }
}
