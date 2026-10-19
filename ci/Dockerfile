# The Linux CI image (sub-phase 0.3): Arch, like the machines PriyaOS runs
# on, with every package pinned to one day of the Arch Linux Archive so a
# rebuild gives the same versions. Renovate bumps the base image digest;
# ARCHIVE_DATE is bumped by hand when we want newer packages.
#
# Built and pushed by .github/workflows/ci-image.yml as
# ghcr.io/prrockzed/priyaos-ci. Workflows use it by digest.

FROM docker.io/library/archlinux:base-devel@sha256:0ccc8b2e55c0c26a6e501e892cc6b1cdea7cfdf77e432941320148b4dabd2bff

ARG ARCHIVE_DATE=2026/10/18

LABEL org.opencontainers.image.source="https://github.com/prrockzed/PriyaOS" \
      org.opencontainers.image.description="PriyaOS CI: Arch, Rust, Qt 6, Quickshell and test tools" \
      org.opencontainers.image.licenses="Apache-2.0"

# Point pacman at the archive day, then install. The same list as
# scripts/dev-setup-arch.sh, minus desktop-only tools, plus CI helpers.
RUN echo "Server=https://archive.archlinux.org/repos/${ARCHIVE_DATE}/\$repo/os/\$arch" \
        > /etc/pacman.d/mirrorlist \
 && pacman -Syyuu --noconfirm \
 && pacman -S --noconfirm --needed \
        git rustup clang mold sqlite jq sudo \
        shellcheck gitleaks typos lychee \
        cargo-nextest cargo-deny cargo-llvm-cov cargo-insta \
        qt6-base qt6-declarative qt6-tools qt6-svg quickshell \
        cage sway grim wtype \
        inter-font ttf-jetbrains-mono noto-fonts \
        namcap \
 && pacman -Scc --noconfirm

# Toolchains live outside any home folder, so every user (root in CI jobs,
# builder for makepkg) shares them.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
COPY rust-toolchain.toml /tmp/toolchain/rust-toolchain.toml
RUN cd /tmp/toolchain \
 && rustup toolchain install \
 && rustup default "$(rustup show active-toolchain | cut -d' ' -f1)" \
 && cargo install --locked dssim \
 && rm -rf /tmp/toolchain "$CARGO_HOME/registry" "$CARGO_HOME/git" \
 && chmod -R a+rwX "$RUSTUP_HOME" "$CARGO_HOME"

# makepkg refuses to run as root.
RUN useradd --create-home builder \
 && echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder \
 && chmod 0440 /etc/sudoers.d/builder
