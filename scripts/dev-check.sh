#!/usr/bin/env bash
# Checks that every tool PriyaOS development needs is installed (sub-phase 0.1).
# Exit 0 when all are found, 1 when any is missing.
#
# Usage: scripts/dev-check.sh          check and print a table
#        scripts/dev-check.sh --list   print the required tool names only
#
# Uses only bash builtins besides the tools it checks, and works with the
# bash 3.2 that ships with macOS.

set -u

common="git rustup cargo rustc rustfmt cargo-clippy python3 pipx sqlite3 shellcheck gitleaks typos lychee cargo-nextest cargo-llvm-cov cargo-deny cargo-insta dssim diff-cover"
case "${OSTYPE:-}" in
linux*) extra="clang mold quickshell cage sway grim wtype namcap" ;;
darwin*) extra="qml" ;;
*)
	echo "dev-check: unsupported OS '${OSTYPE:-unknown}'" >&2
	exit 2
	;;
esac
tools="$common $extra"

if [ "${1:-}" = "--list" ]; then
	for t in $tools; do echo "$t"; done
	exit 0
fi

# rustup installs proxies for rustfmt and cargo subcommands even when the
# component is missing, so for those only a working --version counts.
must_run() {
	case "$1" in rustfmt | cargo-*) return 0 ;; *) return 1 ;; esac
}

version_of() {
	local out
	case "$1" in
	cargo-*) out=$(cargo "${1#cargo-}" --version 2>&1 </dev/null) ;;
	grim | wtype | namcap | cage) out="" ;; # no reliable version flag
	gitleaks) out=$(gitleaks version 2>&1 </dev/null) ;;
	*) out=$("$1" --version 2>&1 </dev/null) ;;
	esac
	local rc=$?
	printf '%s' "${out%%$'\n'*}"
	return $rc
}

missing=""
count=0
for t in $tools; do
	count=$((count + 1))
	if ! command -v "$t" >/dev/null 2>&1; then
		printf '  %-8s %-15s\n' "MISSING" "$t"
		missing="$missing $t"
		continue
	fi
	if v=$(version_of "$t"); then
		printf '  %-8s %-15s %s\n' "ok" "$t" "$v"
	elif must_run "$t"; then
		printf '  %-8s %-15s %s\n' "BROKEN" "$t" "$v"
		missing="$missing $t"
	else
		printf '  %-8s %-15s %s\n' "ok" "$t" "(installed)"
	fi
done

if [ -n "$missing" ]; then
	case "$OSTYPE" in linux*) setup=scripts/dev-setup-arch.sh ;; *) setup=scripts/dev-setup-macos.sh ;; esac
	echo
	echo "Missing:$missing"
	echo "Run $setup to install them."
	exit 1
fi
echo
echo "All $count tools found."
