#!/usr/bin/env bash
# Fails if a script contains anything that could change the machine.
# Used to keep scripts/survey.sh read-only (sub-phase 0.1).
#
# Usage: scripts/tests/check-read-only.sh FILE...
# Prints every offending line as FILE:LINE: text and exits 1 if there are any.
#
# Deliberately strict: a read-only script has no reason to mention these
# commands outside comments, so false positives are fixed by rewording.

set -u

exec perl -ne '
BEGIN {
	@bad = (
		qr/\bsudo\b/, qr/\bdoas\b/, qr/\bpkexec\b/,
		qr/\bmkfs/, qr/(^|[\s;&|(])dd\s/,
		qr/\b(wipefs|sgdisk|parted|cfdisk|fdisk|grub-install|grub-mkconfig|efibootmgr|bootctl|mkswap|umount|shutdown|reboot|poweroff|mkinitcpio)\b/,
		qr/\bbtrfs\s+(subvolume|subvol|property|filesystem|balance|device|send|receive)\b/,
		qr/(^|[\s;&|(`])mount(\s|$)/,
		qr/\bsystemctl\s+(--user\s+)?(start|stop|restart|reload|enable|disable|mask|unmask|daemon-reload|kill|isolate|set-property)\b/,
		qr/\bpacman\s+-(S(?![is])|U|R|D)/,
		qr/\b(rm|mv|cp|ln|chmod|chown|chattr|tee|touch|mkdir|rmdir|truncate|install|swapoff|hyprctl\s+(keyword|dispatch|reload))\b/,
		qr/\bswapon\s+(?!--show)/,
		qr/\bsed\s+(-\w*\s+)*-i/,
		qr/(^|[\s\d;&|(])>>?\s*(?!\/dev\/null\b|&|\d)\S/,
	);
}
$line = $_;
s/(^|\s)#.*$//;
for $re (@bad) {
	if (/$re/) { print "$ARGV:$.: $line"; $n++; last }
}
close ARGV if eof;
END { exit($n ? 1 : 0) }
' "$@"
