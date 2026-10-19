# Third-party components

PriyaOS is licensed under [Apache-2.0](LICENSE). This file lists outside work that the repository includes or loads, with its licence. Once the Cargo workspace exists (sub-phase 0.2), `cargo-deny` checks that every Rust dependency has an allowed licence, and this list covers anything bundled outside Cargo.

| Component | Used in | How | Licence |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | `ui/mockups/` | Loaded from Google Fonts at view time, not bundled | SIL Open Font License 1.1 |
| [Lucide](https://lucide.dev)-style icons | `ui/mockups/` | Inline SVG shapes | ISC License, © Lucide Contributors (portions © Cole Bemis, Feather, MIT) |

Planned, added when they land: [Lucide](https://lucide.dev) icons (ISC) and Noto Sans Devanagari (SIL OFL 1.1) for the desktop UI ([11-design-system-and-ux.md](docs/claude/architecture/11-design-system-and-ux.md)).
