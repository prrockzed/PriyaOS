#!/usr/bin/env bash
# Runs every scripts/tests/test_*.sh and records each outcome in
# target/trace/script-results.json, which `cargo xtask trace` reads.
# CI's `scripts` job and `cargo xtask ci-local` use this;
# `cargo xtask test <suite>` runs the same files by tag.

set -u
# A git hook (pre-push runs this through ci-local) exports GIT_DIR and
# friends; tests that make throwaway repositories must not inherit them.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
	GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_PREFIX
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
results="$root/target/trace/script-results.json"
mkdir -p "$(dirname "$results")"

failed=0
entries=""
for t in "$here"/test_*.sh; do
	id="scripts/tests/$(basename "$t")"
	if bash "$t"; then
		outcome=passed
	else
		outcome=failed
		failed=$((failed + 1))
	fi
	entries="$entries${entries:+,}\n  \"$id\": \"$outcome\""
done
# Test file names are plain ASCII (test_*.sh), so no JSON escaping is needed.
printf '{%b\n}\n' "$entries" >"$results"

if [ $failed -gt 0 ]; then
	echo "$failed test file(s) failed"
	exit 1
fi
echo "All script tests passed."
