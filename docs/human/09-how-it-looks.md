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

## The building blocks and the showroom 🧱

Every Priya screen is made from one box of building blocks, called **PriyaUI**, like LEGO bricks that all click together the same way: buttons, a text box, the box you type questions into, cards, the answer card that fills in as Priya types, little labels ("chips"), pop-up notes ("toasts"), a "thinking" shimmer, and the "nothing here yet" and "something went wrong" pages. Each one comes in light and dark, works with just the keyboard, and tells a screen reader what it is.

The **gallery** is the showroom where every block sits on a shelf in every state: normal, hovered, pressed, switched off, busy, with Hindi text, and so on. To walk through it on your computer:

```sh
cargo xtask gallery
cargo xtask gallery --scale 2
```

Click a block on the left; the chips at the top switch light and dark, make the text twice as big, show Hindi samples or turn animations off.

A robot also photographs every shelf (46 states × light and dark × normal and double size = 184 photos) after every change and compares them with the approved photos. If a photo changes, the pull request shows you the old photo, the new one and a picture of the difference, and it can only go in once you add the **visual-change-approved** label.

## Talking to Priya from the terminal 💬

Before the pretty screens arrive, you can already talk to Priya by typing. Think of `priya` as a walkie-talkie to Priya's engine, which always runs quietly in the background:

| You type | What happens |
|---|---|
| `priya status` | "Are you awake, and what are you using?" |
| `priya doctor` | A health check-up, like a car's dashboard lights. `--fix` repairs what it safely can, after asking. |
| `priya settings get ui.theme` | Reads one setting. Add `--explain` to see which file it came from. |
| `priya settings set ui.theme light` | Changes a setting, the same as editing the settings file. |
| `priya events tail` | Watch the diary of what happens, live. |
| `priya audit verify` | Checks that nobody secretly changed the diary. |

Answers are short and plain. If the engine isn't running, `priya` tells you the exact command to start it.

## See it

A clickable prototype of the dark-theme desktop is saved in the repo at [ui/mockups/prototype/priyaos-live.html](../../ui/mockups/prototype/priyaos-live.html). Download it and open it in any browser. The [mockups README](../../ui/mockups/README.md) explains what else is there.

Back to: [PriyaOS explained simply](README.md)
