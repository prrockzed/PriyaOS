# What is PriyaOS?

## In one sentence

PriyaOS is a computer system where you **talk to your computer** — by typing, speaking or showing it something — and it **does the work for you**, safely, using the best AI for each job.

## A story to explain it

Imagine you have a very smart assistant living in your laptop. Her name is Priya.

- You say: *"Set up my project so it runs."* Priya does it.
- You say: *"Why is my laptop slow?"* Priya looks and tells you in simple words.
- You show her a screenshot of an error. She explains it and offers to fix it.
- You say: *"Undo whatever broke my Wi-Fi yesterday."* She finds it and undoes just that.

Priya isn't one AI. She's more like a **team manager**. She knows many AI helpers (Claude, ChatGPT/Codex, Gemini, Grok, and AIs running on your own laptop), and she sends each job to the helper who's best at it, while keeping the cost low.

## The five big ideas

### 1. Priya AI — one AI that uses all AIs 🧠
You turn on the AI services you have (for example Claude, or Claude + Gemini, or a free AI on your laptop). Priya AI picks the right one for every job:
- small, quick questions go to a small, cheap model,
- hard thinking goes to the strongest model,
- private things stay on your laptop.

It also learns over time which model does best on *your* work. → [More about Priya AI](03-priya-ai.md)

### 2. Priya Sandbox — a safe practice copy 📄
Before doing something risky, Priya can make an **instant photocopy of your whole computer**. The AI works on the photocopy. When it's done, you see a simple list of what changed and decide: **keep** or **throw away**. Your real computer is only changed when you say yes.

You choose when to use it. You can say "do it directly" for small things, or set rules like "always use the sandbox for updates".

### 3. Priya Orchestration — a team of AI workers 👷‍♀️👷
For big jobs ("build login for my app, test it, write the docs"), Priya splits the work into steps, gives each step to the best AI worker, lets them work at the same time, checks their work with a *different* AI, and brings it all back for you to approve.

### 4. Priya Rewind — undo anything ⏪
Priya remembers every change on your computer, whether an AI made it or you did. You can undo one specific change, bring back an old version of a file, or just say what went wrong: *"undo the thing that broke my sound"*.

### 5. Priya Vision and voice — show and tell 👀🎙️
You can talk to Priya ("Hey Priya…") and she can look at your screen or a picture when you ask: *"What does this error mean?"*

## What it's built on

We don't build a computer system from zero. We start with **Arch Linux** (the Linux you already use) and add Priya on top: her own programs, settings, look, and installer. When you turn on your Zenbook, you'll pick **PriyaOS** or **Windows**. Windows stays exactly as it is.

> 🏠 Arch Linux is the house. PriyaOS is the smart furniture, the lights, the security system, and Priya, the helper who lives there.

## Why this is new

| Today | PriyaOS |
|---|---|
| You install one AI app; it's stuck with one company's models | One AI that uses every model you have and picks the best each time |
| The AI works directly on your real computer | You choose: practice copy (sandbox), or directly, per task or per feature |
| If the AI breaks something, you fix it | You throw the copy away, or Rewind undoes exactly that change |
| You manage each AI tool yourself | Priya plans the work and manages a whole team of AI workers |
| You type commands | You type, talk, or show a picture, and get plain answers |

## How we build it

In **17 phases**, one after another. Each phase is planned, built, tested, and only then do we move on. → [The phases](05-the-phases.md)

Next: [How it works](02-how-it-works.md)
