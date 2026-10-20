//! When to restart a module that failed (02-core §5.2): after a pause that
//! doubles from 1 s up to 30 s, until it has failed more than 5 times
//! within 60 s; then the host gives up.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The restart rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// The pause after a first failure.
    pub first: Duration,
    /// The longest pause.
    pub max: Duration,
    /// Failures older than this are forgotten.
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
    GiveUp,
}

/// A module's recent failures.
#[derive(Debug, Clone)]
pub struct Failures {
    policy: RestartPolicy,
    recent: VecDeque<Instant>,
}

impl Failures {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            recent: VecDeque::new(),
        }
    }

    /// Records a failure at `at` and says what to do: the pause doubles
    /// with each failure still inside the window.
    pub fn record(&mut self, at: Instant) -> Decision {
        while self
            .recent
            .front()
            .is_some_and(|first| at.saturating_duration_since(*first) > self.policy.window)
        {
            self.recent.pop_front();
        }
        self.recent.push_back(at);
        let count = self.recent.len();
        if count > self.policy.max_failures {
            return Decision::GiveUp;
        }
        let doublings = u32::try_from(count - 1).unwrap_or(u32::MAX).min(31);
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
        while let Decision::RestartAfter(pause) = failures.record(clock.monotonic()) {
            pauses.push(pause);
            clock.advance(pause);
        }
        assert_eq!(pauses, [secs(1), secs(2), secs(4), secs(8), secs(16)]);

        // Failures far apart are forgotten: the pause starts over.
        let mut failures = Failures::new(RestartPolicy::default());
        for _ in 0..10 {
            assert_eq!(
                failures.record(clock.monotonic()),
                Decision::RestartAfter(secs(1))
            );
            clock.advance(secs(61));
        }

        // The pause stops growing at the cap.
        let patient = RestartPolicy {
            max_failures: 100,
            ..RestartPolicy::default()
        };
        let mut failures = Failures::new(patient);
        let last = (0..40).map(|_| failures.record(clock.monotonic())).last();
        assert_eq!(last, Some(Decision::RestartAfter(secs(30))));
    }
}
