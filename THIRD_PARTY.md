# Third-party components

PriyaOS is licensed under [Apache-2.0](LICENSE). This file lists outside work that the repository includes or loads, with its licence. `cargo deny` checks that every Rust dependency has an allowed licence ([deny.toml](deny.toml)). This list covers everything outside Cargo, and `cargo xtask licenses` fails CI when a folder under `ui/` carries a licence file but has no row here.

| Component | Used in | How | Licence |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [Noto Sans Devanagari](https://fonts.google.com/noto/specimen/Noto+Sans+Devanagari) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [Lucide](https://lucide.dev)-style icons | `ui/mockups/` | Inline SVG shapes | ISC License, © Lucide Contributors (portions © Cole Bemis, Feather, MIT) |
| [Lucide](https://lucide.dev) icons, `lucide-static` 1.50.0 | `ui/PriyaUI/icons/` | The SVGs PriyaUI uses, stroke width set to 1.75 ([README](ui/PriyaUI/icons/README.md)) | ISC License, © Lucide Icons and Contributors (portions © Cole Bemis, Feather, MIT); text in [LICENSE](ui/PriyaUI/icons/LICENSE) |

Fonts for the desktop come from Arch packages, not vendored ([11-design-system-and-ux.md](docs/claude/architecture/11-design-system-and-ux.md)).
