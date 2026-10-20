# Design tokens

`tokens.json` holds every colour, size, font and spacing PriyaOS uses (sub-phase 0.10). `generate/` turns it into theme files for the desktop, apps, terminal and boot screens. See [11-design-system-and-ux.md](../../docs/claude/architecture/11-design-system-and-ux.md).

- `tokens.json` and `tokens.schema.json`: the values and their rules.
- `generate/`: the `priya-themegen` library. `cargo xtask tokens` uses it to write `../PriyaUI/Theme.qml` and `build/tokens.css` and to check contrast; `--check` fails if they are out of date.
- `build/tokens.css`: generated CSS custom properties for HTML mockups. Don't edit it.
