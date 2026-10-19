#!/usr/bin/env bash
# Runs shellcheck on every tracked shell script. CI's `scripts` job and
# `cargo xtask ci-local` both call this, so they check the same files.
# Test fixtures are data for the tests, not scripts, so they are skipped.

set -euo pipefail
cd "$(dirname "$0")/.."

# Listed first, so a git failure stops the script instead of running
# shellcheck on nothing.
list=$(git ls-files --cached --others --exclude-standard '*.sh' '.githooks/*' ':!:scripts/tests/fixtures/**')
if [ -z "$list" ]; then
	echo "lint-shell: found no shell scripts; is this a git checkout?" >&2
	exit 1
fi
files=()
while IFS= read -r f; do files+=("$f"); done <<<"$list"
shellcheck "${files[@]}"
echo "shellcheck: ${#files[@]} scripts ok"
