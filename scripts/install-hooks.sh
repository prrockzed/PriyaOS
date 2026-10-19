#!/usr/bin/env bash
# Turns on the repository's git hooks (.githooks/) for this clone.
# Run once per clone: bash scripts/install-hooks.sh

set -euo pipefail
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
echo "Git hooks are on: .githooks/pre-commit and .githooks/pre-push run from now on."
