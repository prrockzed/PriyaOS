#!/usr/bin/env bash
# Scans only the commits this event adds, so an old, already-handled finding
# doesn't fail every later run. Used by the `secrets` job in ci.yml.
#
# Inputs (environment): EVENT, and PR_BASE/PR_HEAD for pull requests or
# BEFORE for pushes; GITHUB_SHA comes from Actions.

set -euo pipefail

zero=0000000000000000000000000000000000000000

if [ "$EVENT" = pull_request ]; then
	range="$PR_BASE..$PR_HEAD"
elif [ -n "${BEFORE:-}" ] && [ "$BEFORE" != "$zero" ] &&
	git cat-file -e "$BEFORE^{commit}" 2>/dev/null; then
	range="$BEFORE..$GITHUB_SHA"
else
	# A new branch has no "before"; scan what it adds on top of main.
	range="origin/main..$GITHUB_SHA"
fi

echo "Scanning $range"
gitleaks git --log-opts="$range" --redact --no-banner --verbose .
