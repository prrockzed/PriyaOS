#!/usr/bin/env bash
# covers: UX-02
# suite: ui
# acceptance: 0.13
#
# journey_ask_cancel_mid_stream: Esc while an answer streams sends
# v1.intent.cancel; the partial answer stays, marked stopped; a second Esc
# closes the bar.

# shellcheck source=ui/tests/journeys/lib.sh
. "$(dirname "$0")/lib.sh"
journey_needs
journey_start "$here/scripts/cancel.json"

bar open
wait_for '.visible'
type_text "write a long essay about the rivers of India"
press Return
wait_for '.answer | contains("Himalayas")'
press Escape
wait_for '.phase == "stopped"' 3
asked '.method == "v1.intent.cancel" and .params.intent_id == "i1"'
[ "$(field .answer)" = "The rivers of India begin in the Himalayas" ] ||
	fail "the partial answer reads: $(field .answer)"
wait_for '.visible' 1
shot stopped

press Escape
wait_for '.visible == false'
