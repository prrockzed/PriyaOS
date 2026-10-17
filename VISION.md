# PriyaOS — Vision

> **Your whole computer is version-controlled. Agents work on copies of it. You review and merge.**

*Draft, 18 October 2026. Brainstorm notes, not a spec.*

PriyaOS is an AI-native operating system built on Arch Linux. It does not touch the kernel or memory management. Everything new runs as ordinary user-space programs on top of a standard Arch install, and ships first as a package, later as an installable ISO.

The goal is not "Arch with an AI agent installed". That already exists. The goal is to change the basic building blocks of the computer so that agents are first-class, their work is reversible, and you stay in control.

---

## 1. Where the market is (October 2026)

| Player | Approach | Status |
|---|---|---|
| **Microsoft Windows** | Agents run in separate accounts and workspaces with audit logs; built-in registry for MCP tools; agent progress shown on the taskbar | Preview builds (from April 2026) |
| **Google Aluminium OS** | Android-based PC OS with Gemini expected to be central | Expected 2026 |
| **Omarchy** (Arch + Hyprland) | Comes with about 10 coding agents set up (Claude Code, Codex, OpenCode, …); agents can fix crashes and build themes | Shipping, popular |
| **Alibaba ANOLISA** | AI shell that takes plain English and bash; agent security (AgentSecCore), eBPF monitoring (AgentSight), system skills | Open source, v0.4 (May 2026), aimed at servers |
| **Harness** | Boots straight into coding agents, no desktop at all | Open source |
| **Tilo** | Shared desktop for people and agents, built on Fedora | YC-backed, demo "soon" |
| **MagenticOS** | Removes the desktop; the agent is the interface | Waitlist, alpha targeted for Q4 2026 |
| **Shadowfetch** | Agent does the task, then shows changes and test results for approval; "Firebreak" limits it to project files | Early |
| **Gnoppix, MakuluLinux, Deepin** | Local models, an assistant bar, plain-English search | Shipping, incremental |

### What this tells us

- **Everyone either installs agents onto the OS or puts them in a sandbox.** Nobody has changed what the computer's basic building blocks are. That is the opening.
- **Security and permissions alone are not a differentiator any more.** Windows gives each agent its own account and audit log. ANOLISA ships agent security and monitoring as open source. These are basic requirements now.
- **Omarchy is the closest competitor.** It uses the same base (Arch + Hyprland) and already preinstalls agents. Treat it as a source of users, not an enemy: PriyaOS should install on top of it.
- **The big companies will copy obvious features.** Our lasting edge has to be things they can't easily retrofit: open, neutral between agents, local-first, and the fork-and-merge model.

---

## 2. Core building blocks

These are what make PriyaOS AI-native rather than AI-installed.

### 2.1 Fork and merge the whole machine

Every agent task runs on a copy-on-write copy of your system (btrfs snapshots plus overlay mounts, inside systemd-nspawn or bubblewrap). The copy covers packages, `/etc`, dotfiles, services and your projects.

When the task finishes, you see a **diff of the machine**:

```
+ 3 packages installed      (docker, docker-compose, lazydocker)
~ 2 config files changed     (/etc/docker/daemon.json, ~/.config/hypr/hyprland.conf)
+ 1 service enabled          (docker.service)
~ 14 files edited            (~/projects/api/…)
```

Then you merge it or throw it away. It works like git, for the entire computer. A bad agent run costs you nothing.

### 2.2 Tasks are first-class OS objects, like processes

Each task has a goal, a budget (tokens, time, money), permissions, a log and a checker that confirms it actually worked. Tasks survive reboots.

```
priya ps        # running tasks, their budgets and status
priya kill 42   # stop a task
priya log 42    # what it did and why
```

The system monitor shows AI spending next to CPU and memory.

### 2.3 Every program becomes a tool when you install it

A pacman hook reads each newly installed program's help text or man page, and its D-Bus services where available, and registers it as an MCP tool. For graphical apps with no other interface, the accessibility tree (AT-SPI) is the fallback, not screenshots. Agents get more capable every time you install something.

### 2.4 Untrusted text can't trigger dangerous actions

Anything that came from the web, email or a downloaded repo is labeled untrusted. Text derived from untrusted content can never trigger a privileged action (sudo, network sends, reading credentials) without your approval. The labels are enforced by the daemon, not by asking the model to behave.

This is system-level protection against prompt injection, in the spirit of Google DeepMind's CaMeL work, and it is where the AgentKavach runtime-security work fits.

### 2.5 Bring your own agent

PriyaOS does not build its own coding agent. Claude Code, Codex, OpenCode and local Ollama models all run inside priyad's sandboxes under the same task, budget and permission model. PriyaOS stays neutral between them, which Microsoft and Google cannot do.

### 2.6 Local, inspectable memory

An opt-in log of what you worked on, indexed only on your machine. Microsoft Recall got backlash because it was opaque, so this one is open, encrypted, has per-app exclusions, and is stored as plain files you can read, edit and delete.

---

## 3. Ambitious ideas

- **Updates that can't break your system.** `priya update` runs `pacman -Syu` on a copy of the system, boots it in a container, checks your services and apps, fixes what broke, and only then offers to merge. Broken updates are a long-standing Arch pain point, so this is the first demo.
- **Teach by showing.** Do a task once while Priya watches, and it becomes a reusable, permission-limited skill.
- **The OS "dreams" overnight.** It consolidates memory, cleans junk, summarizes yesterday's work and presents a morning brief.
- **The desktop rewrites itself.** "Make my bar minimal and add a Pomodoro timer." Priya edits the Hyprland and Waybar configs on a copy, shows a live preview on a temporary workspace, and you merge.
- **Watch agents work.** Each agent gets a Hyprland special workspace you can peek into, plus a Waybar module showing live tasks and pending approvals.
- **Approve from your phone.** "Agent wants to `sudo systemctl restart nginx` — Allow / Deny."
- **Generated mini-apps.** "Give me a dashboard of my deploys." Priya generates a small app that stays as a saved view you can reopen.
- **Ask about anything on screen.** Using the accessibility tree, Priya can explain or act on whatever window you're looking at.

---

## 4. Architecture

Everything runs as normal programs on Arch. The kernel is unchanged.

```
┌───────────────────────────────────────────────────────────────┐
│ Hyprland + Priya shell                                        │
│   command bar · task panel · approvals · agent workspaces     │
├───────────────────────────────────────────────────────────────┤
│ priyad (Rust)                                                 │
│   task table · policy engine · untrusted-input labels · budgets│
│   ├─ fork manager   btrfs snapshots, overlay mounts,          │
│   │                 systemd-nspawn / bubblewrap,              │
│   │                 Landlock, seccomp, network namespaces     │
│   ├─ tool registry  MCP server, pacman hook,                  │
│   │                 D-Bus and AT-SPI adapters                 │
│   ├─ context store  SQLite + sqlite-vec, local only           │
│   └─ model router   Claude / OpenAI / Ollama                  │
├───────────────────────────────────────────────────────────────┤
│ Arch Linux (btrfs required) · systemd · Linux kernel          │
└───────────────────────────────────────────────────────────────┘
```

Design rule: **the model proposes, the policy engine decides.** The component that plans never enforces permissions.

---

## 5. Roadmap

1. **Weeks 1–6 — `priya` on the AUR.** The daemon and a CLI. Two demos:
   - `priya run "<task>"`: fork the machine, run Claude Code inside the sandbox, show the machine diff, merge or discard.
   - `priya update`: system upgrades that can't break your machine.
2. **Install anywhere on Arch with btrfs, including Omarchy.** Their users are exactly our audience.
3. **Make it feel native.** Auto-tool pacman hook, untrusted-input labeling, Hyprland task panel, phone approvals, task budgets.
4. **Memory and proactive features.** Local context store, overnight "dreaming", teach-by-showing.
5. **Only then, an archiso ISO** — once we know which parts of the OS really need to change.

---

## 6. Risks

- **Merging a running system is hard.** Live databases, running services, and files you edit while an agent works on the same copy all need conflict handling. Start with packages, `/etc` and project folders; leave `/var` for later.
- **Maintaining a distro is a long-term burden** (hardware support, Arch's rolling updates). That's why it ships as a package first.
- **Big companies will copy the obvious features.** Stay focused on what they can't retrofit: openness, agent neutrality, local-first, fork and merge.
- **Token costs.** Budgets per task and a router that prefers local models for routine work.
- **Privacy.** The memory log must stay opt-in, local and inspectable, or it repeats Recall's mistakes.

---

## 7. The name

"Priya" means "beloved" in Sanskrit, which suits a personal OS. Use **Priya** as the name and personality of the system, and keep an acronym as an Easter egg rather than the tagline. The best candidate from earlier brainstorming:

- **P**ersonal **R**easoning **I**ntelligence for **Y**our **A**utomation

---

## Sources

- [VentureBeat: Microsoft remakes Windows for autonomous AI agents](https://venturebeat.com/ai/microsoft-remakes-windows-for-an-era-of-autonomous-ai-agents)
- [Enterprise DNA: Windows 11 agentic taskbar and MCP](https://enterprisedna.co/resources/news/microsoft-windows-11-agentic-taskbar-mcp-2026)
- [Alibaba Cloud Linux 4 Agentic Edition (ANOLISA) overview](https://help.aliyun.com/en/alinux/agentic-os)
- [ANOLISA release notes](https://help.aliyun.com/en/alinux/releasenotes)
- [tilo.so](https://tilo.so)
- [Harness OS](https://harness.autonomous.ai/os)
- [MagenticOS](https://magenticos.com)
- [It's FOSS: AI-focused Linux distros (Oct 2026)](https://itsfoss.com/ai-focused-linux-distros/)
- [Omarchy manual: AI](https://learn.omacom.io/2/the-omarchy-manual/107/ai.md)
- [Android Authority: Aluminium OS](https://androidauthority.com/aluminium-os-android-for-pcs-3619092)
- [SUSE: The shift to an agentic OS](https://www.suse.com/blog/the-shift-to-an-agentic-os-suses-approach-to-linux-in-the-age-of-ai/)
- [AIOS (agiresearch)](https://github.com/agiresearch/AIOS)
