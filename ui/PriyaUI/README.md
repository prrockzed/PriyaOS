# PriyaUI

The shared QML component library every PriyaOS screen is built from (sub-phase 0.11, [11-design-system-and-ux.md](../../docs/claude/architecture/11-design-system-and-ux.md)). New components land here and in the gallery first.

- `Theme.qml`: the generated token singleton (`cargo xtask tokens`); never edit it.
- Building blocks: `Icon` (a tinted Lucide icon from [icons/](icons/README.md)) and `FocusRing` (the keyboard focus ring).
- Components use only QtQuick, QtQuick.Controls.Basic (imported `as T`) and QtQuick.Effects, never Quickshell, so they run in the gallery and in offscreen tests on the Mac and Linux.
- `cargo xtask ui lint` keeps them on tokens, translatable and named for screen readers. Tests are in `ui/tests/components/`.
- `client/PriyaClient.qml`: the JSON-RPC client for priyad (0.13). Its transport is injected (a Quickshell socket in the shell, `ui/tests/MockTransport.qml` in tests); it matches replies to requests, routes notifications by `intent_id` or subscription, and reconnects with a 250 ms → 5 s backoff.
