#!/usr/bin/env bash
# Installs the tools PriyaOS development needs on macOS (sub-phase 0.1).
# It shows what it will do and asks before each step. Nothing uses sudo.
#
# Quickshell and Hyprland don't exist on macOS, so UI work on the Mac is
# limited to PriyaUI components in the gallery (run with `qml`). Everything
# that needs Linux is built and tested on the Zenbook and in CI.
#
# Usage: bash scripts/dev-setup-macos.sh
# Works with the bash 3.2 that ships with macOS.

set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

if [ "$(id -u)" -eq 0 ]; then
	echo "Run this as your normal user." >&2
	exit 1
fi
if [ "$(uname -s)" != "Darwin" ]; then
	echo "This script is for macOS. On Arch use scripts/dev-setup-arch.sh." >&2
	exit 1
fi
if ! command -v brew >/dev/null 2>&1; then
	echo "Homebrew is needed first. Install it from https://brew.sh, then run this again." >&2
	exit 1
fi

ask() {
	local a
	read -r -p "$1 [y/N] " a
	case "$a" in [yY]*) return 0 ;; *) return 1 ;; esac
}

brew_pkgs="rustup qt sqlite python pipx shellcheck gitleaks typos-cli lychee"
cargo_tools="cargo-nextest cargo-llvm-cov cargo-deny cargo-insta dssim"

echo "== Step 1 of 2: Homebrew packages"
echo "  brew install $brew_pkgs"
if ask "Go ahead?"; then
	# shellcheck disable=SC2086 # word list on purpose
	brew install $brew_pkgs
else
	echo "Skipped."
fi

# Homebrew's rustup is keg-only (it conflicts with the rust formula), so its
# bin folder isn't on PATH by default.
rustup_bin="$(brew --prefix rustup 2>/dev/null)/bin"
export PATH="$rustup_bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

echo
echo "== Step 2 of 2: Rust components and developer tools (your home folder only)"
echo "  rustup: stable toolchain if none is set, plus rustfmt, clippy, llvm-tools-preview"
echo "  cargo install --locked: $cargo_tools (takes several minutes)"
echo "  pipx: diff-cover"
if ask "Go ahead?"; then
	if ! rustup default >/dev/null 2>&1; then rustup default stable; fi
	rustup component add rustfmt clippy llvm-tools-preview
	for t in $cargo_tools; do
		command -v "$t" >/dev/null 2>&1 || cargo install --locked "$t"
	done
	command -v diff-cover >/dev/null 2>&1 || pipx install diff-cover
	pipx ensurepath >/dev/null
else
	echo "Skipped."
fi

profile="$HOME/.zprofile"
line="export PATH=\"$rustup_bin:\$HOME/.cargo/bin:\$PATH\""
if ! grep -qsF "$rustup_bin" "$profile"; then
	echo
	echo "To use Rust in new terminals, this line must be in $profile:"
	echo "  $line"
	if ask "Add it now?"; then
		printf '\n# Rust (added by PriyaOS dev-setup-macos.sh)\n%s\n' "$line" >>"$profile"
	fi
fi

echo
echo "== Checking"
bash "$here/dev-check.sh"
