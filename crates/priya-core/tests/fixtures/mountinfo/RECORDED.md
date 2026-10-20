# mountinfo fixtures

Written by hand on 2026-10-20 in the shape of `/proc/self/mountinfo` and
`/etc/fstab` on an Arch Linux laptop, not copied from a real machine (the
owner's machine details stay private). Three layouts:

- `ext4/`: the plain Arch install, root on ext4 and the ESP on `/boot`.
- `btrfs-plain/`: archinstall's btrfs default, with `@` and `@home` but no
  PriyaOS subvolumes, the ESP on `/boot`, and fstab entries with
  `subvolid=`.
- `priyaos/`: the PriyaOS layout of 03-execution §2.

Device names and UUIDs are made up. To record a real one, copy
`/proc/self/mountinfo` and `/etc/fstab` and remove serial numbers.
