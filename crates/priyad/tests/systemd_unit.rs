//! The systemd user unit that starts priyad (0.7).

use std::fs;
use std::path::Path;

fn unit() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/systemd/priyad.service");
    fs::read_to_string(path).expect("the unit exists")
}

// covers: CORE-04
// suite: unit
#[test]
fn the_unit_runs_priyad_as_a_hardened_notify_service() {
    let unit = unit();
    for line in [
        "Type=notify",
        "ExecStart=/usr/bin/priyad",
        "Restart=on-failure",
        "RestartSec=1",
        "NoNewPrivileges=yes",
        "LockPersonality=yes",
        "RestrictRealtime=yes",
        "SystemCallArchitectures=native",
        "WantedBy=default.target",
    ] {
        assert!(unit.lines().any(|l| l == line), "missing {line}");
    }
}

// covers: CORE-04
// suite: integration
#[cfg(target_os = "linux")]
#[test]
fn the_unit_passes_systemd_analyze_verify() {
    // verify checks that ExecStart exists, so point it at the built binary.
    let priyad = env!("CARGO_BIN_EXE_priyad");
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = dir.path().join("priyad.service");
    let unit = unit().replace("ExecStart=/usr/bin/priyad", &format!("ExecStart={priyad}"));
    fs::write(&path, unit).expect("written");
    let output = std::process::Command::new("systemd-analyze")
        .args(["--user", "verify"])
        .arg(&path)
        // CI runs in a container with no user session of its own.
        .env("XDG_RUNTIME_DIR", dir.path())
        .output()
        .expect("systemd-analyze is installed (part of systemd)");
    // Complaints about the unit name it; anything else is the container.
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && !said.contains("priyad.service"),
        "{said}"
    );
}
