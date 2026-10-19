# UI mockups

Pictures and prototypes of how PriyaOS should look, made before building the real screens. All names, prices and tasks in them are example data.

| Folder | What it is | How to open it |
|---|---|---|
| [prototype/](prototype/priyaos-live.html) | **Clickable dark-theme desktop.** Priya bar, plan preview, approvals, Task center, agent workspace, Rewind, Settings, kill switch. | Download `priyaos-live.html` and open it in any browser (needs internet only for fonts). Also published on claude.ai: [PriyaOS Live Prototype](https://claude.ai/artifact/AofC3kfww27ji6BuBQaC5G) (private to the owner). |
| [design-canvas/](design-canvas/) | **Design canvas source**: 8 still screens (desktop + Priya bar, answer card, approvals, Task center, agent workspace, Rewind, Settings, themes). | These `.dc.html` files only render inside claude.ai's Design canvas viewer, so they're kept here as a record. View them at [PriyaOS Desktop Prototype](https://claude.ai/artifact/7kTCZowzBhwX9zMhsrRz9Z) (private to the owner). |

## Keeping copies in sync

The claude.ai pages and these files don't sync. When a prototype changes on claude.ai, copy the new version here in the same commit; when the file here changes, republish it. The repo copy of the prototype is a full HTML document; the claude.ai version is the same page without the `<!doctype>`/`<head>` wrapper, which claude.ai adds when publishing.

These follow [docs/claude/architecture/11-design-system-and-ux.md](../../docs/claude/architecture/11-design-system-and-ux.md). Per-sub-phase mockups go in `ui/mockups/<phase>.<sub>/` from Phase 0 on.
