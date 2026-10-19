#!/usr/bin/env bash
# covers: infra
# suite: unit
# acceptance: 0.1
#
# survey_script_is_read_only: scripts/survey.sh passes shellcheck and contains
# nothing that could change the machine. The two fixture checks prove the
# scanner itself catches writes and accepts the read-only commands we use.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
check="$here/check-read-only.sh"
fail=0

if ! command -v shellcheck >/dev/null 2>&1; then
	echo "FAIL: shellcheck is not installed (run scripts/dev-setup-*.sh)"
	exit 1
fi
shellcheck -x "$root/scripts/survey.sh" || { echo "FAIL: shellcheck scripts/survey.sh"; fail=1; }

out=$("$check" "$root/scripts/survey.sh") || { echo "FAIL: survey.sh is not read-only:"; echo "$out"; fail=1; }

out=$("$check" "$here/fixtures/read-only-allowed.sh") || { echo "FAIL: scanner flags allowed lines:"; echo "$out"; fail=1; }

expected=$(grep -cv '^#' "$here/fixtures/read-only-forbidden.sh")
flagged=$("$check" "$here/fixtures/read-only-forbidden.sh" | wc -l | tr -d ' ')
if [ "$flagged" != "$expected" ]; then
	echo "FAIL: scanner caught $flagged of $expected forbidden lines"
	fail=1
fi

[ $fail -eq 0 ] && echo "ok: survey_script_is_read_only"
exit $fail
