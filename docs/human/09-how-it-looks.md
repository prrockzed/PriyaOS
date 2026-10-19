# How PriyaOS looks and feels

PriyaOS should feel **calm, beautiful and obvious**, like something you'd want to use even if you'd never seen a terminal.

## The feeling we want

| We want | So we… |
|---|---|
| **One place to ask** | Super + Space does everything: questions, jobs, settings, opening apps, undo |
| **No surprises** | Before changing anything, Priya says in one sentence what it will do |
| **Always undoable** | Every result has an Undo button; Esc stops anything |
| **Simple first** | A plain summary on top; technical details only if you click "Details" |
| **Honest** | Shows the cost, which AI was used, and whether something can be undone |
| **Calm** | Background work never jumps in front of you; at most 3 pop-ups at a time |
| **Fast** | The Priya bar opens in under 0.1 seconds; apps and files appear while the AI is still thinking |

## Themes 🌗

| Theme | When |
|---|---|
| **Light** | Bright and clean, for daytime |
| **Dark** | Easy on the eyes at night |
| **True black** | For your OLED screen: real black saves battery and looks deep. Bars dim when idle so the screen doesn't get burn-in. |
| **High contrast** | Extra-clear text |
| **Auto** | Switches light ↔ dark at sunrise and sunset, or on a schedule, with a smooth fade |

**Everything follows the theme**: Priya's screens, the top bar, the terminal, normal apps, the mouse cursor, icons, and even the login and lock screens. One colour file controls them all, so changing the theme changes everything at once.

**Accent colour**: the default is **Marigold** (a warm golden orange). You can pick Indigo, Rose, Plum or Slate. There's no green or blue accent on purpose, because green always means "safe copy" and blue always means "just looking". Later, Priya can pick one from your wallpaper.

**Fonts**: Inter for text, JetBrains Mono for code, Noto Sans Devanagari for Hindi.

## The main screens

| Screen | What it is |
|---|---|
| **Priya bar** | Opens at the top centre with Super + Space. Type, talk or drop a picture. Buttons for **mode** (Look / Sandbox / Direct) and **model** (Auto). Answers appear below as tidy cards. |
| **Cards** | Answer · Command Priya ran · Plan · Approval · What changed · Undo · Files · Cost |
| **Task center** | All your jobs: *Needs you · Running · Ready for review · Done · Failed*, with live progress and a review screen |
| **Review** | A plain summary first ("Installed Docker and set it up for your api project"), then apps, services, files, each change viewable, with Keep / Discard |
| **Approval box** | "Restart nginx?" with why, exactly what, risk, whether it can be undone, and **Allow once / Always allow here / Deny** |
| **Top bar** | Workspaces, clock, Priya status (spinning ring while working, a dot when it needs you), Wi-Fi, sound, battery |
| **Settings** | Clear sections, a search box, and a "Tell Priya what to change" box at the top. Every change can be undone. |
| **Rewind** | A timeline you can slide through to see and undo changes |
| **Agent workspace** | Super + P shows all AI workers working, each in its own tile |

## Keyboard shortcuts ⌨️

| Keys | What it does |
|---|---|
| Super + Space | Priya bar |
| Super + J | Task center (all your jobs) |
| Super + P | Watch AI workers |
| Hold Super + V | Talk to Priya |
| Super + Shift + S | Ask about part of the screen |
| Super + Shift + Escape | Stop everything |

You can change any of them. If they clash with shortcuts you already use, Priya asks which to keep.

## You decide how much Priya asks 🎚️

| Setting | What happens |
|---|---|
| **Ask me first** | Priya shows the plan and asks before every change |
| **Ask when it matters** (default) | Asks only for big, risky, costly or outside-the-computer things |
| **Just do it** | Doesn't ask, but still works safely and **always** asks before anything that leaves your computer (emails, publishing, payments) |

You can set this **per feature**: for example "just do it" for note edits, "ask me first" for system updates.

While Priya works you can always:
- press **Esc** to stop,
- **type** to change direction ("use postgres instead"),
- change the mode or AI model in the plan before it starts.

## Easy for everyone ♿

- Everything works with only the keyboard, or only the mouse.
- Screen readers can read every button.
- Text can be made much bigger without breaking layouts.
- Animations can be turned down.
- Colour is never the only signal; there's always an icon and a word too.
- Priya's screens can be translated (Hindi comes in phase 10).

## How we make sure it looks good

1. Before building a screen, Claude shows you a **picture** of it in light and dark.
2. You approve or ask for changes.
3. Claude builds it from a shared set of building blocks, so everything matches.
4. Every screen is photographed in every theme; the robot on GitHub compares the photos after every change, so nothing gets uglier by accident.
5. You look at the finished screens in a **showroom** app (the gallery) and approve.

## See it

A clickable prototype of the dark-theme desktop is saved in the repo at [ui/mockups/prototype/priyaos-live.html](../../ui/mockups/prototype/priyaos-live.html). Download it and open it in any browser. The [mockups README](../../ui/mockups/README.md) explains what else is there.

Back to: [PriyaOS explained simply](README.md)
