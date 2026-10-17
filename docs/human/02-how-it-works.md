# How PriyaOS works

This page explains each piece with an everyday example. You don't need to know any code.

## The building: Arch Linux

We didn't build a computer system from zero. That would take years. We start with **Arch Linux** (the Linux you already use on your Zenbook) and add our own parts on top.

> 🏠 Arch Linux is the house. PriyaOS is the furniture, the smart lights and the helper who lives there.

## The magic floor: btrfs

Your computer stores files on its disk. PriyaOS uses a special way of storing them called **btrfs** ("butter-F-S").

btrfs has a superpower: it can take a **snapshot**, an instant photo of every file, in under a second, without using extra space. Space is only used later, for the things that change.

> 📸 Like taking a photo of your room. The photo doesn't need a second room; it only remembers what the room looked like.

PriyaOS keeps your computer in a few labelled boxes on this disk:

| Box name | What's inside |
|---|---|
| `@` | The system: apps, settings, the core of the computer |
| `@home` | Your stuff: projects, documents, downloads |
| `@priya` | Priya's own notebook (which tasks ran, what changed) |
| `@priya-forks` | All the photocopies (forks) the AI is working on |

## The helper: priyad

**priyad** ("priya-dee") is a small program that is always running in the background. It's the manager.

> 👩‍💼 Think of a careful office manager. You give her a job. She makes a photocopy, hands it to the worker (the AI), watches the clock and the money, and brings you the result to sign.

priyad:
- makes the forks,
- starts the AI inside the fork,
- stops the AI if it takes too long or spends too much,
- builds the "what changed" list,
- and, only when you say yes, copies the changes into your real computer.

## The worker: Claude (or Codex)

The AI does the actual work: installing apps, writing code, fixing settings.

Inside the fork it is allowed to do **anything**, because nothing it does touches your real computer. That's why it can work fast without asking "may I?" every minute.

> 🧑‍🔧 Like letting a mechanic work on a perfect copy of your car. Whatever they do, your real car is still safe in the garage.

## The secret drawer

Some things the AI should never see, like your SSH keys, passwords, or browser logins. Priya **hides** these from every fork. Inside the copy, those folders look empty.

> 🔒 The photocopy has your secret diary pages blanked out.

## The "stranger danger" sticker

If the AI read something from the internet while working (a web page, a downloaded file), it might have read tricky instructions written by a stranger. Priya puts a red **UNTRUSTED** sticker on that task. Before you keep its changes, Priya asks you to look extra carefully and type `merge` to confirm.

> 🚸 Like a parent checking a note that came from someone they don't know.

## Keeping changes (merge)

There are two kinds of changes:

1. **Your files** (projects, documents). These are copied over **right away**.
   - If you changed the same file yourself while the AI was working, Priya doesn't overwrite your work. It shows you both versions and asks which one to keep. This is called a **conflict**.
2. **The system** (apps, settings). These switch over **the next time you restart**. Priya swaps the old system box for the new one, like swapping a light bulb, and keeps the old one in a drawer.

## Going back (rollback)

Priya keeps your last 3 old systems. If something feels wrong after a restart, type:

```
priya rollback
```

and restart. You're back to how it was before. 🔙

## Safe updates

Normally, updating Linux can sometimes break things. With Priya:

1. Priya makes a fork and updates **the fork**.
2. Priya **starts the fork up**, like a test drive, and checks whether everything works.
3. If something broke, the AI fixes it, still on the fork.
4. Priya shows you the result. You keep it, and it switches over on restart.

> 🚗 A test drive before you buy.

## Every app becomes a tool

When you install a new app, Priya reads its instructions (its "help" page) and writes it into a **phone book of tools**. Later, the AI can look in that phone book: "Is there a tool on this computer that can read JSON files?" Yes, `jq`.

> 📒 Every new app adds its name and job to the phone book automatically.

## Priya remembers

Every finished task is written into Priya's notebook. You can ask:

```
priya history "docker"
```

and see what was done before, when, and what changed. The AI can read the notebook too, so it learns how things are done on *your* computer.

## Putting it together

```
 You: "Set up Docker"  (Super + Space)
        │
        ▼
 priyad makes a photocopy (fork) of your computer  ← takes 1 second
        │
        ▼
 Claude works on the copy (secrets hidden, clock running)
        │
        ▼
 Priya runs a test and makes the "what changed" list
        │
        ▼
 🔔 "Ready for review"  →  you press m (keep) or d (throw away)
        │
        ▼
 Your files update now; system changes apply on next restart
```

Next: [The 7-day plan](03-the-7-day-plan.md)
