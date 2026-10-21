#!/usr/bin/env bash
# covers: OS-01
# suite: unit
#
# pkgver_from_git_describe: the PKGBUILD names a build 0.0.<commits>.g<hash>
# from the checkout it sits in, so every commit gets a newer version
# pacman can compare, and it builds both split packages.

set -u
# Its own repository: nothing from a hook's environment may leak in.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
	GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_PREFIX
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
fail=0

repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT
mkdir -p "$repo/packaging/arch"
cp "$root/packaging/arch/PKGBUILD" "$repo/packaging/arch/"
git -C "$repo" init -q
for n in 1 2 3; do
	echo "$n" >"$repo/file"
	git -C "$repo" add -A
	git -C "$repo" -c user.name=t -c user.email=t@t -c commit.gpgsign=false \
		-c core.hooksPath=/dev/null commit -qm "commit $n"
done
hash=$(git -C "$repo" rev-parse --short=7 HEAD)

# makepkg sets startdir to the PKGBUILD's folder.
got=$(
	# shellcheck disable=SC2034 # read by the PKGBUILD
	startdir="$repo/packaging/arch"
	# shellcheck source=/dev/null
	. "$repo/packaging/arch/PKGBUILD"
	pkgver
)
if [ "$got" != "0.0.3.g$hash" ]; then
	echo "FAIL: pkgver gave '$got', expected '0.0.3.g$hash'"
	fail=1
fi

names=$(
	# shellcheck source=/dev/null
	. "$root/packaging/arch/PKGBUILD"
	# shellcheck disable=SC2154 # set by the PKGBUILD
	echo "${pkgname[*]}"
	declare -F package_priya package_priya-shell >/dev/null && echo "with both functions"
)
if [ "$names" != "priya priya-shell
with both functions" ]; then
	echo "FAIL: expected the split packages priya and priya-shell, got: $names"
	fail=1
fi

exit "$fail"
