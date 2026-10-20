#!/usr/bin/env bash
# covers: QA-09
# suite: integration
#
# ci/publish-shots.sh pushes the changed screenshots to the ui-shots branch
# and writes a table that embeds them; with nothing changed it does nothing.

set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

git init -q --bare "$tmp/remote.git"
shots="$tmp/ui-shots"
mkdir -p "$shots"
SHOTS_REMOTE="$tmp/remote.git" bash "$root/ci/publish-shots.sh" "$shots" o/r 7 100
[ ! -e "$shots/images.md" ] || { echo "no table expected without changes"; exit 1; }

mkdir -p "$shots/after/Button/hover" "$shots/before/Button/hover" "$shots/diff/Button/hover" "$shots/after/New/x"
echo after >"$shots/after/Button/hover/dark@1.png"
echo before >"$shots/before/Button/hover/dark@1.png"
echo diff >"$shots/diff/Button/hover/dark@1.png"
echo new >"$shots/after/New/x/light@2.png"
SHOTS_REMOTE="$tmp/remote.git" bash "$root/ci/publish-shots.sh" "$shots" o/r 7 100
SHOTS_REMOTE="$tmp/remote.git" bash "$root/ci/publish-shots.sh" "$shots" o/r 7 101

table=$(cat "$shots/images.md")
# shellcheck disable=SC2016 # backticks are Markdown here
grep -qF '| `Button/hover/dark@1.png` | <img src="https://raw.githubusercontent.com/o/r/ui-shots/pr-7/run-101/before/Button/hover/dark@1.png"' <<<"$table" || { echo "missing before image: $table"; exit 1; }
# shellcheck disable=SC2016
grep -qF '| `New/x/light@2.png` | — | <img src=' <<<"$table" || { echo "a new picture has no before: $table"; exit 1; }
files=$(git --git-dir="$tmp/remote.git" ls-tree -r --name-only ui-shots)
grep -q '^pr-7/run-100/diff/Button/hover/dark@1.png$' <<<"$files" || { echo "first run not kept: $files"; exit 1; }
grep -q '^pr-7/run-101/after/New/x/light@2.png$' <<<"$files" || { echo "second run not pushed: $files"; exit 1; }
echo "publish-shots: ok"
