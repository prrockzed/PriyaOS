#!/usr/bin/env bash
# covers: UX-02
# suite: unit
#
# shell_unit_runs_the_bar_in_the_session: priya-shell.service starts the
# Priya bar's own Quickshell instance with the graphical session, restarts
# it if it crashes, and wants priyad. Where systemd-analyze exists (Linux),
# both user units must also verify.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
unit="$root/packaging/systemd/priya-shell.service"
fail=0

has() {
	if ! grep -qxF "$1" "$unit"; then
		echo "FAIL: priya-shell.service lacks '$1'"
		fail=1
	fi
}
has "PartOf=graphical-session.target"
has "ExecStart=/usr/bin/qs -p /usr/share/priya/shell"
has "Restart=on-failure"
has "Wants=priyad.service"
has "WantedBy=graphical-session.target"
has "ConditionEnvironment=WAYLAND_DISPLAY"
# Apps started from the bar inherit its limits.
if grep -q "^NoNewPrivileges=" "$unit"; then
	echo "FAIL: priya-shell.service must not set NoNewPrivileges (sudo from the bar would break)"
	fail=1
fi

if command -v systemd-analyze >/dev/null 2>&1; then
	# --user: these are user units; a missing /usr/bin/qs is expected in CI.
	out=$(systemd-analyze --user verify "$unit" "$root/packaging/systemd/priyad.service" 2>&1 |
		grep -v "Command /usr/bin/\(qs\|priyad\) is not executable")
	if [ -n "$out" ]; then
		echo "FAIL: systemd-analyze verify:"
		echo "$out"
		fail=1
	fi
fi

exit "$fail"
