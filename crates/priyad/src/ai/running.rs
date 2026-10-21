//! The answers being written (0.12): how to stop each, a cap on how many,
//! and their tasks, so shutdown can stop them all and wait for each to
//! record how it ended before priyad records that it stopped.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use priya_api::error::{ApiError, ErrorKind};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::warn;

use super::answer::Stop;

/// The most answers being written at once, from everyone: each costs
/// tokens, and a runaway client mustn't run up a bill.
pub const MAX_RUNNING: usize = 32;

/// The answers being written: how to stop each, and their tasks, so
/// shutdown can wait for them to record how they ended.
#[derive(Default)]
pub struct Running {
    stops: Mutex<HashMap<String, watch::Sender<Option<Stop>>>>,
    tasks: Mutex<JoinSet<()>>,
    /// priyad is stopping: no new answers, and an answer whose connection
    /// closes now was stopped by priyad, not by its asker.
    stopping: AtomicBool,
}

impl Running {
    /// Makes room for an answer, or says why there's none.
    pub fn add(&self, id: &str) -> Result<watch::Receiver<Option<Stop>>, ApiError> {
        if self.stopping.load(Ordering::SeqCst) {
            return Err(ApiError::new(ErrorKind::Unavailable, "Priya is stopping."));
        }
        let mut stops = self.stops();
        if stops.len() >= MAX_RUNNING {
            return Err(ApiError::new(
                ErrorKind::Unavailable,
                "Priya is already answering many questions; try again when some are done.",
            ));
        }
        let (stop, stopped) = watch::channel(None);
        stops.insert(id.to_owned(), stop);
        Ok(stopped)
    }

    /// Asks an answer to stop; false if it isn't running.
    pub fn cancel(&self, id: &str, why: Stop) -> bool {
        // Connections close because priyad is stopping: say so.
        let why = if why == Stop::ConnectionClosed && self.stopping.load(Ordering::SeqCst) {
            Stop::DaemonStopping
        } else {
            why
        };
        self.stops()
            .get(id)
            .is_some_and(|stop| stop.send(Some(why)).is_ok())
    }

    /// Frees an answer's place once it has ended.
    pub fn remove(&self, id: &str) {
        self.stops().remove(id);
    }

    /// Runs an answer's task, so shutdown can wait for it.
    pub fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        let mut tasks = self.tasks();
        // Finished tasks are reaped here, so the set doesn't grow.
        while tasks.try_join_next().is_some() {}
        tasks.spawn(task);
    }

    /// Stops every answer and waits up to `patience` for them to record
    /// how they ended. Called once the socket is closed, before priyad
    /// records that it stopped.
    pub async fn shutdown(&self, patience: Duration) {
        self.closing();
        for stop in self.stops().values() {
            let _ = stop.send(Some(Stop::DaemonStopping));
        }
        let mut tasks = std::mem::take(&mut *self.tasks());
        let ended = tokio::time::timeout(patience, async {
            while tasks.join_next().await.is_some() {}
        })
        .await;
        if ended.is_err() {
            warn!(left = tasks.len(), "answers didn't finish in time");
            tasks.shutdown().await;
        }
    }

    /// priyad has been asked to stop: called as soon as the signal comes,
    /// before connections close.
    pub fn closing(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }

    fn stops(&self) -> MutexGuard<'_, HashMap<String, watch::Sender<Option<Stop>>>> {
        self.stops.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tasks(&self) -> MutexGuard<'_, JoinSet<()>> {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Frees an answer's place when its task ends, however it ends (a panic
/// too).
pub struct FreeOnDrop {
    pub running: Arc<Running>,
    pub intent_id: String,
}

impl Drop for FreeOnDrop {
    fn drop(&mut self) {
        self.running.remove(&self.intent_id);
    }
}

/// Stops an answer when its asker goes away first (the connection closed,
/// the module stopped). After a normal end the answer is already over and
/// this does nothing.
pub struct StopOnDrop {
    pub running: Arc<Running>,
    pub intent_id: String,
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.running.cancel(&self.intent_id, Stop::ConnectionClosed);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn answers_are_stopped_by_id_and_only_while_running() {
        let running = Running::default();
        assert!(!running.cancel("i1", Stop::Asked), "nothing to stop");
        let mut stopped = running.add("i1").unwrap();
        assert!(running.cancel("i1", Stop::Asked));
        stopped.changed().await.unwrap();
        assert_eq!(*stopped.borrow(), Some(Stop::Asked));
        running.remove("i1");
        assert!(!running.cancel("i1", Stop::Asked), "it has ended");
    }

    // covers: AI-21
    // suite: unit
    #[test]
    fn no_more_than_max_running_at_once() {
        let running = Running::default();
        let held: Vec<_> = (0..MAX_RUNNING)
            .map(|i| running.add(&format!("i{i}")).unwrap())
            .collect();
        let refused = running.add("one-too-many").unwrap_err();
        assert_eq!(refused.kind, ErrorKind::Unavailable);
        running.remove("i0");
        assert!(running.add("now-there-is-room").is_ok());
        drop(held);
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn once_stopping_a_closed_connection_is_priyad_stopping_and_nothing_new_starts() {
        let running = Running::default();
        let mut stopped = running.add("i1").unwrap();
        running.closing();
        assert!(running.cancel("i1", Stop::ConnectionClosed));
        stopped.changed().await.unwrap();
        assert_eq!(*stopped.borrow(), Some(Stop::DaemonStopping));
        let refused = running.add("i2").unwrap_err();
        assert_eq!(refused.kind, ErrorKind::Unavailable);
    }

    // covers: AI-21
    // suite: unit
    #[tokio::test]
    async fn shutdown_stops_every_answer_and_waits_for_it_to_end() {
        let running = Arc::new(Running::default());
        let recorded = Arc::new(AtomicBool::new(false));
        let mut stopped = running.add("i1").unwrap();
        running.spawn({
            let recorded = recorded.clone();
            async move {
                let _ = stopped.wait_for(Option::is_some).await;
                assert_eq!(*stopped.borrow(), Some(Stop::DaemonStopping));
                // Recording how it ended takes a moment.
                tokio::time::sleep(Duration::from_millis(20)).await;
                recorded.store(true, Ordering::SeqCst);
            }
        });
        running.shutdown(Duration::from_secs(5)).await;
        assert!(recorded.load(Ordering::SeqCst), "shutdown waited for it");

        // One that never ends is given up on.
        running.spawn(std::future::pending());
        let begun = tokio::time::Instant::now();
        running.shutdown(Duration::from_millis(50)).await;
        assert!(begun.elapsed() < Duration::from_secs(5));
    }
}
