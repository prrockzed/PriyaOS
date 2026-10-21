#!/usr/bin/env bash
# covers: DS-02
# suite: ui
# acceptance: 0.13
#
# journey_theme_switch_while_open: the open bar starts dark (ui.theme from
# mock-priyad) and turns light within 1 s of a setting.changed event, as
# when `priya settings set ui.theme light` runs in a terminal.

# shellcheck source=ui/tests/journeys/lib.sh
. "$(dirname "$0")/lib.sh"
journey_needs
journey_start "$here/scripts/theme.json"

bar open
wait_for '.visible and .theme == "dark"'
shot dark
tell_priyad '{"jsonrpc": "2.0", "id": 1, "method": "v1.settings.set", "params": {"key": "ui.theme", "value": "light"}}' >/dev/null
wait_for '.theme == "light"' 1
# The 200 ms colour transition, then the picture.
sleep 0.4
shot light
wait_for '.visible' 1
