//! How `priya` ends when it can't do its job (0.7).

use std::process::Command;

use priya_testkit::TempPriyaHome;

// covers: CORE-04
// suite: unit
// acceptance: 0.7
// regression: 0.7
#[test]
fn cli_exit_code_3_when_daemon_missing() {
    let home = TempPriyaHome::new().expect("a temporary home");
    for args in [&["status"][..], &["--json", "settings", "get"], &["doctor"]] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_priya"));
        home.apply(&mut command);
        let output = command.args(args).output().expect("priya runs");
        assert_eq!(output.status.code(), Some(3), "priya {args:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "Priya's background service isn't running. Start it with: systemctl --user start priyad",
            "priya {args:?}"
        );
        assert!(output.stdout.is_empty(), "priya {args:?}");
    }

    // A usage mistake is 2, and says what was wrong.
    let mut command = Command::new(env!("CARGO_BIN_EXE_priya"));
    home.apply(&mut command);
    let output = command.arg("frobnicate").output().expect("priya runs");
    assert_eq!(output.status.code(), Some(2));
}
