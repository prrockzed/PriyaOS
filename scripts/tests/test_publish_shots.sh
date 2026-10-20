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
png() { printf '\211PNG\r\n\032\n%s' "$1" >"$2"; }
png after "$shots/after/Button/hover/dark@1.png"
png before "$shots/before/Button/hover/dark@1.png"
png diff "$shots/diff/Button/hover/dark@1.png"
png new "$shots/after/New/x/light@2.png"
# Removed baseline: only a before picture.
mkdir -p "$shots/before/Gone/y"
png gone "$shots/before/Gone/y/dark@2.png"
# Neither a plain name nor a PNG: never published.
mkdir -p "$shots/after/Bad"
png bad "$shots/after/Bad/\$(x).png"
echo "<script>" >"$shots/after/New/x/fake@1.png"
# A changed baseline: the guard's pictures win over the harness's.
mkdir -p "$shots/baseline-before/Button/hover" "$shots/baseline-after/Button/hover" "$shots/baseline-diff/Button/hover"
png baseline-diff "$shots/baseline-diff/Button/hover/dark@1.png"
png old-baseline "$shots/baseline-before/Button/hover/dark@1.png"
png new-baseline "$shots/baseline-after/Button/hover/dark@1.png"
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
grep -q '^pr-7/run-101/before/Gone/y/dark@2.png$' <<<"$files" || { echo "a removed picture's before is missing: $files"; exit 1; }
shown=$(git --git-dir="$tmp/remote.git" show ui-shots:pr-7/run-101/before/Button/hover/dark@1.png | tail -c 12)
[ "$shown" = "old-baseline" ] || { echo "the guard's before should win, got $shown"; exit 1; }
if grep -q 'Bad/\|fake@1' <<<"$files"; then
	echo "published a bad name or a non-PNG: $files"
	exit 1
fi
# shellcheck disable=SC2016
grep -qF '| `Gone/y/dark@2.png` | <img' <<<"$table" || { echo "missing removed row: $table"; exit 1; }
if bash "$root/ci/publish-shots.sh" "$shots" o/r '7;x' 100 2>/dev/null; then
	echo "a malformed pull request number was accepted"
	exit 1
fi
echo "publish-shots: ok"
