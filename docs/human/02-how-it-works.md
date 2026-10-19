# How PriyaOS works

Each piece explained with an everyday example. No code knowledge needed.

## The big picture

```
   You  (type · talk · show a picture)
    │
    ▼
 Priya bar  ──▶  Priya AI understands what you want
                    │
          ┌─────────┴──────────┐
          ▼                    ▼
   Just a question?        A job to do?
   → plain answer          → Priya Orchestration makes a plan
                             → picks AI workers and models
                             → works in the sandbox or directly
                             → checks the result
                             → shows you "what changed"
                                    │
                                    ▼
                         You: keep ✅  or  throw away ❌
                                    │
                                    ▼
                    Priya Rewind remembers it forever (undo any time)
```

## The parts, one by one

### The building: Arch Linux 🏠
PriyaOS sits on top of Arch Linux. We don't change Linux itself. We add our own programs as normal Arch packages and make our own installer USB.

### The office manager: priyad 👩‍💼
A program that's always running quietly in the background. It keeps the list of jobs, remembers everything that happened, and talks to the AI helpers.

> Like an office manager who keeps a diary of every job, every decision and every receipt.

### The locked key cabinet: the helper 🔐
Some jobs need full control of the computer (like making a photocopy of the whole system). Only one tiny program, the **helper**, is allowed to do those. It has **no internet** and **no AI** inside it. It only does a short list of safe jobs.

> Like a bank vault guard who only opens the vault for a short list of approved reasons, and never talks to strangers.

Why: even if an AI gets confused, it can't take over your computer.

### The brain: Priya AI 🧠
Picks the best AI model for every job and keeps costs low. Explained in detail in [Priya AI](03-priya-ai.md).

### The team leader: Priya Orchestration 🧑‍🏫
For big jobs, Priya:
1. **Plans**: splits the job into steps ("backend", "frontend", "tests", "docs").
2. **Assigns**: gives each step to the best worker (Claude Code, Codex, Gemini CLI, Grok, or Priya's own worker that can use any model, even one on your laptop).
3. **Runs them at the same time** when they don't depend on each other.
4. **Watches**: if a worker gets stuck, nudges it, or swaps in a different worker.
5. **Checks**: a worker from a *different* AI company reviews the work, like a second teacher grading the exam.
6. **Fixes clashes**: if two workers changed the same file, it combines them or asks you.

> Like a school project leader who splits the work, gives each part to the best student, checks everything, and hands in one finished project.

### The practice copy: Priya Sandbox 📄
Priya can make an **instant photocopy of your whole computer**. It takes about a second and uses almost no space. The AI works on the copy, then you see:

```
New apps:        docker, docker-compose
Turned on:       docker service
Changed files:   5 files in projects/api
Test:            passed ✔
```

You press **keep** or **throw away**.

### You choose how careful to be 🎚️
Every job can run in one of four ways:

| Way | What it means | Example |
|---|---|---|
| **Just look** | AI can only read, can't change anything | "Why is my laptop slow?" |
| **Sandbox** | AI works on the photocopy; you decide what to keep | Installing apps, updates, big code changes |
| **Directly** | AI works on the real computer, but Priya takes a quick snapshot first so you can still undo | Fixing a typo in your notes |
| **Directly, no snapshot** | Fastest, but undo is limited. Only when you clearly ask for it. | Rarely |

You decide per job ("do it directly"), or set rules per feature ("updates always in the sandbox, note edits directly"). If you don't say anything, Priya picks a safe choice based on how risky the job looks.

One honest limit: things that **leave your computer** (sending an email, publishing a website, pushing code to GitHub) can't be undone by any copy. Priya always asks before those.

### The safety rules 🛡️
- **Secret drawer**: your passwords, SSH keys and browser logins are hidden from AI workers.
- **Stranger-danger sticker**: if the AI read something from the internet, the job gets an **UNTRUSTED** label and Priya asks you to look more carefully before keeping it.
- **Permission rules**: clear rules about what each worker may touch ("can read my projects, can never deploy"). Later you can write these rules in plain words.
- **Spending limits**: every job has a time and money limit.
- **Big red button**: **Super + Shift + Escape** stops every AI worker instantly.
- **Diary that can't be secretly edited**: every action is written in a diary where each page is "sealed" to the one before it. If anything gets changed, Priya notices.

### The undo machine: Priya Rewind ⏪
Priya keeps a history of every change:
- changes AI workers made,
- changes to settings,
- changes **you** made yourself (Priya takes a quick automatic photo of the system every hour and around every app install).

So you can:
- **undo one change** and keep everything after it,
- **bring back a file** as it was yesterday at 6pm,
- **take the whole system back** to an earlier point,
- or just **say it**: *"undo whatever broke my Wi-Fi on Tuesday"*. Priya finds the likely changes, explains them, and undoes the one you pick, on a copy first.

> Like a video recording of your room. You can rewind to see when the vase broke, and put back just the vase.

### The memory 📒
- **Task diary**: what was done, when, by which AI, what changed.
- **Project memory**: how each of your projects works ("uses pnpm", "tests run with `make test`").
- **Your preferences**: "answer briefly", "I like dark themes".
- **File finder**: "find the PDF where I compared bike prices". This only works in folders you allow.

You can see, edit or delete any memory at any time. It all stays on your laptop.

### The phone book of tools 📞
Every app you install is automatically added to a phone book that AI workers can search: "Is there a tool here that can read JSON?" → yes, `jq`.

### Talking and showing 🎙️👀
- Hold **Super + V** and talk, or say **"Hey Priya"** (if you turn it on). Your voice is understood on your laptop by default, not sent to the internet.
- Press **Super + Shift + S**, select part of the screen, and ask about it.
- Priya never looks at your screen unless you ask, and never looks at password or banking windows.

### Built from blocks (modules) 🧩
Every feature (voice, updates, Rewind, memory…) is a separate **block** that plugs into Priya's core. To add a new feature later, we add a new block without rebuilding everything. That's how PriyaOS can keep growing easily.

> Like LEGO: the base plate stays, and you keep adding new pieces.

Next: [Priya AI](03-priya-ai.md)
