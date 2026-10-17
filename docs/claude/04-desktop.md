# 04 — Desktop integration (Hyprland)

Week 1 uses existing tools (fuzzel, Waybar, mako, kitty) instead of a custom shell. A custom shell (Quickshell/AGS) is after week 1.

All files live in `desktop/` and are installed to `/usr/share/priya/desktop/`. The ISO copies them into `/etc/skel/.config/`. On an existing install, `priya desktop install` adds a single `source = /usr/share/priya/desktop/hyprland.conf` line to `~/.config/hypr/hyprland.conf` (idempotent, backs up the file first) and prints the Waybar snippet to add.

## 1. Command bar — `priya ask`

`desktop/hyprland.conf`:

```
bind = SUPER, SPACE, exec, priya ask
bind = SUPER, P, togglespecialworkspace, priya
windowrulev2 = float, class:^(priya-review)$
windowrulev2 = size 70% 75%, class:^(priya-review)$
windowrulev2 = center, class:^(priya-review)$
windowrulev2 = workspace special:priya silent, class:^(priya-logs)$
```

`priya ask`:
1. Opens `fuzzel --dmenu --prompt "Priya › " --lines 6`, pre-filled with the 5 most recent task titles (so the user can re-run one).
2. On submit: `task.create` with `kind=agent`, default agent, `cwd` = `$HOME` (or the cwd of the focused kitty window if `hyprctl activewindow -j` shows class `kitty` and its pid's cwd is readable).
3. Opens `kitty --class priya-logs -e priya logs -f <id>` on the `special:priya` workspace (silent), so the user can peek with SUPER+P.
4. `notify-send "Priya" "Working on: <title>"`.

## 2. Waybar module

`desktop/waybar-module.jsonc`:

```jsonc
"custom/priya": {
  "exec": "priya status --waybar",
  "return-type": "json",
  "interval": 3,
  "on-click": "kitty --class priya-review -e priya review",
  "on-click-right": "hyprctl dispatch togglespecialworkspace priya"
}
```

`priya status --waybar` prints one JSON line:

```json
{"text": "◆ 2 ⟳ · 1 ✓", "tooltip": "running: set up docker…\nrunning: fix lint…\nready for review: update system", "class": "review"}
```

`class` is `idle` | `running` | `review` | `failed` (most important wins) for CSS styling.

## 3. Notifications — `priya-notify`

A user service (`priya-notify.service`, `systemd --user`, `WantedBy=graphical-session.target`) subscribes to `events.subscribe` and sends notifications with actions via `notify-send --wait --action=...` (libnotify ≥ 0.8; mako supports actions):

| Event | Notification | Actions |
|---|---|---|
| task → `review` | "Ready for review: <title>" + one-line summary | `Review` → open review TUI; `Discard` |
| task → `failed` | "Task failed: <title>: <error>" | `Logs` |
| task → `merged_pending_reboot` | "Restart to finish applying system changes" | `Restart now` → `systemctl reboot` (after a second confirm notification) |
| task tainted | "Task read untrusted content: <reason>" | — |

## 4. Status for the agent's work

Week 1 "watching an agent" = its log in a kitty window on `special:priya`. The log view (`priya logs -f`) renders events readably: agent text, `▸ tool: Bash  pacman -S docker`, cost so far, elapsed time.
