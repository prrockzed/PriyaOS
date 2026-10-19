#!/usr/bin/env bash
# Runs every scripts/tests/test_*.sh. Until `cargo xtask test` exists
# (sub-phase 0.4), this is how the script tests run locally and in CI.

set -u
here=$(cd "$(dirname "$0")" && pwd)
failed=0
for t in "$here"/test_*.sh; do
	bash "$t" || failed=$((failed + 1))
done
if [ $failed -gt 0 ]; then
	echo "$failed test file(s) failed"
	exit 1
fi
echo "All script tests passed."
