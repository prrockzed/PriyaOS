#!/usr/bin/env bash
# covers: UX-02
# suite: ui
# acceptance: 0.13
#
# journey_ask_stream_copy_close: Super+Space's IPC opens the bar; a typed
# question and Enter stream an answer from mock-priyad; Ctrl+Shift+C
# copies it (pasted back into the input to check); Esc closes the bar.

# shellcheck source=ui/tests/journeys/lib.sh
. "$(dirname "$0")/lib.sh"
journey_needs
journey_start "$here/scripts/answer.json"

question="What's using my disk?"
answer="Your disk is mostly used by Docker images (18.4 GB)."

# What Super+Space runs.
bar toggle
wait_for '.visible and .phase == "idle"'
shot idle
type_text "$question"
press Return
wait_for '.phase == "streaming"'
shot streaming
wait_for '.phase == "done"'
shot answered
# shellcheck disable=SC2016 # a jq variable, not a shell one
asked '.method == "v1.intent.submit" and .params.text == $q' --arg q "$question"
[ "$(field .answer)" = "$answer" ] || fail "the answer reads: $(field .answer)"

ctrl_shift_key c
ctrl_key v
# shellcheck disable=SC2016 # a jq variable, not a shell one
wait_for '.input == $a' --arg a "$answer"

press Escape
wait_for '.visible == false'
