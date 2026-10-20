#!/usr/bin/env bash
# covers: QA-02
# suite: unit
# acceptance: 0.3
# regression: 0.3
#
# branch_protection_requires_ci_pass: the ruleset the setup script applies is
# valid JSON, requires a strict, Actions-reported ci-pass check and a pull
# request, keeps history linear, blocks force-push and deletion, and has no
# bypass actors.

set -u
here=$(cd "$(dirname "$0")" && pwd)
json=$(bash "$here/../setup-branch-protection.sh" --print)

expect() {
	if ! jq -e "$1" >/dev/null <<<"$json"; then
		echo "FAIL: ruleset doesn't satisfy: $1"
		exit 1
	fi
}

expect '.enforcement == "active" and .bypass_actors == []'
expect '.conditions.ref_name.include == ["~DEFAULT_BRANCH"]'
expect '[.rules[].type] | contains(["deletion", "non_fast_forward", "required_linear_history", "pull_request", "required_status_checks"])'
expect '.rules[] | select(.type == "pull_request") | .parameters.required_approving_review_count == 0'
expect '.rules[] | select(.type == "required_status_checks") | .parameters.strict_required_status_checks_policy'
expect '.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks == [{"context": "ci-pass", "integration_id": 15368}]'

echo "ok: branch_protection_requires_ci_pass"
