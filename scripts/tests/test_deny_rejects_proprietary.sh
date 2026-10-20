#!/usr/bin/env bash
# covers: QA-10
# suite: integration
# acceptance: 0.3
# regression: 0.3
#
# deny_rejects_disallowed_license_fixture: our deny.toml must fail a
# workspace whose dependency is licensed LicenseRef-Proprietary, and pass the
# same workspace once that licence is a normal one.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fail=0

if ! cargo deny --version >/dev/null 2>&1; then
	echo "FAIL: cargo-deny is not installed (scripts/dev-check.sh lists the tools)"
	exit 1
fi

# Copy the fixture so cargo can write its lock file outside the repository.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp -R "$here/fixtures/deny-proprietary/." "$work/"

deny() {
	cargo deny --manifest-path "$work/Cargo.toml" --config "$root/deny.toml" \
		check licenses 2>&1
}

if out=$(deny); then
	echo "FAIL: cargo deny accepted a proprietary dependency"
	fail=1
elif ! grep -q 'LicenseRef-Proprietary' <<<"$out"; then
	echo "FAIL: cargo deny failed, but not because of the licence:"
	echo "$out"
	fail=1
fi

# The control case: the same tree with an allowed licence passes, so the
# failure above really came from the licence.
sed -i.bak 's/LicenseRef-Proprietary/MIT/' "$work/blob/Cargo.toml"
if ! out=$(deny); then
	echo "FAIL: cargo deny rejected the fixture with an allowed licence:"
	echo "$out"
	fail=1
fi

[ $fail -eq 0 ] && echo "ok: deny_rejects_disallowed_license_fixture"
exit $fail
