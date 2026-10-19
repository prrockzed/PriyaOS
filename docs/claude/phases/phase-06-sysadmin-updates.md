# Phase 6 — Looking after the computer

**Goal.** PriyaOS keeps the machine healthy. Everyday problems (slow, hot, battery, disk full, failed services, broken packages, Wi-Fi, Bluetooth, audio, displays) are diagnosed from real evidence and explained in plain language. Fixes run in the mode the rules choose, and every fix is checked afterwards. System updates are applied to a copy, boot-tested, repaired by an agent if they break something, and only then applied on restart. A Health page shows the state of the machine with one-click fixes.

**Usable at the end:** Super+Space → "why is my laptop slow?" → an answer with evidence and a **Fix** button → fix runs → "Fixed: CPU back to 6 %". `priya update` (or the top-bar badge) → update tested in Priya Sandbox, breakage fixed by an agent, review → restart → "Update finished, everything works". The Health page shows a score and fixes. "Set up a dev environment for ~/projects/api" installs the toolchain and writes `.priya.toml`.

**Depends on:** Phase 2 (helper, booted runs 2.8, ChangeSets 2.9–2.10, system merge and rollback 2.12, read-only and direct modes 2.13), Phase 3 (tasks, mode rules, Cedar, approvals, budgets, taint), Phase 4 (agents, native agent, checks and `.priya.toml` 4.10, `priya mcp` 4.11, Task center and review GUI 4.13), Phase 5 (intent pipeline, cards, top bar, notification centre, Settings app).

**Features:** ADM-03, ADM-04, ADM-05, ADM-06, ADM-07, ADM-08, ADM-09, ADM-10, ADM-11, ADM-12, ADM-13.

**Read first:** [03 §1 modes, §4 running and boot checks, §5 ChangeSets and merge, §6 limits](../architecture/03-execution-and-sandbox.md), [10 §2 base OS](../architecture/10-base-os-and-distro.md), [02 §5 modules](../architecture/02-core.md), [04 §4 roles, §8 tool-output trimming](../architecture/04-priya-ai.md), [05 §2 capabilities, §4 approvals, §5 taint](../architecture/05-security.md), [06 §6 supervision, §8 verification](../architecture/06-orchestration.md), [07 §1 answers and cards](../architecture/07-interfaces.md), [09 §2–3, §8](../architecture/09-testing.md), [11 §4 Health, Settings, Cards, §5 autonomy](../architecture/11-design-system-and-ux.md), ADR-001, ADR-008.

## Design notes for this phase

- **Where diagnostics run.** A `read_only` run inside an nspawn snapshot cannot see the host's processes, journal, devices or network state. System tools therefore run on the live host as the user under Landlock read-only (the "direct with Landlock read-only" variant of `read_only`, [03 §1](../architecture/03-execution-and-sandbox.md)). The few facts that need root (SMART, kernel ring buffer, `btrfs filesystem usage`) come from new **read-only helper operations** with fixed arguments and capped output. No tool takes a free-form shell string.
- **Live actions.** Some fixes change runtime state, not files: restarting a unit, stopping a process, `rfkill unblock`, switching the default audio sink, cleaning the package cache. These are not ChangeSets ([03 §5](../architecture/03-execution-and-sandbox.md) covers files only). This phase adds a typed, allow-listed `LiveAction` set. Each action has an approval tier, an inverse where one exists, and a `sysadmin.action_performed` event that later appears in the Rewind timeline (Phase 7). This is an extension of P4; the plan includes an ADR for it.
- **Testability.** Every system fact goes through a `SystemProbe` port: `live` runs the allow-listed command, `recorded` replays fixtures captured on the Zenbook. Playbooks and diagnosis are tested against recorded facts; fixes are tested on the fake machine or in the VM.

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 6.1 | System tools for agents | Phase 5 | `priya sys facts cpu` and agents (via `priya mcp`) read processes, disk, journal, network, audio, power, packages safely |
| 6.2 | Diagnosis flow | 6.1 | "Why is X?" → plain answer with evidence, a Fix button, and a check that the fix worked |
| 6.3 | Performance playbooks | 6.2 | Runaway process, memory pressure, full disk, slow boot, battery drain and overheating are diagnosed and fixed |
| 6.4 | Services and logs playbooks | 6.2 | Failed services and containers are explained and fixed; any log line or error can be explained |
| 6.5 | Packages and dependencies playbooks | 6.2 | Broken libraries, file conflicts, keyring errors, stale locks, broken AUR packages and kernel/driver issues are explained and fixed |
| 6.6 | Hardware playbooks | 6.2 | Wi-Fi, Bluetooth, audio, display and brightness problems are diagnosed and fixed |
| 6.7 | Update check and Arch news | 6.1 | Top-bar badge "12 updates"; Arch news needing manual steps is summarised |
| 6.8 | `priya update` pipeline | 6.7 | `priya update` upgrades a copy, boot-tests it, shows a review, applies on restart |
| 6.9 | Update fix loop | 6.8 | An update that breaks a service is repaired by an agent in the copy before the owner sees it |
| 6.10 | Health page and maintenance | 6.3–6.6 | Health page with a score, checklist and Fix buttons; safe maintenance tasks |
| 6.11 | Dev environment setup | 6.1 | "Set up a dev environment for this project" installs toolchains in the sandbox and writes `.priya.toml` |
| 6.12 | Phase gate | all | Real update on the Zenbook; playbooks verified on real hardware |

---

### 6.1 — System tools for agents
**Outcome:** a `sysadmin` module exposes read-only system tools to Priya AI, to agents (through `priya mcp`) and to the CLI (`priya sys facts <area>`); every tool returns structured JSON plus a trimmed text form.
**Features:** ADM-04, ADM-07, ADM-09.
**Depends on:** Phase 5.

**Tasks**
- [ ] `modules/sysadmin` process module skeleton: `module.toml` with `methods = ["sysadmin.facts", "sysadmin.diagnose", "sysadmin.fix", "health.report"]`, `tools = ["sys_processes", "sys_memory", "sys_disk", "sys_journal", "sys_units", "sys_network", "sys_wifi", "sys_bluetooth", "sys_audio", "sys_display", "sys_power", "sys_packages", "sys_kernel", "sys_containers", "sys_hardware"]`, `default_mode = "read_only"`, settings schema `[sysadmin]`.
- [ ] `SystemProbe` port (`fn run(&self, probe: ProbeId, args: ProbeArgs) -> Result<ProbeOutput>`) with adapters `live` and `recorded`; contract suite; add the port to the P2 table in [00-principles](../architecture/00-principles.md).
- [ ] Allow-listed probes with fixed argv, 5 s timeout and a 64 KiB output cap: `ps`/`/proc` (top N by CPU, RSS, per systemd unit), `/proc/pressure/*`, `free`, `df`, `findmnt`, `journalctl -o json` (filters: unit, priority, boot, since; system journal readable by `priya-admin` members through the `wheel`/`systemd-journal` ACL), `systemctl --failed` (system and `--user`), `systemctl show`, `nmcli -t`, `ip -j`, `resolvectl`, `iw`, `rfkill -J`, `bluetoothctl`, `wpctl status`, `pw-dump` (summarised), `hyprctl -j monitors`, `/sys/class/drm`, `/sys/class/backlight`, `upower -d`, `/sys/class/power_supply`, `powerprofilesctl`, `sensors -j`, `pacman -Q/-Qdt/-Qm/-Qk/-Dk`, `/var/log/pacman.log` tail, `uname`, `lsmod`, `lspci -k`, `lsusb`, `docker`/`podman ps -a --format json` and `inspect` (only if installed).
- [ ] Read-only helper operations in `priya-helper` (fixed arguments, `priya-admin` only, output capped): `facts.smart {device}` (`smartctl -j -a` / NVMe log), `facts.kernel_log {since_boot}` (`dmesg --json`, needed because `kernel.dmesg_restrict=1`), `facts.btrfs_usage` (`btrfs filesystem usage -b`, `btrfs device stats`).
- [ ] Output shaping per tool for the token optimizer ([04 §8](../architecture/04-priya-ai.md)): head + tail + error lines for logs, top-N tables, units always explicit (MiB, %, W, °C).
- [ ] Redaction pass on all probe output (secret patterns from 3.6, plus Wi-Fi PSKs, MAC addresses and SSIDs hashed unless the user enables "show network names to AI").
- [ ] `priya sys facts <area> [--json]` and `priya sys record --scenario <name>` (captures every probe into `modules/sysadmin/tests/fixtures/facts/<name>/` with a `RECORDED.md` template, for building fixtures on the Zenbook).
- [ ] Register the tools with `priya mcp` (TOOL-01 server from 4.11) so every agent can call them, read-only inside forks.

**Tests**
- Unit: each probe parser on recorded outputs (Zenbook + fake machine + VM), output shaping limits, redaction (PSK, MAC, API keys in journal lines).
- Contract: `SystemProbe` suite against `live` (fake machine) and `recorded` adapters; helper read-only ops reject unknown devices and non-admin callers.
- Integration (fake machine): `priya sys facts disk` matches `df` inside the booted fake machine; a probe that hangs is killed at 5 s with a plain error; Landlock denies any write attempt from the probe runner.
- Security: probe argv cannot be influenced by model output beyond typed enum/filters (fuzz `ProbeArgs`); `facts.smart` with `../` or a non-block device refused.
- Perf: `priya sys facts all` < 3 s on the Zenbook.
- Regression: `sys_probe_contract`, `sys_facts_redaction`, `helper_facts_readonly_ops`.

**Done when**
- [ ] All 15 tools return valid JSON on the fake machine and on the Zenbook (recorded set committed).
- [ ] No probe can write, and none runs a shell (asserted by the security test).
- [ ] An agent in a sandbox run calls `sys_disk` through `priya mcp` and gets an answer.

---

### 6.2 — Diagnosis flow
**Outcome:** a question about the machine becomes a read-only diagnosis: facts gathered, findings computed, a plain-language explanation with evidence, one or more fixes offered, and after a fix a re-check that says whether the problem is gone.
**Features:** ADM-04, ADM-07, ADM-13.
**Depends on:** 6.1.

**Tasks**
- [ ] Playbook format `modules/sysadmin/playbooks/<id>.toml`: `symptoms` (phrases, keywords), `facts` (probe list), `rules` (deterministic checks producing findings, e.g. `disk.root.free_pct < 5 → disk_full`), `fixes` (each: kind `task` with an intent text / `live_action` / `setting`, risk, reversibility), `verify` (probe + expected predicate), `explain` hints. Schema exported with `cargo xtask schemas`.
- [ ] Playbook matcher: keyword/embedding match of the request to symptoms, plus "no playbook" fallback where Priya AI (role `smart`) picks probes itself, bounded to 8 probe calls.
- [ ] Diagnosis engine `sysadmin.diagnose {text, context}`: read-only task → facts → rules → Priya AI explanation with **cited evidence** (each claim references a fact id) and confidence → `Diagnosis {findings, explanation, fixes, verify}`; emits `sysadmin.diagnosed`.
- [ ] Privacy: facts carry a privacy class; journal and process lists default to `system` class, which follows the user's privacy routing (setting `sysadmin.logs_local_only`, default off; Cedar `UseModel` check).
- [ ] `LiveAction` set (ADR in [decisions.md](../architecture/decisions.md) + [03](../architecture/03-execution-and-sandbox.md) §5 note): `unit.restart|start|stop {unit, scope}`, `process.stop {pid, signal}`, `rfkill.unblock {type}`, `audio.set_default {node}`, `audio.set_profile {card, profile}`, `nm.connection_up {id}`, `power.profile {name}`, `pkgcache.clean {keep}`, `journal.vacuum {size}`, `pacman.unlock`. Each declares tier (`notify`/`ask`/`confirm`), inverse (or "can't be undone"), privileged (via new helper op `live.action`) or user-level.
- [ ] Fix execution `sysadmin.fix {diagnosis_id, fix_id}`: `task` fixes become normal tasks (mode by [03 §1.1](../architecture/03-execution-and-sandbox.md) rules); `live_action` fixes go through Cedar + approvals; `setting` fixes go through `v1.settings.nl_change`.
- [ ] Verification step (ADM-13): after the fix, re-run the playbook's `verify` probes, compare before/after, emit `sysadmin.verified {fixed: bool, evidence}`; not fixed → offer the next fix or "Ask Priya to look deeper" (agent run in read-only mode).
- [ ] Bar and CLI: `priya sys diagnose "<text>"`; intent routing via module `intents` examples ("why is my laptop slow", "my wifi doesn't work", "what does this error mean").
- [ ] Diagnosis eval set `eval/sysadmin/` (fixture facts + expected root cause + expected fix id), run in the nightly `eval` suite.

**UI**
- Answer card variant `diagnosis` (new `PriyaUI` component, gallery first): one-line verdict, "What I found" evidence list (each item expandable to the raw fact and the command that produced it), fix options as buttons with mode chip and reversibility line, "Check again". Verification result card: "Fixed" / "Still happening" with before/after values.
- States: gathering facts (live list of probes), explanation streaming, no problem found, low confidence ("I'm not sure; here's what I checked"), fix running, verify running, fixed, not fixed, error (probe failed), offline (local model, shorter explanation), long content (30 findings), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: Tab through evidence and fixes, Enter runs the focused fix, D toggles details. Screen-reader names: "Diagnosis: <verdict>", "Evidence <n> of <m>", "Fix: <label>, <mode>, <reversibility>".
- 👤 Mockup approval (`ui/mockups/6.2/`) before building; journey sign-off ("ask → evidence → fix → verified").

**Human in the loop:** nothing changes until the user presses a fix (or autonomy "Just do it" allows `notify`-tier live actions). Every fix shows its mode, tier and whether it can be undone. "Not fixed" never hides; the user decides the next step.

**Tests**
- Unit: rule evaluation, playbook schema validation, evidence-citation checker (every claim cites a fact), LiveAction tier table, verify predicate evaluation.
- Contract: playbook files load and validate; `LiveAction` inverse pairs round-trip on a fake executor.
- Integration (fake machine): "why is my disk full" with the seeded full-disk fixture → finding `disk_full` → `pkgcache.clean` → verify passes; a live action denied by policy is explained.
- UI: `diagnosis` card states × themes × scales; journey test with mock priyad.
- Eval: on `eval/sysadmin/` (≥ 40 scenarios at the end of 6.6), root cause correct at top-1 ≥ 90 %, explanation judged correct by a `review`-role model from another family ≥ 90 %, zero fixes proposed that contradict the evidence.
- Regression: `diagnose_cites_evidence`, `fix_then_verify_reports_result`, `live_action_requires_tier`.

**Done when**
- [ ] A diagnosis without a matching playbook still produces an evidence-backed answer within 8 probe calls.
- [ ] Every fix path ends with a verification result shown to the user.
- [ ] 👤 Owner understands three diagnosis cards without help.

---

### 6.3 — Performance playbooks
**Outcome:** slow, hot, out of memory, out of disk, slow boot and short battery life are diagnosed with numbers and fixed or improved, each with a verified before/after.
**Features:** ADM-04, ADM-10, ADM-13.
**Depends on:** 6.2.

**Tasks**
- [ ] Seeded fixture kit `tests/fixtures/sysadmin/<scenario>/seed.sh` applied to a fork of the fake machine (never the host), plus recorded facts for hardware-only data.
- [ ] Playbook `perf.runaway_process` — fixture: unit `priya-test-runaway.service` running `stress-ng --cpu 2`; finding: one process > 150 % CPU for > 2 min; fixes: `process.stop` (ask), limit with `CPUQuota` drop-in (task, sandbox), disable autostart (task); verify: load average and process CPU.
- [ ] Playbook `perf.memory_pressure` — fixture: `stress-ng --vm 1 --vm-bytes 90%` in a 2 GiB cgroup; finding: PSI memory `some avg10 > 20`; fixes: stop the hog (ask), enable zram via `/etc/systemd/zram-generator.conf` (task, sandbox, applies on restart); verify: PSI below 5.
- [ ] Playbook `perf.disk_full` — fixture: root at 97 % from 1.5 GiB of old packages in `@pkg`, 600 MiB journal, a 1 GiB file in `~/Downloads`, 3 old `@priya-prev-*`; fixes: `pkgcache.clean {keep: 1}` (notify), `journal.vacuum {size: 200M}` (notify), "show my biggest files" (file list card, never auto-delete), remove old previous systems except the newest (confirm, helper); verify: free ≥ 15 %.
- [ ] Playbook `perf.slow_boot` — recorded `systemd-analyze blame` / `critical-chain` with `NetworkManager-wait-online.service` 30 s; fix: disable wait-online (task, sandbox, restart); verify after restart (VM).
- [ ] Playbook `power.battery_drain` (ADM-10) — recorded facts from the Zenbook: 14 W idle discharge, one process with high wakeups, brightness 100 %, profile `performance`, blur on; suggestions ranked by estimated watts saved: `power.profile power-saver`, brightness, close the app, blur off on battery; battery-health suggestion: charge limit 80 % (`charge_control_end_threshold`, task, sandbox: udev rule) when supported; verify: discharge rate after 10 min (L4 only).
- [ ] Playbook `perf.thermal` — recorded `sensors` with CPU at 95 °C and throttling flags; explanation, top consumers, fan state; suggestions only (no firmware changes).
- [ ] "Speed up my laptop" umbrella intent that runs the performance playbooks' fact sets once and returns a ranked list.

**Human in the loop:** process stops and package-cache cleaning show exactly what will be removed or stopped; no user file is ever deleted by a performance playbook.

**Tests**
- Unit: each playbook's rules on its recorded facts (positive and negative fixtures, e.g. high CPU from a compile the user started → explained, not "runaway").
- Integration (fake machine): runaway, memory pressure and disk-full scenarios end to end: seed → diagnose → fix → verify passes; teardown leaves the fake machine clean.
- E2E (VM): slow-boot fix applied through system merge, VM restarted, `systemd-analyze` shows the gain.
- Eval: 12 performance scenarios in `eval/sysadmin/perf/` meet the 6.2 thresholds.
- Regression: `pb_runaway_fixed_verified`, `pb_memory_pressure_zram`, `pb_disk_full_frees_space`, `pb_slow_boot_vm`.

**Done when**
- [ ] All six playbooks pass their fixtures; runaway, memory pressure and disk full are fixed and verified on the fake machine.
- [ ] Battery suggestions on the Zenbook recording rank the biggest saving first (checked by the owner in 6.12).

---

### 6.4 — Services and logs playbooks
**Outcome:** "why did nginx fail?", "why does my container keep stopping?" and "what does this error mean?" get correct plain answers; config-caused failures are fixed in the sandbox and verified by restarting the service.
**Features:** ADM-05, ADM-07, ADM-13.
**Depends on:** 6.2.

**Tasks**
- [ ] Playbook `svc.config_error` — fixture: nginx installed in the fake machine with a syntax error in `/etc/nginx/nginx.conf`; facts: `systemctl status`, journal for the unit, `nginx -t` (allow-listed config tests: `nginx -t`, `sshd -t`, `apachectl configtest`, `named-checkconf`); fix: edit in sandbox → in-fork config test → merge → `unit.restart` (ask); verify: unit `active`.
- [ ] Playbook `svc.port_conflict` — fixture: `priya-test-http.service` on 8080 plus the target service on 8080; finding from `ss -ltnp` and the unit log; fixes: change one port (task) or stop the other unit (ask).
- [ ] Playbook `svc.user_unit_loop` — fixture: `~/.config/systemd/user/sync.service` with a missing `ExecStart` binary, restart loop; fix in home (direct mode with safety snapshot by default rules); verify: unit stable for 30 s.
- [ ] Playbook `svc.timer_never_runs` — fixture: timer enabled but its service masked; explanation and fix.
- [ ] Playbook `container.failed` (ADM-05) — recorded `docker inspect` fixtures: exit 137 with `OOMKilled: true`, exit 1 with a missing env var in logs, image pull auth error; explanations and fixes (raise memory limit in compose file — task in the project; add env var — asks for the value, never invents secrets).
- [ ] Log and error explainer (ADM-07): `sysadmin.explain {text | journal_ref}` classifies a log line or pasted error as harmless / worth watching / needs action, with the reason; seeded set of 30 common journal lines (ACPI BIOS errors, `iwlwifi` firmware fallbacks, `bluetoothd` SAP plugin, `gkr-pam`, Xwayland warnings, kernel taint messages).
- [ ] "Explain this" from the Phase 5 context capture: selected text in kitty or the focused terminal's last error → explainer.
- [ ] Journal summariser for "what went wrong since I booted?": groups errors by unit, dedupes repeats, links each group to a playbook when one matches.

**Human in the loop:** service restarts are `ask` tier with the unit name and reason; config edits go through the normal review.

**Tests**
- Unit: config-test output parsers; `ss` parser; journal grouping and dedupe; explainer classification on the 30-line set (deterministic part).
- Integration (fake machine, booted nspawn): nginx config error, port conflict, user unit loop, timer — each seeded, diagnosed, fixed in sandbox, merged, restarted, verified.
- Eval: explainer classification accuracy ≥ 90 % on the 30-line set; container explanations correct on all 3 recorded fixtures; judged by `review` role from another family.
- Chaos: kill the fix task between merge and `unit.restart` → after priyad restart the user is told the restart is still pending; no double restart.
- Regression: `pb_nginx_config_fixed`, `pb_port_conflict`, `pb_user_unit_loop`, `explain_log_lines_set`, `container_oom_explained`.

**Done when**
- [ ] Four service fixtures are fixed and verified on the fake machine without manual steps beyond approvals.
- [ ] Explainer thresholds met; harmless lines are never presented as problems.

---

### 6.5 — Packages and dependencies playbooks
**Outcome:** broken shared libraries, file conflicts, dependency conflicts, signature errors, a stale pacman lock, AUR packages broken by an update, and kernel/driver issues are explained and fixed in the sandbox.
**Features:** ADM-06, ADM-11, ADM-13.
**Depends on:** 6.2.

**Tasks**
- [ ] Local test repo `tests/fixtures/repo/` of tiny packages built with `makepkg` (`pkg-a`, `pkg-b` conflicting with `pkg-a`, `libpriyatest` 1 and 2 with a soname bump, `priya-test-svc` 1 and 2), served as a `file://` repo inside the fake machine for deterministic package scenarios.
- [ ] Playbook `pkg.missing_library` — fixture: `libpriyatest` upgraded alone (partial upgrade), a binary fails with `error while loading shared libraries`; facts: `pacman -Qk`, `ldd` (allow-listed on the failing binary only), `pacman.log`; fix: full upgrade via the 6.8 pipeline or rebuild the dependent package; explanation of why partial upgrades break Arch.
- [ ] Playbook `pkg.file_conflict` — fixture: unowned `/usr/bin/jq` makes `pacman -S jq` fail with "exists in filesystem"; facts: `pacman -Qo`; fix in sandbox: move the stray file to `/var/lib/priya/quarantine/` (in the ChangeSet), install, merge.
- [ ] Playbook `pkg.dependency_conflict` — fixture: install `pkg-b` while `pkg-a` is installed; plain-language choice card ("keep A" / "replace with B"), never decided by the AI alone.
- [ ] Playbook `pkg.keyring` — fixture: package signed by a key not in the fixture keyring; fix: update `archlinux-keyring` first in the sandbox, then retry (packaged into the update pipeline as a pre-step).
- [ ] Playbook `pkg.stale_lock` — fixture: `/var/lib/pacman/db.lck` with no pacman process; fix: `pacman.unlock` live action (ask) that the helper performs only when no `pacman`/`paru`/`makepkg` process exists.
- [ ] Playbook `pkg.aur_rebuild` — fixture: a foreign package linked against `libpriyatest.so.1` after the bump; fix: rebuild with `paru` in the sandbox (tainted: AUR source; PKGBUILD diff shown in review).
- [ ] Kernel and driver playbook `kernel.issues` (ADM-11): running kernel ≠ installed (`/usr/lib/modules/$(uname -r)` missing → "restart needed" explanation, USB/Wi-Fi modules won't load until restart); DKMS build failure (recorded `pacman.log` + `dkms status`); "possibly missing firmware" warnings from `mkinitcpio` (harmless explanation); firmware load errors from `facts.kernel_log` (links to 6.6).
- [ ] `.pacnew`/`.pacsave` detector feeding Health (6.10) and the update review (6.8).

**Human in the loop:** conflicts that need a choice are always asked in plain language; AUR rebuilds show the PKGBUILD diff and carry the untrusted label.

**Tests**
- Unit: `pacman` error-message parsers (file conflict, dependency conflict, signature, lock, missing library); `ldd` parser; DKMS status parser.
- Integration (fake machine): each package scenario seeded from the local repo → diagnose → fix in sandbox → merge (system on fake reboot via nspawn boot) → verify (`pacman -Dk` clean, binary runs).
- Security: AUR rebuild task is tainted and its system merge needs `confirm`; `pacman.unlock` refused while a fake `pacman` process runs.
- Eval: 10 package/kernel scenarios in `eval/sysadmin/pkg/` meet the 6.2 thresholds.
- Regression: `pb_missing_library`, `pb_file_conflict`, `pb_dep_conflict_asks`, `pb_keyring_first`, `pb_stale_lock`, `pb_aur_rebuild_tainted`, `pb_kernel_restart_needed`.

**Done when**
- [ ] All seven package scenarios fixed and verified on the fake machine.
- [ ] No package fix ever runs `pacman -Sy <pkg>` (partial upgrade) — enforced by a policy check on the fix plan.

---

### 6.6 — Hardware playbooks
**Outcome:** Wi-Fi, Bluetooth, audio, external display and brightness problems are diagnosed from recorded or live facts and fixed; reversible hardware scenarios are checked on the real Zenbook.
**Features:** ADM-09, ADM-10, ADM-11, ADM-13.
**Depends on:** 6.2.

**Tasks**
- [ ] 👤 Record hardware fixtures on the Zenbook with `priya sys record --scenario <name>` while the owner creates each condition (airplane mode on, Bluetooth off, headphones plugged, HDMI display attached at wrong scale, Wi-Fi with wrong password). Claude provides the script; the owner runs it.
- [ ] Playbook `wifi.blocked` — facts: `rfkill -J`, `nmcli radio`; fix: `rfkill.unblock wifi` (notify; inverse: block); verify: `nmcli -t -f STATE general` = connected within 20 s.
- [ ] Playbook `wifi.backend_conflict` — fixture on the fake machine: `/etc/NetworkManager/conf.d/wifi_backend.conf` set to `iwd` while `iwd.service` is disabled and `wpa_supplicant` active; fix in sandbox (config + unit enablement), merge, restart NetworkManager (ask). This scenario is reused by Phase 7's undo-by-prompt demo.
- [ ] Playbook `wifi.auth_failed` — recorded NM journal "secrets were required"; fix: re-enter password (the user types it in a system prompt; it never passes through the model).
- [ ] Playbook `wifi.firmware_missing` (ADM-11) — recorded `iwlwifi: no suitable firmware found`; fix: install the matching `linux-firmware-*` package in the sandbox (restart needed).
- [ ] Playbook `bt.off_or_blocked` — `bluetooth.service` disabled or adapter soft-blocked; fixes: enable service (sandbox), unblock (notify). Playbook `bt.headset_no_audio` — headset connected in HFP profile only; fix: `audio.set_profile` to A2DP (notify, inverse available).
- [ ] Playbook `audio.no_sound` — recorded cases: default sink is HDMI, sink muted, `sof-firmware` missing (`sof-audio-pci-intel … firmware file not found`); fixes: `audio.set_default` (notify), unmute (notify), install `sof-firmware` (sandbox, restart).
- [ ] Playbook `display.external` — recorded `hyprctl -j monitors` with an external display at 1.0 scale on a 4K panel, or not listed while `/sys/class/drm` shows it connected; fix: monitor line in `~/.config/hypr` (home change, live merge) and `hyprctl reload`; verify from `hyprctl monitors`.
- [ ] Playbook `display.brightness` — recorded backlight paths (OLED panels may expose `intel_backlight` or need a kernel parameter); explain and propose the kernel parameter change as a sandboxed system change with restart.
- [ ] Peripheral catch-all: `lsusb`/`lspci -k` "no driver bound" finding → plain explanation and package suggestion.

**Human in the loop:** passwords are entered only in system prompts, never in the bar; changes needing a restart say so before they run.

**Tests**
- Unit: `rfkill`, `nmcli`, `wpctl`, `pw-dump`, `hyprctl`, `upower`, backlight parsers on Zenbook recordings and on synthetic edge cases (two Wi-Fi adapters, no battery, no audio card).
- Integration (fake machine): `wifi.backend_conflict` and `bt.off_or_blocked` (service part) seeded, fixed in sandbox, merged, verified by unit state; the rest against recorded facts with a fake `LiveAction` executor.
- Eval: 14 hardware scenarios in `eval/sysadmin/hw/` meet the 6.2 thresholds.
- L4 (Zenbook, owner-run, all reversible): airplane mode → "my Wi-Fi doesn't work" → fixed; Bluetooth off → fixed; wrong default sink → fixed; external display at wrong scale → fixed. Results recorded in this file.
- Regression: `pb_wifi_blocked`, `pb_wifi_backend_conflict`, `pb_bt_service`, `pb_audio_default_sink`, `pb_display_scale`.

**Done when**
- [ ] All hardware playbooks pass on recorded facts; two pass end to end on the fake machine.
- [ ] Four L4 scenarios pass on the Zenbook.

---

### 6.7 — Update check and Arch news
**Outcome:** a daily read-only check shows how many updates are waiting (repo, AUR, Flatpak, `[priyaos]`), flags security fixes and kernel updates, and summarises Arch news items that need manual steps for packages actually installed.
**Features:** ADM-03, ADM-11.
**Depends on:** 6.1.

**Tasks**
- [ ] `modules/updates` process module skeleton: `methods = ["updates.check", "updates.run", "updates.status"]`, `subscribes = ["timer.daily", "system.swapped"]`, `default_mode = "sandbox"` and a shipped locked mode rule `when = { module = "updates" } mode = "sandbox" lock = true`; uses the `PackageManager` port.
- [ ] Unprivileged check: sync databases into a temporary `DBPath` (same approach as `checkupdates`), list repo updates with versions and download sizes; `paru -Qua` for AUR when enabled; `flatpak remote-ls --updates` when Flatpak is present.
- [ ] Security flags: fetch `https://security.archlinux.org/issues/all.json`, match installed versions (as `arch-audit` does), mark updates that fix known issues.
- [ ] Arch news: fetch `https://archlinux.org/feeds/news/`, store seen items in module state; classify each item (role `fast`) as "needs manual steps for your packages" / "informational" using the installed package list; news text is untrusted input — it is shown and used for explanation only, never turned into commands directly.
- [ ] Events: `updates.available {repo, aur, flatpak, security, kernel, download_bytes, news_attention}`; top-bar badge count via the Phase 5 status item; notification only for security fixes or news needing attention (quiet otherwise).
- [ ] Respect conditions: skip on metered connections (NetworkManager `Metered`), skip below 20 % battery when not charging, record "last checked"; offline → silent retry.
- [ ] Settings → Updates page (11 §4): check frequency, include AUR, include Flatpak, notify for security only / all, hold packages (`IgnorePkg` with a warning about partial upgrades).
- [ ] CLI: `priya update --check [--json]`, `priya update news`.

**UI**
- Top-bar update badge; Updates settings page; "Updates waiting" card in the bar (counts, security and kernel highlights, news needing attention with a plain summary and a link to the full item).
- States: up to date, updates waiting, security updates, news needs attention, checking, offline/last checked, metered (skipped), error, long content (300 packages), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: badge focusable from the top bar; Enter opens the card; "Update now" is the primary button. Screen-reader names: "12 updates waiting, 2 security fixes".
- 👤 Mockup approval (`ui/mockups/6.7/`) before building; gallery sign-off.

**Tests**
- Unit: temp-DB check result parsing, version comparison (`vercmp` semantics, epochs), security-issue matching, RSS parsing, metered/battery gating.
- Contract: `PackageManager` port suite extended with `check_updates` (pacman adapter against the local test repo; recorded `paru -Qua` output).
- Integration (fake machine): local test repo publishes `pkg-a` 2 → check shows 1 update, badge event emitted; check never modifies `/var/lib/pacman/sync` of the live system.
- Eval: news classification on 25 recorded Arch news items with labels (manual intervention or not, relevant to a fixture package list) ≥ 92 % accuracy.
- Security: a news item containing "run `curl … | sh`" is rendered as text and never reaches a tool call (injection corpus entry).
- Regression: `updates_check_readonly`, `updates_badge_event`, `news_manual_intervention_flag`, `updates_security_flag`.

**Done when**
- [ ] On the Zenbook the badge shows the same count as `checkupdates`, within one daily cycle.
- [ ] Live sync databases are untouched by checks (asserted).

---

### 6.8 — `priya update` pipeline
**Outcome:** `priya update` (or "update my computer", or the badge) runs the full update in a Priya Sandbox fork, boot-tests it, shows an update review, and applies it on the next restart; afterwards the system checks itself and offers rollback if something broke.
**Features:** ADM-03, ADM-14.
**Depends on:** 6.7.

**Tasks**
- [ ] Update task plan (one task, sequential steps sharing one fork): preflight → keyring → download → upgrade → AUR/Flatpak → `.pacnew` handling → boot check → review. CLI `priya update [--no-aur] [--no-flatpak] [--yes-restart-later]`; module method `updates.run`.
- [ ] Preflight: free space ≥ download + 2 GiB and above the 10 % fork threshold ([03 §6](../architecture/03-execution-and-sandbox.md)), AC power or battery ≥ 30 %, no pending system merge, `/boot` location check — when `/boot` is on the ESP, kernel packages are excluded with `--ignore` and the review explains why ([03 §2](../architecture/03-execution-and-sandbox.md)).
- [ ] In the fork: `pacman -Sy archlinux-keyring` first when it has an update, then `pacman -Syuw` (downloads land in the shared `@pkg` cache) and `pacman -Su --noconfirm`; capture hook output (mkinitcpio, DKMS, GRUB) as step logs. Signed packages from configured mirrors do not taint the task; AUR builds do.
- [ ] AUR: `paru -Sua` in the fork as the user (never root), PKGBUILD diffs since the last build attached to the review; Flatpak: system installs in the fork, user installs as a home change.
- [ ] `.pacnew` handling in the fork: for each new `.pacnew`, Priya AI (role `smart`) proposes a three-way merge of the user's file and the new default; shown as its own diff group; untouched configs are replaced automatically only when the live file equals the old packaged default.
- [ ] Boot check (2.8) on the final fork state: new failed units vs the live baseline, hardware-bound units skipped (setting list), kernel image and initramfs present in the fork's `/boot`, `grub.cfg` references them; failures go to 6.9.
- [ ] Update review (Task center review, 4.13) with an update-specific summary: package groups (kernel and drivers, desktop, system, apps, AUR, Flatpak) with old → new versions, security fixes, news needing attention, `.pacnew` merges, boot-check result, download size; actions **Restart now / Restart later / Discard**.
- [ ] Merge: system part via 2.12 (swap on restart, `done_pending_reboot`); home part live. On drift refusal, offer **Redo on current system** (new fork, packages already cached). Restart is always asked; never automatic.
- [ ] Post-restart verification: priyad sees `system.swapped`, waits for the session, runs the health subset (failed units vs pre-update baseline, network up, default audio sink present, displays present) → notification "Update finished, everything works" or "<unit> stopped working — Roll back / Ask Priya to fix" (rollback = `v1.system.rollback`, on restart).
- [ ] VM update test fixtures: `tests/vm/update-dates.toml` with an Arch Linux Archive base date and target date (e.g. base `2026/08/01`, target `2026/09/01`); the VM image is installed from `https://archive.archlinux.org/repos/<base>/$repo/os/$arch` and updated against the target date, so real updates (including a kernel) exist and runs are reproducible.
- [ ] Full-kernel boot check for updates that change the kernel, initramfs or bootloader: boot the fork's root in a QEMU/KVM VM (virtio, same kernel cmdline) to the login prompt before offering the merge; falls back to the container check when KVM is unavailable

**UI**
- Update progress in the Task center (steps with plain names: "Getting updates", "Installing on a copy", "Testing that it starts", "Ready for you"), update review, restart prompt, post-restart notification.
- States: preflight blocked (low space / battery / pending merge), downloading (bytes, ETA), upgrading, boot check running, boot check failed (→ 6.9), ready for review, conflicts (`.pacnew`), drift (redo offered), restart pending, finished OK, finished with problems, rolled back, offline, long content (300 packages), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: R restart now, L later, D discard in the review; package groups expandable with arrow keys. Screen-reader names on every group and action ("Kernel and drivers, 3 updates, expand").
- 👤 Mockup approval (`ui/mockups/6.8/`) before building; journey sign-off ("badge → update → review → restart → finished").

**Human in the loop:** the user decides when to restart; the review says plainly what changes and that the old system stays available (`priya rollback`; from Phase 9 also the "previous system" boot entry). Discard leaves the live machine untouched.

**Tests**
- Unit: step plan builder, preflight rules, package grouping, `.pacnew` auto-replace rule (live == old default), boot-check comparison with skip list, post-restart baseline diff.
- Integration (fake machine): local test repo update `pkg-a` 1 → 2 and `priya-test-svc` 1 → 2 (working) → fork, boot check passes, review summary correct, merge, fake reboot, post-restart check green; drift during review → refusal → redo succeeds; discard leaves no subvolume behind.
- E2E (VM): ALA base → target real `pacman -Syu` including a kernel → boot check → merge → restart → post-restart check green → `priya rollback` → restart → old kernel running (`uname -r`) → roll forward by updating again.
- Chaos: kill priyad, the helper, and the VM at every journaled update step (after download, mid-upgrade in fork, during swap, before restart) → recovery report, live system either old or new, never mixed.
- Perf: update of 150 packages on the VM: fork + upgrade + boot check overhead over plain `pacman -Syu` ≤ 90 s (excluding downloads).
- Regression: `update_pipeline_fake_machine`, `update_drift_redo`, `update_vm_ala_restart_rollback`, `update_chaos_swap`, `update_post_restart_check`.

**Done when**
- [ ] VM: a real ALA update merges, survives restart, passes the post-restart check, rolls back cleanly and rolls forward again.
- [ ] No update ever changes the live system before the user chooses restart (asserted in integration by comparing `@` generation).

---

### 6.9 — Update fix loop
**Outcome:** when the boot check of an update finds newly failed units (or hook errors), an agent repairs the fork before the owner sees the review; the review shows the updates and the fixes as separate groups, and honestly says when it could not fix something.
**Features:** ADM-03, ADM-13.
**Depends on:** 6.8.

**Tasks**
- [ ] Freeze a read-only snapshot after the upgrade step so the review can show two layers: "Updates" (base → upgraded) and "Fixes Priya made" (upgraded → fixed), each a ChangeSet in the same task.
- [ ] Fix step: agent run in the same fork (`WorkspaceShare::SharedWith`), assigned by Priya Orchestration for category `sysadmin`, with a goal built from the failing units, their journal excerpts, `.pacnew` files and Arch news for the involved packages; prompt rule: prefer config fixes, never downgrade unless asked, never edit files under `/usr`.
- [ ] Limits: max 2 fix attempts, each followed by a full boot check; budget per attempt (default 15 min, cost cap from settings); stuck detection from 4.9.
- [ ] Outcome handling: fixed → review shows both layers with "Fixed: <unit> starts again"; not fixed → review offers **Apply anyway** (confirm tier, failing unit named), **Hold back <package>** (only for leaf packages, with a partial-upgrade warning), **Discard**.
- [ ] Seeded fixable fixtures in the local test repo: `priya-test-svc` 2 renames a config key so the service fails until `/etc/priya-test-svc.conf` is migrated; `priya-test-daemon` 2 needs a system user that isn't created (missing sysusers entry). Unfixable fixture: `priya-test-crash` 2 whose binary exits 139.
- [ ] Record outcomes for Priya AI learning (fixed / not fixed / reverted later) per agent and model.
- [ ] Update notification copy: "I updated 47 apps and fixed one setting that the update broke. Ready to restart."

**Human in the loop:** fixes are always shown separately and can be discarded with the whole update; "Apply anyway" requires typing the confirmation word.

**Tests**
- Unit: goal builder (bounded size, includes only relevant journal lines), attempt counter, outcome-to-review mapping.
- Integration (fake machine): config-rename fixture → fixed on attempt 1 or 2 → boot check green → merge; sysusers fixture → fixed; crash fixture → stops after 2 attempts, review says "not fixed", nothing merged without confirm.
- Eval: each fixable fixture fixed in ≥ 9 of 10 runs with the default agent assignment; unfixable fixture never reported as fixed (0 of 10).
- E2E (VM): ALA update plus an injected broken unit drop-in in the fork → fix loop repairs it → restart → green.
- Regression: `update_fix_config_rename`, `update_fix_sysusers`, `update_unfixable_honest`, `update_fix_layers_in_review`.

**Done when**
- [ ] Eval thresholds met on the fake machine; the VM injection scenario passes nightly.
- [ ] No review ever claims "fixed" while the boot check shows the unit failed (asserted).

---

### 6.10 — Health page and maintenance
**Outcome:** `priya health` and the Health page show a score and a checklist in plain language; each warning has a Fix (a safe maintenance task or live action) or an "Ask Priya" link; a weekly read-only health check notifies only when something needs attention.
**Features:** ADM-12, SEC-24.
**Depends on:** 6.3, 6.4, 6.5, 6.6.

**Tasks**
- [ ] `health.report` and `priya health [--json]`: checks with weights grouped as Storage, System, Packages, Hardware, Priya — free space per filesystem, btrfs device errors, last scrub age, SMART/NVMe health, failed units, restart needed (running kernel ≠ installed), orphans (`pacman -Qdt`), `.pacnew`/`.pacsave` count, foreign packages missing from the AUR, package cache size, journal size, time sync, battery health (`energy_full / energy_full_design`), forks left behind, number of `@priya-prev-*`, `priya doctor` layout checks, model provider health, security updates waiting.
- [ ] Score: 100 minus weighted penalties; any critical caps the score at 49; the formula is in the module docs and unit-tested.
- [ ] Maintenance actions (each a task or live action with its tier): clean package cache (keep 2), vacuum journal, remove orphans (sandbox), merge `.pacnew` with AI help (sandbox), collect leftover forks (helper GC), remove old previous systems beyond N while always keeping the newest (confirm), start a btrfs scrub (helper op, background, read-mostly).
- [ ] Weekly read-only health check (module timer, no changes) → notification only for warnings that are new since last week.
- [ ] Health app `ui/apps/Health` (Quickshell floating window) and a Health entry in the Settings sidebar search; top-bar dot when critical.
- [ ] "How healthy is my computer?" intent → Health summary card in the bar with a link to the page.
- [ ] Security advisories: match installed packages against the Arch security tracker (arch-audit data) daily; Health page lists affected packages with "Update safely" buttons

**UI**
- Health page: score ring, groups with status icons and plain sentences, Fix / Ask Priya per item, "Last checked", "Check now".
- States (11 §4): healthy, warnings, critical, plus checking, fix running, fix done (item updates in place), error (a check failed to run), offline, long content (25 items), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0. Never colour alone: every status has an icon and a word.
- Keyboard path: arrow keys through items, Enter runs Fix, ? asks Priya about the item. Screen-reader names: "Health score 82 of 100, 2 warnings", "Warning: package cache uses 4.2 GB. Fix: clean it".
- 👤 Mockup approval (`ui/mockups/6.10/`) before building; gallery sign-off and journey ("open Health → fix cache → score updates").

**Human in the loop:** nothing is cleaned automatically; scheduled maintenance is Phase 13 (AUTO-04). Removing previous systems is `confirm` tier and never removes the newest.

**Tests**
- Unit: each check on fixtures (ok / warn / critical), score formula, "new since last week" diff.
- Integration (fake machine): seeded unhealthy machine (full cache, orphan, `.pacnew`, leftover fork, 4 previous systems) → report lists all → each Fix → report green; newest `@priya-prev-*` survives (safety rule test).
- UI: page states × themes × scales; a11y lint; journey test.
- Perf: `priya health` < 4 s on the Zenbook (SMART and scrub-status included).
- Regression: `health_report_seeded`, `health_fix_all_green`, `health_never_deletes_newest_prev`, `health_page_states`.

**Done when**
- [ ] Seeded machine goes from critical to healthy using only Fix buttons.
- [ ] 👤 Owner reads the Health page on the Zenbook and understands every item without explanation.

---

### 6.11 — Dev environment setup
**Outcome:** "set up a dev environment for ~/projects/api" detects the stack, installs toolchains and services in the sandbox, writes `.priya.toml` `setup`/`run`/`check`, proves the check passes in the fork, and merges.
**Features:** ADM-08.
**Depends on:** 6.1.

**Tasks**
- [ ] `modules/devenv` (or a `sysadmin` sub-area): stack detector over project files — `Cargo.toml`/`rust-toolchain.toml`, `package.json` + lockfile (npm, pnpm, yarn, bun) + `.nvmrc`, `pyproject.toml` (uv, poetry, pip) + `.python-version`, `go.mod`, `Gemfile`, `pom.xml`/`build.gradle`, `docker-compose.yml`/`compose.yaml`, `.tool-versions`/`mise.toml`, `Makefile`.
- [ ] Plan builder: pacman packages first (`rustup`, `nodejs` + `corepack`, `python` + `uv`, `go`, `jdk-openjdk`), language toolchains through their official managers inside the project (`rustup toolchain install`, `corepack enable`), services (database via compose, or pacman + `initdb` in the fork), editor integration left to the user. Shown as a plan preview card (4.12).
- [ ] Docker: if needed, prefer rootless Podman; if the user asks for Docker, adding the user to the `docker` group is shown with a plain warning ("equal to admin rights") and needs `ask`.
- [ ] Write `.priya.toml` (`setup`, `run`, `check`) and run `setup` + `check` inside the fork; the review shows check output.
- [ ] Record working commands for project memory (consumed by Phase 8 8.4; stored as task outcome now).
- [ ] Fixture projects `tests/fixtures/devenv/` (Rust workspace, pnpm monorepo, uv Python API with Postgres in compose, Go service).

**UI**
- Plan preview card with detected stack chips, packages, services, and the `.priya.toml` preview; review shows check results.
- States: detecting, unknown stack (asks), plan ready, installing, check passed, check failed (agent tries once more), ready for review, error, offline (local package cache only), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard path through chips and the toml preview; screen-reader names on stack chips ("Detected: Rust, Postgres").
- 👤 Mockup approval (`ui/mockups/6.11/`) before building; gallery sign-off.

**Human in the loop:** the user edits the plan in place (e.g. "use Postgres from pacman, not Docker"); group membership changes are always highlighted in review.

**Tests**
- Unit: detector on every fixture project and on mixed repos (Rust + web frontend); plan builder output snapshots (`insta`).
- Integration (fake machine): each fixture project → sandbox setup → `check` passes in fork → merge → `check` passes live (home merge, system packages after fake reboot).
- Eval: 8 public sample repos (pinned commits, vendored as fixtures) → correct stack detection 8/8, check passes in ≥ 7/8.
- Regression: `devenv_rust_fixture`, `devenv_pnpm_fixture`, `devenv_uv_postgres_fixture`, `devenv_docker_group_warning`.

**Done when**
- [ ] All four fixture projects set up and pass their checks after merge.
- [ ] 👤 Owner sets up one real project on the Zenbook this way.

---

## Phase gate

### 6.12 — Phase gate
**Outcome:** the machine looks after itself on the owner's Zenbook: a real update goes through the whole pipeline, and diagnosis works on real problems.
**Features:** all Phase 6 features.
**Depends on:** 6.1–6.11.

**Tasks**
- [ ] Run the full regression suite and the nightly suites (e2e, perf, chaos, security, eval); fix failures.
- [ ] L4 checklist on the Zenbook (below); record results in this file.
- [ ] Update `features.md` statuses for ADM-03…ADM-13, the ADR for live actions, the `SystemProbe` port in [00-principles](../architecture/00-principles.md), and [03](../architecture/03-execution-and-sandbox.md) for diagnostics running on the live host read-only.
- [ ] Update `docs/human/05-the-phases.md` and the system-care section of `docs/human/04-everything-priyaos-will-do.md`.

**Done when**
- [ ] Whole regression suite green on `main`, plus nightly e2e (ALA update, restart, rollback), perf, chaos, security and eval suites.
- [ ] Owner demo: `priya update` with a seeded breakage in the VM → fix loop repairs it → review shows "Updates" and "Fixes Priya made" → restart → "everything works"; Super+Space → "why is my laptop slow?" with the runaway fixture → fixed and verified; Health page from critical to healthy.
- [ ] L4 on the Zenbook (the owner runs each step; Claude asks before anything restarts the machine): one real `priya update` → review → owner-chosen restart → post-restart check green → `priya rollback` → restart → previous kernel running → update again; four reversible hardware scenarios from 6.6; battery suggestions checked against a 10-minute measurement; `priya health` readable without explanation.
- [ ] Diagnosis eval ≥ 40 scenarios at the 6.2 thresholds; fix-loop eval at the 6.9 thresholds.
- [ ] Docs and feature statuses updated; `docs/human/05-the-phases.md` status updated.

## Out of scope

- Scheduled or automatic maintenance and monitors that act on their own (Phase 13: AUTO-03, AUTO-04).
- Rewind of manual changes and undo by prompt (Phase 7); live actions appear in the Rewind timeline from Phase 7.
- Full-kernel boot checks in a VM (the nspawn boot check does not test kernel, initramfs or bootloader; the "previous system" boot entry arrives in Phase 9).
- Firmware updates (fwupd), BIOS settings, and hardware repair advice beyond explanation.
- Network firewall and per-run egress control (Phase 14).
- Remote machines and fleet health (Phase 15).
