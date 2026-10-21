#!/usr/bin/env bash
# The package job's checks (0.14), run as root in a fresh CI container:
#   bash scripts/package-check.sh <first build dir> <next build dir>
# Each folder holds priya-*.pkg.tar.zst and priya-shell-*.pkg.tar.zst.
#
# - package_installs_and_doctor_passes: install, `pacman -Qkk`,
#   `priya --version`, then priyad as a normal user and `priya doctor`.
# - package_upgrade_keeps_and_migrates_db: the next build over it keeps
#   the user's history and settings.
# - package_removal_leaves_no_system_files: `pacman -Rns` leaves nothing
#   of the packages under /usr.
# (namcap_clean runs in the build job.)

set -euo pipefail
first=$1
next=$2
user=builder
home=$(getent passwd "$user" | cut -d: -f6)
runtime=$(mktemp -d)
chown "$user" "$runtime"
chmod 700 "$runtime"

step() { echo "==> $*"; }

# Runs a command as the user, with a session's folders.
as_user() {
	sudo -u "$user" env HOME="$home" XDG_RUNTIME_DIR="$runtime" "$@"
}

start_priyad() {
	as_user priyad --foreground >"$runtime/priyad.log" 2>&1 &
	priyad_pid=$!
	for _ in $(seq 1 100); do
		[ -S "$runtime/priya/priyad.sock" ] && return 0
		sleep 0.1
	done
	cat "$runtime/priyad.log"
	echo "priyad didn't start" >&2
	return 1
}

# $! is sudo's, which doesn't pass on a signal from its own process group:
# stop priyad itself.
stop_priyad() {
	pkill -TERM -u "$user" -x priyad
	wait "$priyad_pid" || true
}

step "package_installs_and_doctor_passes"
pacman -U --noconfirm "$first"/*.pkg.tar.zst
pacman -Qkk priya priya-shell
priya --version
start_priyad
# Exit 1 still prints the report; anything else (3: priyad unreachable)
# means no report.
status=0
as_user priya --json doctor >"$runtime/doctor.json" || status=$?
[ "$status" -le 1 ]
jq -e '.checks | length > 0' "$runtime/doctor.json" >/dev/null
# Ok or info everywhere, apart from what a container can't have: a
# desktop and a user systemd.
bad=$(jq -r '.checks[] | select(.status != "ok" and .status != "info")
	| select((.id | startswith("desktop.") or startswith("systemd.")) | not)
	| "\(.id): \(.status): \(.summary)"' "$runtime/doctor.json")
if [ -n "$bad" ]; then
	echo "doctor found problems:"
	echo "$bad"
	exit 1
fi
as_user priya settings set ui.theme light
before=$(as_user priya --json status | jq -e .event_count)
stop_priyad

step "package_upgrade_keeps_and_migrates_db"
pacman -U --noconfirm "$next"/*.pkg.tar.zst
[ "$(pacman -Q priya | cut -d' ' -f2)" = "$(basename "$next"/priya-0*.pkg.tar.zst | cut -d- -f2,3)" ]
start_priyad
theme=$(as_user priya --json settings get ui.theme | jq -r .value)
events=$(as_user priya --json status | jq -e .event_count)
stop_priyad
# The history from before is all there, plus this start.
if [ "$theme" != light ] || [ "$events" -le "$before" ]; then
	echo "the upgrade lost the user's data: theme=$theme events=$events (before: $before)"
	exit 1
fi

step "package_removal_leaves_no_system_files"
files=$(pacman -Qlq priya priya-shell | grep -v '/$')
pacman -Rns --noconfirm priya-shell priya
left=""
for f in $files; do
	if [ -e "$f" ]; then
		left="$left $f"
	fi
done
if [ -n "$left" ]; then
	echo "left behind:$left"
	exit 1
fi
echo "All package checks passed."
