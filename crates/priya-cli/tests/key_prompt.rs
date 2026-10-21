//! `priya ai key set` at a real terminal (0.12): a key typed there is read
//! when Enter is pressed. It once waited forever, holding stdin's lock
//! while its reader thread waited for that same lock.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use priya_testkit::TempPriyaHome;
use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};

// covers: AI-21
// suite: integration
#[test]
fn key_set_reads_a_key_typed_at_a_terminal() {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("a pty");
    grantpt(&master).expect("grantpt");
    unlockpt(&master).expect("unlockpt");
    let name = ptsname(&master, Vec::new()).expect("its name");
    let terminal = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY,
        Mode::empty(),
    )
    .expect("its terminal end");

    let home = TempPriyaHome::new().expect("a temporary home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_priya"));
    home.apply(&mut command);
    let mut child = command
        .args(["ai", "key", "set", "gemini"])
        .stdin(Stdio::from(terminal))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // A keyring that can't be reached, so no real one is ever touched:
        // storing fails, but only after the key was read.
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/priya-test-bus",
        )
        .spawn()
        .expect("priya runs");

    let mut typing = std::fs::File::from(master);
    typing.write_all(b"not-a-real-key\n").expect("typed");
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waited") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("priya ai key set still waiting 10 s after Enter");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut said = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut said)
        .expect("read");
    assert!(said.contains("Paste the key for gemini"), "{said}");
    assert!(!said.contains("Stopped"), "the key was read: {said}");
    // Only said once the key was read and storing it was tried: there is
    // no keyring here (none on macOS, no session bus on Linux), so nothing
    // was stored.
    let keyring = [
        "There's no keyring to keep keys in",
        "The keyring didn't work",
    ];
    assert!(keyring.iter().any(|m| said.contains(m)), "{said}");
    assert!(!status.success(), "{said}");
    drop(typing);
}

// covers: AI-21
// suite: integration
#[test]
fn key_set_reads_a_piped_key() {
    let home = TempPriyaHome::new().expect("a temporary home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_priya"));
    home.apply(&mut command);
    let mut child = command
        .args(["ai", "key", "set", "gemini"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/priya-test-bus",
        )
        .spawn()
        .expect("priya runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"not-a-real-key\n")
        .expect("piped");
    let output = child.wait_with_output().expect("priya ends");
    let said = String::from_utf8_lossy(&output.stderr);
    // No prompt for a piped key; it is read, then storing finds no keyring.
    assert!(!said.contains("Paste the key"), "{said}");
    let keyring = [
        "There's no keyring to keep keys in",
        "The keyring didn't work",
    ];
    assert!(keyring.iter().any(|m| said.contains(m)), "{said}");
}
