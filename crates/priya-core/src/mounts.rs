//! What the disk looks like, from `/proc/self/mountinfo` and `/etc/fstab`
//! (read by the caller), compared with the PriyaOS layout (03-execution
//! §2): btrfs root on `@` with `/boot` inside it, flat subvolumes for
//! `/home`, logs, the package cache and `/var/lib/priya`, and an fstab `/`
//! entry that doesn't pin a subvolume.
//!
//! Read-only: it only describes. Fixing the layout comes with the root
//! helper (2.4).

use serde::Serialize;

/// One line of `mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Where it is mounted.
    pub mount_point: String,
    /// The filesystem type, such as `btrfs` or `ext4`.
    pub fs_type: String,
    /// What is mounted, such as `/dev/nvme0n1p2`.
    pub source: String,
    /// Mount and filesystem options together, such as `subvol=/@`.
    pub options: Vec<String>,
}

impl Mount {
    /// The btrfs subvolume, without its leading `/`.
    pub fn subvolume(&self) -> Option<&str> {
        self.option("subvol").map(|s| s.trim_start_matches('/'))
    }

    fn option(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find_map(|o| o.strip_prefix(name)?.strip_prefix('='))
    }
}

/// Parses `/proc/self/mountinfo`. Lines it can't read are skipped.
pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines().filter_map(parse_mountinfo_line).collect()
}

fn parse_mountinfo_line(line: &str) -> Option<Mount> {
    // id parent major:minor root mount-point options [optional…] - type source super-options
    let (before, after) = line.split_once(" - ")?;
    let fields: Vec<&str> = before.split(' ').collect();
    let mut after = after.split(' ');
    let fs_type = after.next()?;
    let source = after.next()?;
    let super_options = after.next().unwrap_or("");
    let mut options: Vec<String> = fields.get(5)?.split(',').map(unescape).collect();
    options.extend(
        super_options
            .split(',')
            .filter(|o| !o.is_empty())
            .map(unescape),
    );
    Some(Mount {
        mount_point: unescape(fields.get(4)?),
        fs_type: fs_type.to_owned(),
        source: unescape(source),
        options,
    })
}

/// mountinfo and fstab write spaces and such as `\040` (octal).
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)));
        match octal {
            Some(digits) => {
                let value = digits
                    .iter()
                    .fold(0_u32, |n, d| n * 8 + u32::from(d - b'0'));
                out.push(u8::try_from(value).unwrap_or(b'?'));
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One entry of `/etc/fstab`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FstabEntry {
    pub mount_point: String,
    pub fs_type: String,
    pub options: Vec<String>,
}

/// Parses `/etc/fstab`, skipping comments and blank lines.
pub fn parse_fstab(text: &str) -> Vec<FstabEntry> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            Some(FstabEntry {
                mount_point: unescape(fields.get(1)?),
                fs_type: (*fields.get(2)?).to_owned(),
                options: fields
                    .get(3)
                    .map(|o| o.split(',').map(unescape).collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// The subvolumes the PriyaOS layout mounts, and where.
pub const PRIYAOS_SUBVOLUMES: &[(&str, &str)] = &[
    ("@", "/"),
    ("@home", "/home"),
    ("@log", "/var/log"),
    ("@pkg", "/var/cache/pacman/pkg"),
    ("@priya", "/var/lib/priya"),
];

/// How the disk compares with the PriyaOS layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Layout {
    /// The root filesystem's type, if `/` is mounted.
    pub root_fs: Option<String>,
    /// btrfs subvolumes mounted, as `(mount point, subvolume)`, sorted.
    pub subvolumes: Vec<(String, String)>,
    /// Whether the fstab `/` entry names a subvolume (`subvol=`/`subvolid=`),
    /// which stops a previous system from being booted.
    pub fstab_pins_root: bool,
    /// Whether any fstab entry uses `subvolid=`, which breaks after a swap.
    pub fstab_uses_subvolid: bool,
    /// Whether `/boot` is part of the root filesystem (not its own mount).
    pub boot_inside_root: bool,
    /// The PriyaOS subvolumes that aren't mounted where they should be.
    pub missing: Vec<String>,
}

impl Layout {
    pub fn inspect(mounts: &[Mount], fstab: &[FstabEntry]) -> Self {
        let root = mounts.iter().rev().find(|m| m.mount_point == "/");
        let mut subvolumes: Vec<(String, String)> = mounts
            .iter()
            .filter(|m| m.fs_type == "btrfs")
            .filter_map(|m| Some((m.mount_point.clone(), m.subvolume()?.to_owned())))
            .collect();
        subvolumes.sort();
        subvolumes.dedup();
        let missing = PRIYAOS_SUBVOLUMES
            .iter()
            .filter(|(name, at)| {
                !subvolumes
                    .iter()
                    .any(|(point, subvolume)| point == at && subvolume == name)
            })
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let pins = |entry: &FstabEntry| {
            entry
                .options
                .iter()
                .any(|o| o.starts_with("subvol=") || o.starts_with("subvolid="))
        };
        Self {
            root_fs: root.map(|m| m.fs_type.clone()),
            fstab_pins_root: fstab.iter().any(|e| e.mount_point == "/" && pins(e)),
            fstab_uses_subvolid: fstab
                .iter()
                .any(|e| e.options.iter().any(|o| o.starts_with("subvolid="))),
            boot_inside_root: !mounts.iter().any(|m| m.mount_point == "/boot"),
            subvolumes,
            missing,
        }
    }

    /// Whether everything Priya Sandbox needs is in place.
    pub fn is_priyaos(&self) -> bool {
        self.root_fs.as_deref() == Some("btrfs")
            && self.missing.is_empty()
            && self.boot_inside_root
            && !self.fstab_pins_root
            && !self.fstab_uses_subvolid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: CORE-05
    // suite: unit
    #[test]
    fn mountinfo_lines_and_escapes_are_read() {
        let mounts = parse_mountinfo(
            "29 1 0:26 /@ / rw,relatime shared:1 - btrfs /dev/nvme0n1p2 rw,compress=zstd:3,subvol=/@\n\
             garbage line\n\
             40 29 259:1 / /mnt/my\\040disk rw - ext4 /dev/sdb1 rw\n",
        );
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].subvolume(), Some("@"));
        assert_eq!(mounts[0].fs_type, "btrfs");
        assert_eq!(mounts[1].mount_point, "/mnt/my disk");
        assert_eq!(mounts[1].subvolume(), None);

        let fstab = parse_fstab("# comment\n\nUUID=x / btrfs rw,subvolid=256 0 0 # pinned\n");
        assert_eq!(fstab.len(), 1);
        assert_eq!(fstab[0].options, ["rw", "subvolid=256"]);
    }
}
