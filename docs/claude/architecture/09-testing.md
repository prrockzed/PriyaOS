# 09 — Testing strategy

Every phase ends with a **phase gate**: its tests at all applicable levels pass, its demo works, its feature-catalog rows are updated. No phase starts before the previous gate passes (parallel work inside a phase is fine).

## 1. Levels

| Level | Where | What | How to run |
|---|---|---|---|
| **L0 Unit** | Mac or Linux | Pure logic: state machines, plan DAG, router scoring, mode resolution, ChangeSet classification, 3-way decisions, policy evaluation with fixtures, NL-settings patch validation, parsers | `cargo test --workspace` (Linux-only crates skip on macOS) |
| **L1 Contract** | Mac or Linux | Every adapter against its port's shared suite: providers (recorded HTTP cassettes), agents (recorded stream fixtures), snapshot backend, sandbox runner, notifier, STT/TTS | `cargo test -p <crate> --features contract` |
| **L2 Fake machine** | Zenbook (root) or CI | Helper + priyad against a btrfs loop image with a pacstrapped minimal Arch: forks, runs, ChangeSets, merges, conflicts, swaps, rollback, Rewind | `sudo scripts/test-fake-machine.sh [scenario]` |
| **L3 VM** | QEMU/KVM on the Zenbook | Built ISO → install into a VM disk that contains a fake Windows layout → scripted end-to-end scenarios across reboots | `scripts/vm-test.sh out/priyaos-*.iso [suite]` |
| **L4 Real laptop** | Zenbook | Phase-specific manual checklist, only after L3 passes | checklist in each phase file |

## 2. Special suites

- **Router evaluation** (`priya-ai`): a versioned set of ~300 labelled prompts (category, difficulty, modality, privacy) with expected roles; reports routing accuracy, simulated cost, and regression vs the last release. Online quality is measured from real outcomes ([04-priya-ai.md](04-priya-ai.md) §6).
- **Live model smoke tests**: opt-in (`PRIYA_LIVE=1` + keys), tiny prompts per enabled provider, run before releases only.
- **Agent end-to-end**: fixed small tasks ("create a Rust hello world with a passing test", "install jq and write a script using it") run per agent adapter in L2, checking ChangeSet contents and verifier results.
- **Prompt-injection corpus** (Phase 2 basic, Phase 14 full): repos/web pages/files containing hostile instructions; asserts that secrets stay masked, egress is blocked, taint is set, merges require confirmation.
- **Chaos**: kill `priyad` / `priya-helper` / the VM at every journaled step of merge, swap, rollback; assert recovery reports and no corruption.
- **Performance budgets**: fork creation < 2 s, ChangeSet for 10k touched files < 10 s, intent → first token in the bar < 1.5 s with a local fast model, idle priyad RSS < 80 MiB.
- **Voice/vision**: recorded audio and screenshot fixtures with expected transcripts/extractions (L1); live mic/screen checks are L4.

## 3. CI

GitHub Actions:
- every push: `fmt`, `clippy -D warnings`, L0 + L1 on `ubuntu-latest` and `macos-latest`;
- nightly and on `main`: L2 subset on `ubuntu-latest` with sudo (busybox-based fake root; pacman scenarios skipped);
- L3 runs on the Zenbook (self-hosted runner optional later).

## 4. Safety rules for testing

- Development never merges, swaps, rolls back or runs `doctor --fix` on the host's real root. L2/L3 only.
- No `mkfs`, `dd`, `wipefs`, `sgdisk`, `parted`, `cfdisk`, `grub-install`, `efibootmgr` on real devices — VM or the owner's manual install only.
- Live model tests use small budgets (`--max-usd 0.50` per suite).
