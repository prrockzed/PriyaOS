## What and why

<!-- One or two sentences. -->

**Sub-phase:** <!-- e.g. 0.5 --> · **Features:** <!-- IDs from docs/claude/features.md, e.g. CORE-01 -->

## Tests added

<!-- Test names. Every bug fix adds a test that failed before the fix. -->

## Screenshots

<!-- UI changes only: light and dark, before and after. Delete otherwise. -->

## Coverage

<!-- Only if diff coverage is below the bar: what isn't covered and why. -->

## Design records

<!-- ADRs or architecture files changed, or "none". -->

## Safety checklist

- [ ] Nothing runs against the host's real root, disks or boot setup (only the fake machine or the VM).
- [ ] No secrets, API keys or personal data in code, fixtures or recordings.
- [ ] `cargo xtask ci-local` passes.
