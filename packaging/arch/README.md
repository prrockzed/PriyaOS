# Arch packages

`PKGBUILD` builds PriyaOS from this checkout as two packages (sub-phase 0.14):

- `priya`: `priyad`, `priya`, the `priyad` user unit, the default settings and prompts under `/usr/share/priya/`, and shell completions.
- `priya-shell`: the Priya bar's Quickshell config with the `PriyaUI` module under `/usr/share/priya/shell/`, and the `priya-shell` user unit.

```text
cd packaging/arch
makepkg -si
```

`pkgver` is `0.0.<commits>.g<hash>`, so every commit installs as newer (makepkg writes it into the PKGBUILD; `git checkout packaging/arch/PKGBUILD` undoes that). `priya.install` only prints the next steps: it enables nothing and touches no user files. CI's `package` job builds both packages and runs namcap; `package-install` installs, upgrades and removes them in a fresh container (`scripts/package-check.sh`).
