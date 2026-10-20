//! Crash safety under a hard kill (sub-phase 0.5, CORE-01): a process
//! appending in a loop is killed with SIGKILL, three times. Afterwards the log
//! must verify, hold no partial row, and still contain every event the store
//! acknowledged before the kill.
//!
//! SIGKILL leaves the kernel's page cache intact, so this proves that commits
//! are all or nothing and that an acknowledged commit outlives the process.
//! Surviving a power cut (`synchronous=FULL`) needs a VM test.
//!
//! The appending process is this test binary again, running the ignored
//! `append_loop_child`. After each acknowledged batch it prints the batch's
//! last seq, and it stops on its own once its stdin closes, so it never
//! outlives the test.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use priya_core::clock::SystemClock;
use priya_store::{EventStore, StoreConfig};
use priya_testkit::events::setting_event;
use serde_json::Value;

const CHILD_DIR: &str = "PRIYA_APPEND_LOOP_DIR";
const ACK: &str = "acked ";
/// Batches the child must acknowledge before it may be killed, so every
/// round tests something however slow the machine is.
const ACKS_BEFORE_KILL: usize = 3;

/// Every tenth payload is big enough to go to the blob store.
fn event(n: u64) -> priya_api::events::NewEvent {
    let value = if n.is_multiple_of(10) {
        "x".repeat(10_000)
    } else {
        n.to_string()
    };
    setting_event(&format!("crash.key{n}"), Value::from(value))
}

/// The appending child. Dropping it kills it, so a failing test leaves no
/// process behind.
struct AppendLoop {
    child: Child,
    _stdin: ChildStdin,
    /// Acknowledged seqs, read from the child's stdout.
    acks: mpsc::Receiver<i64>,
    reader: Option<JoinHandle<()>>,
    stderr: std::path::PathBuf,
}

impl AppendLoop {
    fn spawn(dir: &std::path::Path, round: u64) -> Self {
        let stderr = dir.join(format!("child-{round}.stderr"));
        let mut child = Command::new(std::env::current_exe().expect("the test binary's path"))
            .args([
                "append_loop_child",
                "--exact",
                "--include-ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_DIR, dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(&stderr).expect("a file for the child's stderr"))
            .spawn()
            .expect("the appending child starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (sender, acks) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(seq) = line.strip_prefix(ACK).and_then(|s| s.parse().ok())
                    && sender.send(seq).is_err()
                {
                    return;
                }
            }
        });
        Self {
            child,
            _stdin: stdin,
            acks,
            reader: Some(reader),
            stderr,
        }
    }

    /// Fails the test, with the child's stderr, if the child has exited.
    fn assert_running(&mut self) {
        if let Some(status) = self.child.try_wait().expect("the child's status") {
            let mut stderr = String::new();
            File::open(&self.stderr)
                .and_then(|mut f| f.read_to_string(&mut stderr))
                .expect("the child's stderr");
            panic!("the appending child exited early ({status}):\n{stderr}");
        }
    }

    /// Waits for `count` acknowledgements; returns the highest seq.
    fn wait_for_acks(&mut self, count: usize) -> i64 {
        let mut highest = 0;
        for _ in 0..count {
            match self.acks.recv_timeout(Duration::from_secs(60)) {
                Ok(seq) => highest = highest.max(seq),
                Err(_) => {
                    self.assert_running();
                    panic!("the child acknowledged nothing for 60 s");
                }
            }
        }
        highest
    }

    /// SIGKILLs the child; returns the highest seq it acknowledged.
    fn kill(mut self, highest: i64) -> i64 {
        self.assert_running();
        // SIGKILL on Unix: no chance to clean up.
        self.child.kill().expect("the child is killed");
        self.child.wait().expect("the killed child is reaped");
        if let Some(reader) = self.reader.take() {
            reader.join().expect("the stdout reader doesn't panic");
        }
        self.acks.try_iter().fold(highest, i64::max)
    }
}

impl Drop for AppendLoop {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// covers: CORE-01
// suite: integration
// acceptance: 0.5
#[test]
fn events_survive_kill_during_append_loop() {
    let dir = tempfile::tempdir().unwrap();
    let mut acknowledged = 0;
    for round in 0..3_u64 {
        let mut child = AppendLoop::spawn(dir.path(), round);
        let highest = child.wait_for_acks(ACKS_BEFORE_KILL);
        // Vary where in a batch the kill lands.
        std::thread::sleep(Duration::from_millis(round * 7));
        let highest = child.kill(highest);
        assert!(highest > acknowledged, "round {round} appended nothing new");
        acknowledged = highest;
    }

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let config = StoreConfig::in_data_dir(dir.path());
        let store = EventStore::open(&config, Arc::new(SystemClock)).unwrap();
        let report = store.verify_chain(1..=i64::MAX).await.unwrap();
        assert!(report.ok, "{report:?}");

        // No partial rows and nothing acknowledged lost: seqs run 1..=n with
        // n at least the last acknowledged seq, and every blob is there.
        let records = store.read_after(0, usize::MAX >> 1).await.unwrap();
        assert_eq!(records.len() as u64, report.checked);
        assert!(
            records.len() as i64 >= acknowledged,
            "{} events stored, but {acknowledged} were acknowledged",
            records.len()
        );
        for (i, record) in records.iter().enumerate() {
            assert_eq!(record.seq, i as i64 + 1);
            store.payload(record).await.unwrap();
        }
        // And the log still takes new events.
        store.append(event(0)).await.unwrap();
        assert!(store.verify_chain(1..=i64::MAX).await.unwrap().ok);
    });
}

// covers: infra
// suite: integration
#[test]
#[ignore = "helper process for events_survive_kill_during_append_loop"]
fn append_loop_child() {
    let Some(dir) = std::env::var_os(CHILD_DIR) else {
        return;
    };
    // Stop when the parent closes stdin (or goes away).
    let stop = Arc::new(AtomicBool::new(false));
    std::thread::spawn({
        let stop = stop.clone();
        move || {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            stop.store(true, Ordering::Relaxed);
        }
    });
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let store = EventStore::open(
            &StoreConfig::in_data_dir(std::path::Path::new(&dir)),
            Arc::new(SystemClock),
        )
        .unwrap();
        let mut stdout = std::io::stdout();
        let mut n = 0_u64;
        while !stop.load(Ordering::Relaxed) {
            n += 1;
            let records = store
                .append_all((0..5).map(|i| event(n * 5 + i)).collect())
                .await
                .unwrap();
            let last = records.last().unwrap().seq;
            writeln!(stdout, "{ACK}{last}").unwrap();
            stdout.flush().unwrap();
        }
    });
}
