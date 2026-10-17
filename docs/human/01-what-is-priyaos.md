# What is PriyaOS?

## The short answer

PriyaOS is a computer system where an AI helper can do real work for you, **and can never break your computer**.

## A story to explain it

Imagine you have a drawing you love. 🖍️

You ask your friend to "make it better". Your friend could draw straight on your drawing. If they mess up, your drawing is ruined.

Now imagine instead:

1. You make a **photocopy** of your drawing.
2. Your friend draws on the **photocopy**.
3. They show you the photocopy and say "here is what I changed".
4. If you like it, you **keep** the changes. If you don't, you **throw the photocopy away**. Your real drawing was never touched.

PriyaOS does exactly this with your whole computer:

- The **drawing** is your computer: your files, your apps, your settings.
- The **photocopy** is called a **fork**. Making one takes less than a second and uses almost no space.
- The **friend** is an AI helper like Claude.
- **"Here is what I changed"** is the **review**.
- **Keeping the changes** is called **merging**.

## What it looks like

You press **Super + Space** and type:

> "Set up Docker for my api project."

Priya makes a photocopy of your computer and gives it to Claude. Claude installs Docker, writes the files, and tests them, all on the copy. A few minutes later you get a notification:

> ✅ Ready for review: set up Docker for my api project

You open it and see a simple list:

```
New apps:        docker, docker-compose
Turned on:       docker service
Changed files:   5 files in projects/api
Test:            passed ✔
```

You press **m** to keep it, or **d** to throw it away. That's it.

## Why this is new

| Today (normal computers) | PriyaOS |
|---|---|
| You install an AI app. It works directly on your real computer. | The AI only ever works on a copy. |
| If the AI makes a mistake, you have to fix it yourself. | If the AI makes a mistake, you throw the copy away. |
| You have to watch the AI and click "allow" again and again. | The AI can work freely, because the copy is safe. You check once at the end. |
| System updates can break your computer. | Priya tests updates on a copy first, fixes problems, and only then asks you. |

Big companies (Microsoft, Google) and other projects are adding AI to computers too. But they all let the AI work on the real thing, inside a fenced area. **Nobody else gives the AI a full copy of your computer and lets you choose what to keep.** That is our special idea.

## The two things we finish this week

1. **"Do this for me"** — you ask, the AI does it on a copy, you review and keep it.
2. **"Update my computer safely"** — Priya updates a copy, checks that it still starts up properly, fixes anything broken, and only then asks you. If you still don't like it after restarting, one command takes you back.

Everything else from [VISION.md](../../VISION.md) (like approving things from your phone) comes after this week.

## Where it runs

On your **ASUS Zenbook 14 OLED**, next to Windows. When you turn on the laptop, you choose: **PriyaOS** or **Windows**. Windows stays exactly as it is.

Next: [How it works](02-how-it-works.md)
