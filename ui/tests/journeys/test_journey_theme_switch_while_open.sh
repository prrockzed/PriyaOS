#!/usr/bin/env bash
# covers: DS-02
# suite: ui
# acceptance: 0.13
#
# journey_theme_switch_while_open: the open bar starts light (ui.theme from
# mock-priyad; dark is the default, so this shows the setting was read)
# and turns dark within 1 s of a setting.changed event, as when `priya
# settings set ui.theme dark` runs in a terminal.

# shellcheck source=ui/tests/journeys/lib.sh
. "$(dirname "$0")/lib.sh"
journey_needs
journey_start "$here/scripts/theme.json"

bar open
wait_for '.visible and .theme == "light"'
shot light
tell_priyad '{"jsonrpc": "2.0", "id": 1, "method": "v1.settings.set", "params": {"key": "ui.theme", "value": "dark"}}' >/dev/null
WAIT=1 wait_for '.theme == "dark"'
# The 200 ms colour transition, then the picture.
sleep 0.4
shot dark
# Still open: a theme change doesn't close the bar.
WAIT=1 wait_for '.visible'
