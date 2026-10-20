#!/usr/bin/env bash
# Publishes the screenshots of a pull request that differ from their
# baselines (the harness's before/, after/ and diff/ folders) or that change
# a baseline (the guard's baseline-before/, baseline-after/ and
# baseline-diff/, shown in preference) to the `ui-shots` branch, and writes
# a Markdown table that embeds them, for the PR report (sub-phase 0.11).
#
# Usage: publish-shots.sh <ui-shots folder> <owner/repo> <pr number> <run id>
# Needs GH_TOKEN with contents: write (SHOTS_REMOTE replaces the GitHub
# remote, for the script test). Writes <folder>/images.md; nothing when no
# picture changed. Only plainly named PNG files are published, at most 30.

set -euo pipefail
dir=$(cd "$1" && pwd)
repo=$2
pr=$3
run=$4
limit=30

[[ "$pr" =~ ^[0-9]+$ && "$run" =~ ^[0-9]+$ ]] || {
	echo "pull request and run must be numbers" >&2
	exit 1
}

is_png() {
	[ "$(head -c 8 "$1" | od -An -tx1 | tr -d ' \n')" = "89504e470d0a1a0a" ]
}

# Every picture named in before/ or after/, sorted, once.
pictures=()
while IFS= read -r picture; do
	[[ "$picture" =~ ^[A-Za-z0-9._-]+(/[A-Za-z0-9._@-]+)*\.png$ ]] || continue
	pictures+=("$picture")
done < <(for part in before after baseline-before baseline-after; do
	[ -d "$dir/$part" ] && (cd "$dir/$part" && find . -name '*.png' | sed 's|^\./||')
done | sort -u)
[ "${#pictures[@]}" -gt 0 ] || exit 0

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
git -C "$work" init -q
if [ -n "${SHOTS_REMOTE:-}" ]; then
	git -C "$work" remote add origin "$SHOTS_REMOTE"
else
	# The token goes in a header, never in the URL or the logs.
	git -C "$work" remote add origin "https://github.com/${repo}.git"
	auth=$(printf 'x-access-token:%s' "$GH_TOKEN" | base64 | tr -d '\n')
	git -C "$work" config http.https://github.com/.extraheader "AUTHORIZATION: basic $auth"
fi

dest="pr-$pr/run-$run"
shown=("${pictures[@]:0:$limit}")
for attempt in 1 2 3 4 5; do
	if git -C "$work" fetch -q --depth 1 origin ui-shots 2>/dev/null; then
		git -C "$work" checkout -q -B ui-shots FETCH_HEAD
	else
		# The first picture ever; a fresh name each try, as a branch from an
		# earlier try may exist.
		git -C "$work" checkout -q --orphan "new-$attempt"
	fi
	for picture in "${shown[@]}"; do
		# A changed baseline shows old against new baseline; otherwise the
		# baseline against this rendering.
		prefix=""
		if [ -f "$dir/baseline-before/$picture" ] || [ -f "$dir/baseline-after/$picture" ]; then
			prefix="baseline-"
		fi
		for part in before after diff; do
			source="$dir/$prefix$part/$picture"
			if [ -f "$source" ] && is_png "$source"; then
				mkdir -p "$work/$dest/$part/$(dirname "$picture")"
				cp "$source" "$work/$dest/$part/$picture"
			fi
		done
	done
	git -C "$work" add -A
	git -C "$work" -c user.name="github-actions[bot]" \
		-c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
		commit -q -m "Screenshots for pull request $pr (run $run)" || true
	# Another run may have pushed meanwhile: fetch again and retry.
	if git -C "$work" push -q origin HEAD:ui-shots 2>/dev/null; then
		break
	fi
	[ "$attempt" -lt 5 ] || {
		echo "couldn't push to ui-shots" >&2
		exit 1
	}
	sleep $((attempt * 2))
done

base="https://raw.githubusercontent.com/$repo/ui-shots/$dest"
{
	echo "| Picture | Before | After | Diff |"
	echo "|---|---|---|---|"
	for picture in "${shown[@]}"; do
		row="| \`$picture\` |"
		for part in before after diff; do
			if [ -f "$work/$dest/$part/$picture" ]; then
				row="$row <img src=\"$base/$part/$picture\" width=\"240\"> |"
			else
				row="$row — |"
			fi
		done
		echo "$row"
	done
	if [ "${#pictures[@]}" -gt "$limit" ]; then
		echo
		echo "…and $((${#pictures[@]} - limit)) more in the \`ui-shots\` artifact."
	fi
} >"$dir/images.md"
