#!/usr/bin/env bash
# covers: UX-02
# suite: unit
# acceptance: 0.13
#
# hyprland_snippets_agree: the Lua snippet (Hyprland 0.55 and later) and
# the hyprland.conf one bind Super+Space to the same IPC call and start
# priya-shell after importing the same variables, so either config gets
# the same bar. Where luac exists, the Lua must parse.

set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
lua="$root/data/desktop/hyprland.lua"
conf="$root/data/desktop/hyprland.conf"
fail=0

toggle="qs -p /usr/share/priya/shell ipc call priya-bar toggle"
import="systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP"
start="systemctl --user start priya-shell"

expect() {
	if ! grep -qF -- "$2" "$1"; then
		echo "FAIL: $(basename "$1") lacks: $2"
		fail=1
	fi
}
expect "$lua" "hl.bind(\"SUPER + SPACE\", hl.dsp.exec_cmd(\"$toggle\"))"
expect "$lua" "hl.on(\"hyprland.start\""
expect "$lua" "\"$import\""
expect "$lua" "\" && $start\""
expect "$conf" "bind = SUPER, SPACE, exec, $toggle"
expect "$conf" "exec-once = $import && $start"

for luac in luac luac5.4; do
	if command -v "$luac" >/dev/null 2>&1; then
		"$luac" -p "$lua" || fail=1
		break
	fi
done

exit "$fail"
