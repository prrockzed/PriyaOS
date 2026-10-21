# shellcheck shell=bash
# Journey harness for the Priya bar (0.13): the real shell config
# (ui/shell/priya) in headless sway, talking to mock-priyad, driven by
# `qs … ipc call` and wtype, captured with grim. Sourced by
# test_journey_*.sh; each journey gets its own XDG_RUNTIME_DIR, so the
# bar finds mock-priyad where it looks for priyad.
#
# Pictures and logs land in target/journeys/<journey>/.

set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../../.." && pwd)
journey=$(basename "$0" .sh)
journey=${journey#test_}
out="$root/target/journeys/$journey"
pids=()

fail() {
	echo "FAIL ($journey): $*" >&2
	exit 1
}

# Without a Wayland toolchain (macOS) there is nothing to run; in CI a
# missing tool is a failure, not a skip.
journey_needs() {
	local missing=()
	for tool in sway qs wtype grim jq cargo; do
		command -v "$tool" >/dev/null || missing+=("$tool")
	done
	[ ${#missing[@]} -eq 0 ] && return 0
	[ -n "${CI:-}" ] && fail "missing ${missing[*]}"
	echo "skipped ($journey): needs ${missing[*]}"
	exit 0
}

# Waits up to $2 seconds (default 10) for the command in $1 to succeed.
wait_until() {
	local deadline=$((SECONDS + ${2:-10}))
	until eval "$1" >/dev/null 2>&1; do
		[ $SECONDS -ge $deadline ] && return 1
		sleep 0.1
	done
}

bar() { qs -p "$root/ui/shell/priya" ipc call priya-bar "$@"; }
state() { bar state; }
field() { state | jq -r "$1"; }

# Waits for the bar's state to match the jq test in $1.
wait_for() {
	wait_until "state | jq -e '$1'" "${2:-10}" ||
		fail "waited for $1; the bar is at $(state)"
}

# Each wtype first waits a little: its keymap reaches the compositor after
# it starts, and a key sent before that is lost.
keys() { wtype -s 150 "$@"; }
type_text() { keys "$1"; }
press() { keys -k "$1"; }
# ctrl_key v, shift_key Tab, ctrl_shift_key c
ctrl_key() { keys -M ctrl -k "$1" -m ctrl; }
shift_key() { keys -M shift -k "$1" -m shift; }
ctrl_shift_key() { keys -M ctrl -M shift -k "$1" -m shift -m ctrl; }

# Whether the bar sent a request whose JSON matches the jq test in $1.
asked() {
	jq -e -s "any(.[]; $1)" "$work/requests.ndjson" >/dev/null ||
		fail "the bar never sent $1; it sent: $(jq -c '{method, params}' "$work/requests.ndjson")"
}

shot() { grim "$out/$1.png"; }

# Notes the bar's state after a step, in steps.txt, to read when a journey
# fails.
checkpoint() { echo "$1: $(state)" >>"$out/steps.txt"; }

# Sends one request to mock-priyad as another client would (a settings
# change from a terminal), and prints the reply.
tell_priyad() {
	python3 - "$XDG_RUNTIME_DIR/priya/priyad.sock" "$1" <<'PY'
import socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
s.sendall(sys.argv[2].encode() + b"\n")
print(s.makefile().readline(), end="")
PY
}

# Prints sway's Wayland socket, or fails while there is none.
wayland_socket() {
	for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
		[ -S "$socket" ] && echo "$socket" && return 0
	done
	return 1
}

# Starts mock-priyad with the script $1, then sway, then the bar's shell,
# and waits until the bar is connected.
journey_start() {
	rm -rf "$out" && mkdir -p "$out"
	work=$(mktemp -d)
	export XDG_RUNTIME_DIR="$work/run"
	mkdir -m 700 "$XDG_RUNTIME_DIR"
	trap journey_end EXIT

	cargo build -q -p priya-testkit --bin mock-priyad || fail "building mock-priyad"
	"$root/target/debug/mock-priyad" --socket "$XDG_RUNTIME_DIR/priya/priyad.sock" \
		--script "$1" --log "$work/requests.ndjson" >"$out/mock.log" 2>&1 &
	pids+=($!)

	# Arch's sway has file capabilities, which a container won't exec; a
	# plain copy has none and runs the same.
	cp "$(command -v sway)" "$work/sway"
	WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
		WLR_HEADLESS_OUTPUTS=1 "$work/sway" -c "$here/sway.conf" >"$out/sway.log" 2>&1 &
	pids+=($!)
	# The lock file comes before the socket.
	wait_until wayland_socket || fail "sway didn't start: $(tail -n 5 "$out/sway.log")"
	WAYLAND_DISPLAY=$(basename "$(wayland_socket)")
	export WAYLAND_DISPLAY

	QML_IMPORT_PATH="$root/ui" QT_QPA_PLATFORM=wayland LIBGL_ALWAYS_SOFTWARE=1 \
		FONTCONFIG_FILE="$root/ui/tests/fontconfig/fonts.conf" \
		QT_LOGGING_RULES="priya.bar.perf.debug=true;quickshell.*.debug=true" \
		qs -p "$root/ui/shell/priya" >"$out/shell.log" 2>&1 &
	pids+=($!)
	wait_until "state | jq -e .connected" 20 || {
		diagnose
		fail "the bar didn't connect (see $out/diagnosis.txt)"
	}
}

# What a failed start leaves to look at.
diagnose() {
	{
		echo "--- $XDG_RUNTIME_DIR"
		ls -la "$XDG_RUNTIME_DIR" "$XDG_RUNTIME_DIR/priya"
		echo "--- the bar's state"
		bar state
		echo "--- mock-priyad, asked directly"
		tell_priyad '{"jsonrpc": "2.0", "id": 1, "method": "v1.settings.get", "params": {"key": "ui.theme"}}'
	} >"$out/diagnosis.txt" 2>&1
	cat "$out/diagnosis.txt" >&2
}

journey_end() {
	local status=$?
	[ -f "$work/requests.ndjson" ] && cp "$work/requests.ndjson" "$out/"
	for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null; done
	wait 2>/dev/null
	rm -rf "$work"
	if [ $status -ne 0 ]; then
		echo "--- the bar's log" >&2
		tail -n 30 "$out/shell.log" >&2
	else
		echo "ok: $journey"
	fi
	exit $status
}
