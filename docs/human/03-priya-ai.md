# Priya AI

Priya AI is PriyaOS's own AI. We don't train a new model. Priya AI is a **smart dispatcher** that uses all the AI models you've turned on and sends each job to the right one.

## A story

Imagine a hospital. 🏥

- A small cut goes to the nurse. Quick and cheap.
- A broken bone goes to a regular doctor.
- Brain surgery goes to the top specialist.

You wouldn't send a small cut to the brain surgeon. It's slow and expensive. Priya AI is the **front desk** that decides who sees which patient.

## Example: only Claude is turned on

Claude comes in different sizes, like small, medium and large (for example Haiku, Sonnet, Opus and Fable, in different versions).

| You ask | Priya AI sends it to | Why |
|---|---|---|
| "What time is it in Tokyo?" | the small model (Haiku) | quick and cheap |
| "Change this function to handle errors" | the medium model (Sonnet) | great at editing code |
| "Analyse why my app is slow and plan a fix" | the large model (Opus) | best at deep thinking |

## Example: several AI companies turned on

If you have Claude, ChatGPT/Codex, Gemini, Grok and a free model on your laptop, Priya AI picks across **all** of them. For example: code edits to whichever does them best, a picture question to the best image model, private documents to the one on your laptop.

## How it decides

For every job, Priya AI asks itself:
1. **What kind of job is this?** A question, a code change, an analysis, a picture?
2. **How hard is it?**
3. **Is it private?** Then keep it on the laptop if you've said so.
4. **Which models can do it?** And how good, how fast and how expensive is each one?
5. **What does it already know from your laptop?** Which model actually did well on this kind of job before?

Then it picks the best balance of **quality, cost and speed**.

You can always ask **"why did you use that model?"** and Priya answers in plain words, for example: *"I used Sonnet because this was a code edit. Haiku got 3 of the last 10 similar jobs wrong. Opus would have cost 5 times more."*

## It gets smarter on your laptop 📈

Every finished job teaches Priya AI something:
- Did the tests pass?
- Did you keep the result or throw it away?
- Did you undo it later?
- Did you give it a 👍 or 👎?

Over time it learns which model is really best for **your** work. This learning stays on your laptop.

## Saving tokens (saving money) 💰

AI companies charge by the "token" (roughly, pieces of words). Priya AI saves tokens in several ways:

| Trick | Simple meaning |
|---|---|
| **Try cheap first** | Use a small model first. Only if it fails the check, move to a bigger one. |
| **Reuse** | If you ask the same thing again, reuse the earlier answer. |
| **Remember the start** | AI companies give a discount when a message starts the same way as before. Priya arranges messages so this happens often. |
| **Shorten long text** | Long error logs are trimmed to the important lines before being sent. |
| **Fetch, don't stuff** | Only send the files and memories that are needed, not everything. |
| **Short answers** | Small jobs get short answers. |
| **Overnight discount** | Jobs that can wait (like tidying memory) use the cheaper "batch" option. |

`priya ai savings` shows how much was saved.

## You're in control 🎛️

Just say it, or change it in settings:
- "Use only Claude today"
- "Use Opus for this one"
- "Cheapest mode"
- "Keep anything from my Documents folder on local models"
- "Add my LM Studio at 192.168.1.20"
- "Turn on Grok"

**Modes you can pick:**

| Mode | What it does |
|---|---|
| **Best** | Best quality within your budget |
| **Balanced** (default) | Good quality, sensible cost |
| **Cheapest** | Lowest cost that's still good enough |
| **Local only** | Never sends anything to the internet |
| **One company** | e.g. only Claude, but still picks the best Claude model |
| **One model** | Always the same model |

## Adding a new model is easy ➕

- **A new model from a company you already use** (e.g. a new Claude version): Priya notices it automatically and tells you.
- **An AI on your own laptop or another computer** (Ollama, LM Studio and similar): one line in settings, or just tell Priya the address.
- **A brand-new AI company**: we add one small "adapter" (a translator) once, and then it works like the others.

## Your AI workers can use any model too 🔌

AI workers like Claude Code, Codex and OpenCode usually talk only to their own company's models. Priya AI can stand in the middle like a **universal plug adapter**, so a worker can use whichever model Priya picks, including one on your laptop. (Some combinations work less well than a worker's "home" model. Priya tracks that and learns.)

## Your keys are safe 🔑

Your AI account keys are stored in the laptop's encrypted key store. They're never written in settings files, never shown to AI workers unless needed, and never written in logs.

Next: [Everything PriyaOS will do](04-everything-priyaos-will-do.md)
