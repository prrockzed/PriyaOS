#!/usr/bin/env bash
# covers: infra
# suite: unit
# acceptance: 0.1
#
# survey_reports_provided_packages: the first survey run listed `rust` as
# installed because rustup provides it. pkgver must name the providing
# package instead, and stay empty for packages that aren't there.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fail=0

stubs=$(mktemp -d)
trap 'rm -rf "$stubs"' EXIT
cat >"$stubs/pacman" <<'STUB'
#!/bin/sh
[ "$1" = "-Q" ] || exit 1
case "$2" in
rust | rustup) echo "rustup 1.29.1-1" ;;
hyprland) echo "hyprland 0.56.2-4" ;;
*) echo "error: package '$2' was not found" >&2; exit 1 ;;
esac
STUB
chmod +x "$stubs/pacman"

PATH="$stubs:$PATH"
# shellcheck source=/dev/null # survey.sh is checked on its own
. "$root/scripts/survey.sh"

expect() {
	local got
	got=$(pkgver "$1")
	if [ "$got" != "$2" ]; then
		echo "FAIL: pkgver $1: expected '$2', got '$got'"
		fail=1
	fi
}
expect rust "provided by rustup"
expect rustup "1.29.1-1"
expect hyprland "0.56.2-4"
expect linux-zen ""

[ $fail -eq 0 ] && echo "ok: survey_reports_provided_packages"
exit $fail
