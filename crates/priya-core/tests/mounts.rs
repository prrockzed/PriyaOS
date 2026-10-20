//! The disk layout read from recorded mountinfo and fstab files (0.7).

use std::path::{Path, PathBuf};

use priya_core::mounts::{Layout, parse_fstab, parse_mountinfo};
use priya_testkit::fixture;

fn layout(name: &str) -> Layout {
    let dir: PathBuf =
        fixture(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mountinfo")).join(name);
    let read = |file: &str| std::fs::read_to_string(dir.join(file)).expect("a fixture file");
    Layout::inspect(
        &parse_mountinfo(&read("mountinfo")),
        &parse_fstab(&read("fstab")),
    )
}

// covers: CORE-05
// suite: unit
// acceptance: 0.7
// regression: 0.7
#[test]
fn doctor_btrfs_layout_from_mountinfo_fixtures() {
    let ext4 = layout("ext4");
    assert_eq!(ext4.root_fs.as_deref(), Some("ext4"));
    assert!(ext4.subvolumes.is_empty());
    assert!(!ext4.boot_inside_root, "the ESP is on /boot");
    assert!(!ext4.is_priyaos());

    let plain = layout("btrfs-plain");
    assert_eq!(plain.root_fs.as_deref(), Some("btrfs"));
    assert_eq!(plain.missing, ["@priya"]);
    assert!(plain.fstab_pins_root && plain.fstab_uses_subvolid);
    assert!(!plain.boot_inside_root);
    assert!(!plain.is_priyaos());

    let priyaos = layout("priyaos");
    assert!(priyaos.missing.is_empty(), "{:?}", priyaos.missing);
    assert!(
        priyaos.boot_inside_root,
        "the ESP is on /efi; /boot is in @"
    );
    assert!(!priyaos.fstab_pins_root && !priyaos.fstab_uses_subvolid);
    assert!(priyaos.is_priyaos());
    assert!(
        priyaos
            .subvolumes
            .contains(&("/var/lib/docker".to_owned(), "@containers".to_owned()))
    );
}
