#!/usr/bin/env bash
# Runs every scripts/tests/test_*.sh. Until `cargo xtask test` exists
# (sub-phase 0.4), this is how the script tests run locally and in CI.

set -u
# A git hook (pre-push runs this through ci-local) exports GIT_DIR and
# friends; tests that make throwaway repositories must not inherit them.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
	GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_PREFIX
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
