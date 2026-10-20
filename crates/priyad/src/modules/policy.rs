//! When to restart a module that failed (02-core §5.2): after a pause that
//! doubles with each failure in a row, from 1 s up to 30 s; a run that
//! lasted a whole window starts the pauses over. More than 5 failures
//! within 60 s and the host gives up.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The restart rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// The pause after a first failure.
    pub first: Duration,
    /// The longest pause.
    pub max: Duration,
    /// Failures older than this are forgotten; a run this long was healthy.
    pub window: Duration,
    /// More failures than this within the window, and the host gives up.
    pub max_failures: usize,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            first: Duration::from_secs(1),
            max: Duration::from_secs(30),
            window: Duration::from_secs(60),
            max_failures: 5,
        }
    }
}

/// What to do after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    RestartAfter(Duration),
    /// Final for this module: the supervisor stops, and enabling it again
    /// starts with a fresh [`Failures`].
    GiveUp,
}

/// A module's recent failures.
#[derive(Debug, Clone)]
pub struct Failures {
    policy: RestartPolicy,
    /// Failure times inside the window, for giving up.
    recent: VecDeque<Instant>,
    /// Failures in a row, for the pause.
    streak: u32,
}

impl Failures {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            recent: VecDeque::new(),
            streak: 0,
        }
    }

    /// Records a failure at `at` of a run that started at `started`, and
    /// says what to do.
    pub fn record(&mut self, started: Instant, at: Instant) -> Decision {
        while self
            .recent
            .front()
            .is_some_and(|first| at.saturating_duration_since(*first) > self.policy.window)
        {
            self.recent.pop_front();
        }
        self.recent.push_back(at);
        if self.recent.len() > self.policy.max_failures {
            return Decision::GiveUp;
        }
        if at.saturating_duration_since(started) >= self.policy.window {
            self.streak = 0;
        }
        self.streak = self.streak.saturating_add(1);
        let doublings = (self.streak - 1).min(31);
        let pause = self
            .policy
            .first
            .saturating_mul(1 << doublings)
            .min(self.policy.max);
        Decision::RestartAfter(pause)
    }
}

#[cfg(test)]
mod tests {
    use priya_core::clock::Clock;
    use priya_testkit::FakeClock;

    use super::*;

    const fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    // covers: CORE-02
    // suite: unit
    // acceptance: 0.8
    #[test]
    fn backoff_schedule_and_give_up_after_five_in_sixty_seconds() {
        let clock = FakeClock::new();
        let mut failures = Failures::new(RestartPolicy::default());
        // A module that fails as soon as it is restarted: 1, 2, 4, 8, 16 s,
        // and the sixth failure, 31 s in, is more than five within 60 s.
        let mut pauses = Vec::new();
        let now = || clock.monotonic();
        while let Decision::RestartAfter(pause) = failures.record(now(), now()) {
            pauses.push(pause);
            clock.advance(pause);
        }
        assert_eq!(pauses, [secs(1), secs(2), secs(4), secs(8), secs(16)]);

        // One that fails 20 s after each start never fails six times in a
        // minute, and its pause grows to the 30 s cap.
        let mut failures = Failures::new(RestartPolicy::default());
        let mut pauses = Vec::new();
        for _ in 0..8 {
            let started = now();
            clock.advance(secs(20));
            let Decision::RestartAfter(pause) = failures.record(started, now()) else {
                panic!("it isn't given up on");
            };
            pauses.push(pause.as_secs());
            clock.advance(pause);
        }
        assert_eq!(pauses, [1, 2, 4, 8, 16, 30, 30, 30]);

        // A run that lasted a whole minute was healthy: the pause starts over.
        let started = now();
        clock.advance(secs(60));
        assert_eq!(
            failures.record(started, now()),
            Decision::RestartAfter(secs(1))
        );
    }

    // covers: CORE-02
    // suite: unit
    #[test]
    fn six_failures_exactly_sixty_seconds_apart_still_count() {
        let clock = FakeClock::new();
        let start = clock.monotonic();
        let policy = RestartPolicy::default();
        let at = |offset: Duration| start + offset;
        let mut failures = Failures::new(policy);
        for i in 0..5 {
            assert!(matches!(
                failures.record(start, at(secs(i * 12))),
                Decision::RestartAfter(_)
            ));
        }
        assert_eq!(
            failures.record(start, at(secs(60))),
            Decision::GiveUp,
            "within 60 s"
        );

        let mut failures = Failures::new(policy);
        for i in 0..5 {
            failures.record(start, at(secs(i * 12)));
        }
        let later = at(secs(60) + Duration::from_nanos(1));
        assert!(
            matches!(failures.record(start, later), Decision::RestartAfter(_)),
            "the first is out"
        );
    }
}
