#!/usr/bin/env bash
# Runs shellcheck on every tracked shell script. CI's `scripts` job and
# `cargo xtask ci-local` both call this, so they check the same files.
# Test fixtures are data for the tests, not scripts, so they are skipped.

set -euo pipefail
cd "$(dirname "$0")/.."

files=()
while IFS= read -r -d '' f; do files+=("$f"); done < <(git ls-files -z --cached --others --exclude-standard '*.sh' '.githooks/*' ':!:scripts/tests/fixtures/**')
shellcheck "${files[@]}"
echo "shellcheck: ${#files[@]} scripts ok"
