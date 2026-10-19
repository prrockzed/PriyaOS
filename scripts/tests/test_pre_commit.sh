#!/usr/bin/env bash
# covers: QA-03
# suite: integration
# acceptance: 0.3
#
# Runs `xtask pre-commit` in a throwaway git repository holding a one-crate
# workspace:
#   pre_commit_rejects_unformatted_rust
#   pre_commit_rejects_staged_fake_api_key
#   pre_commit_passes_clean_change_under_30s

set -u
# Git hooks export these (a hook in a worktree sets GIT_DIR). Left set, every
# git command below would act on the caller's repository instead of the
# throwaway one.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
	GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_PREFIX
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fail=0

cargo build -q --locked -p xtask --manifest-path "$root/Cargo.toml" || exit 1
xtask="$root/target/debug/xtask"

repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT
cd "$repo" || exit 1
git init -q
# Belt and braces: stop before touching anything if git isn't using it.
if [ "$(git rev-parse --absolute-git-dir)" != "$(pwd -P)/.git" ]; then
	echo "FAIL: git is not using the throwaway repository; refusing to continue"
	exit 1
fi
git config user.name test
git config user.email test@example.invalid
git config commit.gpgsign false
mkdir -p demo/src
cat >Cargo.toml <<'TOML'
[workspace]
members = ["demo"]
resolver = "3"
TOML
cat >demo/Cargo.toml <<'TOML'
[package]
name = "demo"
version = "0.1.0"
edition = "2024"
TOML
cat >demo/src/lib.rs <<'RS'
pub fn two() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_is_two() {
        assert_eq!(super::two(), 2);
    }
}
RS
cargo generate-lockfile -q
git add -A
git -c core.hooksPath=/dev/null commit -qm base # no hooks in this repo

# pre_commit_passes_clean_change_under_30s: this times the hook's own work
# on a tiny crate; the real budget is checked on the Zenbook (phase notes).
sed -i.bak 's/    2$/    1 + 1/' demo/src/lib.rs && rm demo/src/lib.rs.bak
git add demo/src/lib.rs
start=$(date +%s)
if ! out=$("$xtask" pre-commit 2>&1); then
	echo "FAIL: pre-commit rejected a clean change:"
	echo "$out"
	fail=1
fi
took=$(($(date +%s) - start))
if ! grep -q 'unit tests (changed crates)' <<<"$out"; then
	echo "FAIL: pre-commit didn't test the changed crate (file-to-crate mapping broken?):"
	echo "$out"
	fail=1
fi
if [ "$took" -ge 30 ]; then
	echo "FAIL: pre-commit took ${took}s on a one-line change (budget 30s)"
	fail=1
fi
git -c core.hooksPath=/dev/null commit -qm clean

# pre_commit_rejects_unformatted_rust: only the staged copy is unformatted.
{ printf 'pub fn  three()->u32{3}\n'; cat demo/src/lib.rs; } >staged.rs && mv staged.rs demo/src/lib.rs
git add demo/src/lib.rs
cargo fmt --all # the working copy is now formatted; the index is not
if out=$("$xtask" pre-commit 2>&1); then
	echo "FAIL: pre-commit accepted an unformatted staged file"
	fail=1
elif ! grep -q 'not formatted: demo/src/lib.rs' <<<"$out"; then
	echo "FAIL: pre-commit failed, but didn't name the unformatted file:"
	echo "$out"
	fail=1
fi
git reset -q --hard

# A syntax error is reported as such, not as "not formatted".
printf 'pub fn broken( {\n' >demo/src/lib.rs
git add demo/src/lib.rs
if out=$("$xtask" pre-commit 2>&1); then
	echo "FAIL: pre-commit accepted a file that doesn't parse"
	fail=1
elif ! grep -q "rustfmt couldn't parse demo/src/lib.rs" <<<"$out"; then
	echo "FAIL: pre-commit didn't report the parse error:"
	echo "$out"
	fail=1
fi
git reset -q --hard

# pre_commit_rejects_staged_fake_api_key: built at run time, so this file
# itself never contains a key-shaped string.
# Shape of an Anthropic key: sk-ant-api03-, 93 key characters, then AA.
key="sk-ant-""api03-$(printf 'q7Zp2Lx9Wm4Rt8Kv%.0s' 1 2 3 4 5)AbCdEf123GhIjAA"
printf 'const KEY: &str = "%s";\n' "$key" >demo/src/key.rs
git add demo/src/key.rs
if out=$("$xtask" pre-commit 2>&1); then
	echo "FAIL: pre-commit accepted a staged API key"
	fail=1
elif ! grep -qE 'RuleID:[[:space:]]*anthropic-api-key' <<<"$out"; then
	echo "FAIL: pre-commit failed, but not because of the key:"
	echo "$out"
	fail=1
fi

[ $fail -eq 0 ] && echo "ok: pre_commit_passes_clean_change_under_30s, pre_commit_rejects_unformatted_rust, pre_commit_rejects_staged_fake_api_key"
exit $fail
