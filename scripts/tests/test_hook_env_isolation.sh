#!/usr/bin/env bash
# covers: QA-03
# suite: integration
# bug: hooks run from a git worktree export GIT_DIR, and
#   test_pre_commit.sh then ran `git init`/`git config`/`git commit` against
#   the real repository instead of its throwaway one (2026-10-20)
#
# script_tests_ignore_git_env_from_hooks: runs test_pre_commit.sh with
# GIT_DIR and GIT_WORK_TREE pointing at a decoy repository, as a hook in a
# worktree would, and checks the decoy is left exactly as it was.

set -u
here=$(cd "$(dirname "$0")" && pwd)
fail=0

decoy=$(mktemp -d)
trap 'rm -rf "$decoy"' EXIT
git init -q "$decoy/repo"
git -C "$decoy/repo" -c user.name=owner -c user.email=owner@example.invalid \
	commit -q --allow-empty -m "decoy root"
before_config=$(cat "$decoy/repo/.git/config")
before_head=$(git -C "$decoy/repo" rev-parse HEAD)

GIT_DIR="$decoy/repo/.git" GIT_WORK_TREE="$decoy/repo" \
	bash "$here/test_pre_commit.sh" >/dev/null 2>&1

if [ "$(cat "$decoy/repo/.git/config")" != "$before_config" ]; then
	echo "FAIL: test_pre_commit.sh changed the git config of the repository in GIT_DIR"
	diff <(echo "$before_config") "$decoy/repo/.git/config"
	fail=1
fi
if [ "$(git -C "$decoy/repo" rev-parse HEAD)" != "$before_head" ]; then
	echo "FAIL: test_pre_commit.sh committed to the repository in GIT_DIR:"
	git -C "$decoy/repo" log --oneline -3
	fail=1
fi

[ $fail -eq 0 ] && echo "ok: script_tests_ignore_git_env_from_hooks"
exit $fail
