# Translations

Qt `.ts` files for the UI and Fluent `.ftl` files for everything else (ADR-020). Hindi first, in Phase 10.

- `priya_en.ts`: every `qsTr` string of `ui/PriyaUI` and `ui/shell`, collected by `cargo xtask ui i18n` (lupdate, without line numbers). CI's `ui` job and `ci-local` run `cargo xtask ui i18n --check`, which fails when a string is missing or left over. Translations start from a copy of this file.
- The UI shows its own translated words for each API error kind (`ui/PriyaUI/ErrorMessages.js`) and falls back to the daemon's English `user_message` for kinds it doesn't know.
