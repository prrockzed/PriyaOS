# Third-party components

PriyaOS is licensed under [Apache-2.0](LICENSE). This file lists outside work that the repository includes or loads, with its licence. `cargo deny` checks that every Rust dependency has an allowed licence ([deny.toml](deny.toml)). This list covers everything outside Cargo, and `cargo xtask licenses` fails CI when a folder under `ui/` carries a licence file but has no row here.

| Component | Used in | How | Licence |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [Noto Sans Devanagari](https://fonts.google.com/noto/specimen/Noto+Sans+Devanagari) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [Lucide](https://lucide.dev)-style icons | `ui/mockups/` | Inline SVG shapes | ISC License, © Lucide Contributors (portions © Cole Bemis, Feather, MIT) |

Planned, added when they land: [Lucide](https://lucide.dev) icons (ISC) vendored for the desktop UI; fonts for the desktop come from Arch packages ([11-design-system-and-ux.md](docs/claude/architecture/11-design-system-and-ux.md)).
