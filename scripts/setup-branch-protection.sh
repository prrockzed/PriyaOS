#!/usr/bin/env bash
# Protects main with a GitHub ruleset (sub-phase 0.3, QA-02): changes land
# only through pull requests whose `ci-pass` check is green and up to date,
# history stays linear, and nobody (admins included) can force-push, delete
# the branch or bypass the rules.
#
# Running it again is safe: it updates the "Protect main" ruleset in place.
#
# Usage:
#   bash scripts/setup-branch-protection.sh [owner/repo]   apply (needs gh)
#   bash scripts/setup-branch-protection.sh --print        show the ruleset JSON

set -euo pipefail

name="Protect main"
# GitHub Actions' app id: only checks reported by Actions count as ci-pass,
# so another app can't post a fake green check with the same name.
actions_app_id=15368

ruleset() {
	cat <<JSON
{
  "name": "$name",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["squash"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "ci-pass", "integration_id": $actions_app_id }
        ]
      }
    }
  ]
}
JSON
}

if [ "${1:-}" = "--print" ]; then
	ruleset
	exit 0
fi

repo=${1:-prrockzed/PriyaOS}
id=$(gh api "repos/$repo/rulesets" --jq ".[] | select(.name == \"$name\") | .id")

if [ -n "$id" ]; then
	ruleset | gh api -X PUT "repos/$repo/rulesets/$id" --input - --jq '"Updated ruleset \(.id): \(.name)"'
else
	ruleset | gh api -X POST "repos/$repo/rulesets" --input - --jq '"Created ruleset \(.id): \(.name)"'
fi
