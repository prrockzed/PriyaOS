# UI tests

Screenshot baselines and accessibility checks for PriyaUI and the shell. Added in sub-phase 0.11.

Every QML file under `ui/` must pass `qmllint` with no warnings: the pre-commit hook checks staged files, and CI's `ui` job runs `cargo xtask ui qmllint --require-quickshell` on all of them. The shell's files import Quickshell, which only Linux has, so on the Mac the hook leaves them to CI.
