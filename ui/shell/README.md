# Desktop shell

The Quickshell configuration: the Priya bar, and later the top bar, notifications and lock screen. Added in sub-phase 0.13.

## `priya/`: the Priya bar's own instance

- `shell.qml`: the root, run as its own Quickshell instance with `qs -p /usr/share/priya/shell` (the `priya-shell` user unit), next to Waybar or any other Quickshell config.
- `PriyaBar.qml`: a layer-shell panel (`Overlay`, namespace `priya-bar`, exclusive keyboard focus while open) at the top centre, `min(ui.bar.width_px, 60 % of the screen)` wide. It is made at start and only shown or hidden. `IpcHandler` target `priya-bar` has `toggle()`, `open()` and `close()`. Hyprland's focus grab closes it on a click outside; other compositors (sway, used by the journey tests) don't have one, so there Esc closes it.
- `SocketTransport.qml`: PriyaClient's transport, priyad's socket at `$XDG_RUNTIME_DIR/priya/priyad.sock`, one JSON value per line.

What the bar shows is `PriyaBarContent` and what it does is `BarSession`, both in PriyaUI and tested offscreen. PriyaUI is found through `QML_IMPORT_PATH` (the package installs it next to the shell in 0.14).

To try it from a checkout on a Wayland desktop with priyad running:

```text
QML_IMPORT_PATH=$PWD/ui qs -p ui/shell/priya
qs -p ui/shell/priya ipc call priya-bar toggle
```

Quickshell exists only on Linux, so `qmllint` checks these files in CI (`cargo xtask ui qmllint --require-quickshell`) and on the Zenbook, not on the Mac.
