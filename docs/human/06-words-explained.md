# Words explained

Short meanings for the words you'll see in this project.

| Word | Simple meaning | Example |
|---|---|---|
| **Operating system (OS)** | The main program that runs your computer and everything on it | Windows, macOS, Arch Linux, PriyaOS |
| **Arch Linux** | The Linux that PriyaOS is built on top of | The house we're furnishing |
| **Kernel** | The deepest part of the OS that talks to the hardware. We don't change it. | The engine of a car |
| **btrfs** | A way of storing files that can take instant snapshots | A notebook that can photocopy itself in a second |
| **Subvolume** | A labelled box on a btrfs disk (`@`, `@home`, …) | Different drawers in the same cupboard |
| **Snapshot** | An instant, space-saving photo of a box | A photo of your room |
| **Fork** | A writable copy of your whole computer that the AI works on | A photocopy of your drawing that a friend can draw on |
| **Diff** | The list of what changed between before and after | "You added 2 apps and edited 5 files" |
| **Merge** | Keeping the AI's changes on your real computer | Gluing the good parts of the photocopy back |
| **Discard** | Throwing the fork away | Binning the photocopy |
| **Conflict** | You and the AI both changed the same file; Priya asks which to keep | Two people edited the same page |
| **Rollback** | Going back to the system you had before the last system change | An "undo" button for the whole computer |
| **priyad** | Priya's background manager program | The office manager |
| **priya** | The command you type (`priya run`, `priya review` …) | Talking to the office manager |
| **Agent** | An AI that can *do* things, not just chat | Claude Code, Codex |
| **Task** | One job you gave Priya | "Set up Docker" |
| **Budget** | The time and money limit for one task | "Stop after 30 minutes or $2" |
| **Taint / Untrusted** | The task read something from the internet, so check it carefully | A note from a stranger |
| **Masked / hidden** | Secret folders the AI can't see | Blanked-out diary pages |
| **Container (nspawn)** | A closed room where a program runs, seeing only the fork | A practice room with a copy of your stuff |
| **Daemon / service** | A program that runs in the background all the time | The fridge humming in the kitchen |
| **Hyprland** | The program that draws windows on your screen | The walls and windows of the house |
| **Waybar** | The bar at the top of the screen; Priya adds an icon there | The noticeboard |
| **GRUB** | The menu at startup where you pick PriyaOS or Windows | The front door with two doorbells |
| **EFI partition** | A small shared area that both Windows and PriyaOS use to start up | The building's shared lobby |
| **BIOS / UEFI setup** | The laptop's settings screen before any OS starts (F2 on ASUS) | The building's fuse box |
| **Secure Boot** | A setting that only allows signed startup programs | A guard at the door |
| **BitLocker** | Windows' disk lock; needs a recovery key if startup settings change | A safe's backup combination |
| **ISO** | A file that holds the whole installer; you put it on a USB | A box with a flat-pack house inside |
| **Package** | An app ready to install with `pacman` | An item in an app store |
| **pacman** | Arch's app installer | The app store's checkout counter |
| **MCP** | A standard way for AI agents to use tools | A universal plug for tools |
| **Virtual machine (VM)** | A whole pretend computer running in a window | A flight simulator |
| **Rust** | The programming language we write Priya in | The language the furniture's instructions are written in |
