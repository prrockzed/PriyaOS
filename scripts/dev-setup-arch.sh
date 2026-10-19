#!/usr/bin/env bash
# Installs the tools PriyaOS development needs on Arch Linux (sub-phase 0.1).
# It shows what it will do and asks before each step. Only the pacman and
# Ollama steps use sudo; everything else installs into your home folder.
#
# Usage: bash scripts/dev-setup-arch.sh

set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)

if [ "$(id -u)" -eq 0 ]; then
	echo "Run this as your normal user. It asks for sudo only when it needs it." >&2
	exit 1
fi
if [ ! -f /etc/arch-release ]; then
	echo "This script is for Arch Linux. On macOS use scripts/dev-setup-macos.sh." >&2
	exit 1
fi

ask() {
	local a
	read -r -p "$1 [y/N] " a
	[[ $a == [yY]* ]]
}

packages=(
	base-devel git rustup clang mold sqlite python python-pipx shellcheck
	qt6-declarative qt6-tools qt6-svg quickshell
	cage sway grim wtype # headless UI tests and screenshots
	inter-font ttf-jetbrains-mono noto-fonts
	gitleaks typos lychee namcap
)
cargo_tools=(cargo-nextest cargo-llvm-cov cargo-deny cargo-insta dssim)
# QEMU and archiso arrive with Phase 2; not installed now.

echo "== Step 1 of 3: system packages (pacman, needs sudo)"
to_install=()
not_found=()
for p in "${packages[@]}"; do
	pacman -Qq "$p" >/dev/null 2>&1 && continue
	if pacman -Si "$p" >/dev/null 2>&1; then to_install+=("$p"); else not_found+=("$p"); fi
done
if [ ${#to_install[@]} -eq 0 ]; then
	echo "All system packages are already installed."
else
	echo "These packages will be installed:"
	printf '  %s\n' "${to_install[@]}"
	if pacman -Qq rust >/dev/null 2>&1 && [[ " ${to_install[*]} " == *" rustup "* ]]; then
		echo "Note: the 'rust' package is installed. pacman will offer to replace it with 'rustup'; say yes."
	fi
	if ask "Install them with sudo pacman -S --needed?"; then
		sudo pacman -S --needed "${to_install[@]}"
	else
		echo "Skipped."
	fi
fi
if [ ${#not_found[@]} -gt 0 ]; then
	echo "Not in the official repos, so not installed: ${not_found[*]}"
	echo "Install them yourself (for example from the AUR) if you need them."
fi

echo
echo "== Step 2 of 3: Rust components and developer tools (your home folder only, no sudo)"
echo "  rustup: stable toolchain if none is set, plus rustfmt, clippy, llvm-tools-preview"
echo "  cargo install --locked: ${cargo_tools[*]} (takes several minutes)"
echo "  pipx: diff-cover"
if ask "Go ahead?"; then
	if ! rustup default >/dev/null 2>&1; then rustup default stable; fi
	rustup component add rustfmt clippy llvm-tools-preview
	for t in "${cargo_tools[@]}"; do
		command -v "$t" >/dev/null 2>&1 || cargo install --locked "$t"
	done
	command -v diff-cover >/dev/null 2>&1 || pipx install diff-cover
	pipx ensurepath >/dev/null
	export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
else
	echo "Skipped."
fi

echo
echo "== Step 3 of 3 (optional): Ollama and one small local model (needs sudo)"
model=${OLLAMA_MODEL:-llama3.2:3b}
echo "  sudo pacman -S ollama; sudo systemctl enable --now ollama; ollama pull $model (about 2 GB)"
if ask "Install Ollama and pull $model?"; then
	pacman -Qq ollama >/dev/null 2>&1 || sudo pacman -S --needed ollama
	sudo systemctl enable --now ollama
	ollama pull "$model"
else
	echo "Skipped."
fi

echo
echo "== Git hooks"
bash "$here/install-hooks.sh"

echo
echo "== Checking"
bash "$here/dev-check.sh"
