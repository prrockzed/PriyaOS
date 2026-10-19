#!/usr/bin/env bash
# covers: infra
# suite: unit
# acceptance: 0.1
#
# dev_check_reports_missing_tool: with a PATH of stub tools, dev-check passes
# when all are present, exits 1 naming cargo-nextest when it's missing, and
# treats a rustup proxy whose --version fails as broken.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
check="$root/scripts/dev-check.sh"
fail=0

stubs=$(mktemp -d)
trap 'rm -rf "$stubs"' EXIT
for t in $("$BASH" "$check" --list); do
	printf '#!/bin/sh\necho "%s 1.0 (stub)"\n' "$t" >"$stubs/$t"
	chmod +x "$stubs/$t"
done

run_check() { PATH="$stubs" "$BASH" "$check"; }

out=$(run_check) || { echo "FAIL: dev-check fails with every tool present:"; echo "$out"; fail=1; }

rm "$stubs/cargo-nextest"
if out=$(run_check); then
	echo "FAIL: dev-check passed without cargo-nextest"
	fail=1
elif ! grep -q 'MISSING  *cargo-nextest' <<<"$out"; then
	echo "FAIL: dev-check didn't name cargo-nextest:"; echo "$out"; fail=1
fi
printf '#!/bin/sh\necho stub\n' >"$stubs/cargo-nextest"
chmod +x "$stubs/cargo-nextest"

printf '#!/bin/sh\necho "error: rustfmt is not installed" >&2\nexit 1\n' >"$stubs/rustfmt"
if out=$(run_check); then
	echo "FAIL: dev-check passed with a broken rustfmt proxy"
	fail=1
elif ! grep -q 'BROKEN  *rustfmt' <<<"$out"; then
	echo "FAIL: dev-check didn't report rustfmt as broken:"; echo "$out"; fail=1
fi

[ $fail -eq 0 ] && echo "ok: dev_check_reports_missing_tool"
exit $fail
