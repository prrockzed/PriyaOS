#!/usr/bin/env bash
# covers: DS-06
# suite: ui
# acceptance: 0.13
#
# journey_keyboard_only: the whole path with keys alone, no pointer: open,
# type, Enter, Tab to Copy answer, Enter copies (pasted back into the
# input after Shift+Tab to check), Esc closes.

# shellcheck source=ui/tests/journeys/lib.sh
. "$(dirname "$0")/lib.sh"
journey_needs
journey_start "$here/scripts/answer.json"

answer="Your disk is mostly used by Docker images (18.4 GB)."

bar open
wait_for '.visible'
type_text "What's using my disk?"
press Return
wait_for '.phase == "done"'
# Tab reaches Copy answer (the answer has no code block before it), and
# Enter there copies; Shift+Tab goes back to the input.
press Tab
press Return
shift_key Tab
ctrl_key v
wait_for ".input == \"$answer\"" 3
shot copied

press Escape
wait_for '.visible == false'
