# Everything PriyaOS will do

Every feature we've planned, in simple words, with the phase that builds it and where the idea came from.

**Idea from:** 🙋 you · 💬 ChatGPT conversation · 🤖 Claude · 🔎 research on other products, or found while reviewing the plans

The technical version, with feature IDs, is [docs/claude/features.md](../claude/features.md). New ideas get added there first, then here.

## 🧱 The foundation

| Feature | Phase | Idea from |
|---|---|---|
| A diary of everything that happens, sealed so it can't be secretly edited | 0 | 🤖 |
| Built from blocks (modules), so new features plug in easily | 0 | 🤖 |
| Settings in layers: system, you, each project, each job | 0 | 🤖 |
| Everything you can say, you can also type as a command | 0 | 🤖 |
| A health check (`priya doctor`) | 0 | 🤖 |
| Only one tiny program has full control, with no internet and no AI | 2 | 🤖 |
| Jobs survive restarts | 3 | 💬 |
| Change any setting by just saying it | 5 | 🙋 |

## 📄 Priya Sandbox and ways of working

| Feature | Phase | Idea from |
|---|---|---|
| Instant photocopy of the whole computer for the AI to work on | 2 | 🤖 |
| One list of everything that changed (apps, services, files, settings) | 2 | 🤖 |
| Keep or throw away | 2 | 🤖 |
| If you and the AI changed the same file, Priya asks instead of overwriting | 2 | 🤖 |
| System changes switch over on restart; one command goes back (kernel included) | 2 | 🤖 |
| Four ways of working: just look / sandbox / directly with snapshot / directly | 2 | 🙋 |
| Choose the way per job and per feature, with rules | 3 | 🙋 |
| Directly-done changes can still be undone | 2 | 🙋 🤖 |
| See changes before they happen | 2 | 💬 |
| Always asks before anything that leaves your computer (send, publish, push) | 3 | 🤖 |
| AI workers don't need to ask "may I?" inside the sandbox | 4 | 🤖 |
| An extra-strong locked box for untrusted work | 14 | 🔎 |
| If a new system won't start, pick the previous one from the start menu | 2 | 🤖 |
| Open a terminal inside a safe copy to try things by hand | 2 | 🔎 |
| If an update fails to start twice, the previous system starts by itself | 9 | 🔎 |

## 🧠 Priya AI

| Feature | Phase | Idea from |
|---|---|---|
| Works with Claude, ChatGPT/Codex, Gemini, Grok, OpenRouter, and AIs on your laptop | 1 | 🙋 |
| Notices new models automatically | 1 | 🤖 🔎 |
| Add a model with one line of settings | 1 | 🙋 |
| Switch model, company or mode by saying it | 1 | 🙋 |
| Picks the right size within one company (small / medium / large) | 1 | 🙋 |
| Picks the best company for each job | 1 | 🙋 |
| Jobs ask for a "kind" of model (fast, smart, coding, vision, private), not a name | 1 | 🤖 |
| Modes: best, balanced, cheapest, local only, one company, one model | 1 | 🙋 🤖 |
| Local AIs work offline | 1 | 🙋 💬 |
| Decide what's allowed to leave your laptop | 1 | 💬 |
| Saves tokens (reuse, trimming, cache discounts, short answers) | 1 | 🙋 |
| Reuses answers to repeated questions | 1 | 💬 |
| Tries cheap first, checks, and only then uses a bigger model | 1 | 🤖 🔎 |
| Learns which model is best on your laptop | 1 | 🙋 🤖 |
| Explains why it picked a model | 1 | 🤖 |
| Tracks cost with daily and monthly limits | 1 | 💬 |
| Keeps your AI keys safe | 1 | 💬 |
| Lets any AI worker use any model (universal plug) | 1 | 🤖 🔎 |
| Shows how much money it saved | 1 | 🤖 |
| Costs shown in rupees (or your own currency) | 1 | 🔎 |
| Warns when a model is being retired and moves your choices to the new one | 1 | 🔎 |
| Uses cheaper overnight processing for jobs that can wait | 13 | 🤖 🔎 |
| A basic version that answers questions from day 1 | 0 | 🤖 |

## 👷 Priya Orchestration (AI workers)

| Feature | Phase | Idea from |
|---|---|---|
| Works with Claude Code, Codex, Gemini CLI, OpenCode | 4 | 💬 🙋 |
| Priya's own worker that can use any model, even a local one | 4 | 🤖 |
| Knows which workers are installed and logged in | 4 | 🤖 |
| Picks the best worker and model for each step | 4 | 🙋 |
| Live progress: waiting, working, stuck, failed, done | 4 | 💬 |
| Pause, continue, stop, restart a worker | 4 | 💬 |
| Give a worker new instructions while it's working | 4 | 🤖 |
| Workers can run in the background | 4 | 💬 |
| All workers report in the same way | 4 | 🔎 |
| Each project can say how to set up, run and test it | 4 | 🔎 |
| Automatic tests and checks after changes | 4 | 💬 |
| Comment on a change and the worker fixes it | 5 | 🔎 |
| Watch workers in their own desktop space | 5 | 🤖 |
| Big jobs split into a plan you can approve or change by talking | 12 | 🤖 |
| Several workers at the same time, each in its own copy | 12 | 💬 |
| Workers can hand out smaller jobs, never with more permissions than they have | 12 | 💬 |
| Workers share notes and hand work to each other | 12 | 💬 🔎 |
| Combines changes when two workers touched the same thing | 12 | 💬 |
| A worker from a different AI company checks the work | 12 | 💬 🤖 |
| Shares CPU, memory and battery fairly | 12 | 💬 |
| Notices a stuck worker and tries a different model or worker | 12 | 🤖 |
| Continues after a crash or restart | 12 | 💬 |
| More workers: Grok, Aider, Goose, Cursor, Copilot | 12 | 🙋 🔎 |
| Scoreboards: which worker and model does best | 12 | 🤖 |
| Install and log in to AI workers from Settings | 4 | 🔎 |
| Workers on other computers; moving jobs between computers | 15 | 💬 |
| Talk to outside AI agents through a common standard (A2A) | 15 | 🔎 |

## 💻 Building software

| Feature | Phase | Idea from |
|---|---|---|
| Runs tests, checks and builds automatically | 4 | 💬 |
| Understands how a project is put together | 12 | 💬 |
| A separate branch per worker | 12 | 💬 |
| Workers review each other's code | 12 | 💬 |
| Opens pull requests with summaries | 12 | 💬 |
| Asks before merging important changes | 12 | 💬 |
| Watches automated tests online and explains failures | 12 | 💬 |
| Publishes your app (deploy) after you approve, watches it, rolls back if needed | 12 | 💬 |

## 🛠️ Looking after the computer

| Feature | Phase | Idea from |
|---|---|---|
| Install and set up apps by asking | 4 | 💬 |
| A terminal where you can type plain words or normal commands | 5 | 💬 🔎 |
| Safe updates: test on a copy, start it up, fix problems, then apply | 6 | 🤖 |
| Finds why it's slow (CPU, memory, disk, network) | 6 | 💬 |
| Explains why a service or app crashed | 6 | 💬 |
| Fixes broken app installs | 6 | 💬 |
| Explains error messages and logs | 6 | 💬 |
| Sets up programming tools for a project | 6 | 💬 |
| Helps with Wi-Fi, Bluetooth, screens, sound, devices | 6 | 💬 |
| Battery and speed tips | 6 | 💬 |
| Explains driver and update problems | 6 | 💬 |
| Health checks and clean-ups | 6 | 💬 |
| Checks that a fix really worked | 6 | 💬 |
| Updates that change the core of the system are test-started in a virtual computer first | 6 | 🔎 |
| Warns about known security problems in installed apps | 6 | 🔎 |

## 🛡️ Safety

| Feature | Phase | Idea from |
|---|---|---|
| The tiny full-control program follows strict rules | 2 | 💬 |
| Every AI worker has a name tag for rules and the diary | 3 | 💬 |
| Clear permission rules for what each worker may touch | 3 | 💬 |
| Secrets hidden from workers; keys locked away; secrets removed from logs | 3 | 💬 🤖 |
| Approval levels: just do it / tell me / ask me / make me type "confirm" / never | 3 | 💬 |
| A readable report of everything AI did | 3 | 💬 |
| Big red stop button | 3 | 🤖 |
| Time, money and resource limits | 3 | 💬 |
| "Untrusted" sticker when the AI read internet content | 3 | 🤖 |
| Tests with tricky "trap" instructions | 3 | 🤖 |
| Finds password-like files lying around and offers to hide them | 3 | 🔎 |
| Warns if a change would add or expose a password | 3 | 🔎 |
| You can mark an "untrusted" job as OK after checking it | 3 | 🔎 |
| Each job only reaches the websites it's allowed to | 14 | 💬 |
| Tracks exactly which pieces of information came from untrusted places | 14 | 🤖 |
| Write safety rules in plain words | 14 | 💬 |
| Scans add-ons and scripts before running them | 14 | 💬 |
| Add-ons must be signed and show what they want access to | 14 | 💬 |
| Notices when a worker tries to go where it shouldn't | 14 | 💬 |
| Deep monitoring of what worker programs do | 14 | 🔎 |
| One list of all AI tools, with who's allowed to use each | 14 | 🔎 |
| Secure Boot, disk encryption, firewall | 14 | 🤖 |
| Possibly a separate user account per AI worker | 14 | 🔎 |
| A page showing your security status | 14 | 🔎 |

## ⏪ Priya Rewind

| Feature | Phase | Idea from |
|---|---|---|
| One timeline of every change (AI, settings, you) | 7 | 🤖 |
| Automatic hourly snapshots and snapshots around app installs | 7 | 🤖 |
| Undo one change, keep everything after it | 7 | 🙋 🤖 |
| Bring back any file as it was at any time | 7 | 🤖 |
| Undo by saying what went wrong | 7 | 🙋 |
| Take the whole system back to a point in time | 7 | 🤖 |
| See who changed each file and why | 7 | 💬 |
| Checks that the undo really worked | 7 | 💬 |
| Undo settings changes | 7 | 🤖 |
| Manages disk space for history | 7 | 🤖 |

## 📒 Memory, files and tools

| Feature | Phase | Idea from |
|---|---|---|
| Search past jobs | 4 | 🤖 |
| AI workers get a toolbox from Priya (MCP) | 4 | 🤖 |
| Project memory: how each project works | 8 | 💬 |
| Remembers your preferences | 8 | 💬 |
| Find files by describing them | 8 | 💬 |
| Summaries of folders, documents and code | 8 | 💬 |
| Finds duplicate and old files | 8 | 💬 |
| Connects related files, jobs and projects | 8 | 💬 |
| Separate personal and work memory | 8 | 💬 |
| See, fix, export and delete any memory | 8 | 💬 |
| Private things are only indexed on your laptop | 8 | 💬 |
| "Continue where I left off" | 8 | 💬 |
| Every installed app joins the tool phone book | 8 | 🤖 |
| Workers find tools on their own | 8 | 💬 |
| Overnight tidy-up ("dreaming") and a morning summary | 13 | 🤖 |
| Uses apps through their built-in controls | 11 | 💬 |
| App makers can add actions for AI | 16 | 💬 |

## 🖥️ Talking to your computer (desktop)

| Feature | Phase | Idea from |
|---|---|---|
| Review screen in the terminal | 2 | 🤖 |
| Priya bar (Super + Space): ask and get an answer | 0 | 💬 |
| Plain answers with handy cards (changes, plan, approve, undo) | 5 | 🙋 |
| Knows whether you want an answer or a job done | 5 | 💬 |
| Open apps, files and settings by asking | 5 | 💬 |
| Ask about the app, terminal or project you're in | 5 | 💬 |
| Explains errors and suggests fixes | 5 | 💬 |
| Shows plans, progress and approval requests | 5 | 💬 |
| Notifications with buttons | 5 | 🤖 |
| Status icon in the top bar, with today's cost | 5 | 🤖 |
| The complete Settings app | 5 | 🤖 |
| Priya bar complete: instant app/file results, mode and model buttons, attachments | 5 | 💬 🤖 |
| Small pop-ups for approvals and finished jobs | 3 | 🤖 |
| Task center: all jobs, live progress, review window | 4 | 🤖 |
| "Here's what I'm about to do" preview, changeable before starting | 4 | 🤖 |
| Arranges windows for a task and remembers layouts | 5 | 💬 |
| Background work never jumps in front of you | 5 | 💬 |
| Keyboard, mouse and terminal always work normally | 5 | 💬 |
| Makes small custom apps when you ask ("a dashboard of my deploys") | 16 | 💬 |
| Restyles the desktop when you ask, with a preview first | 16 | 🤖 |

## 🎙️ Voice

| Feature | Phase | Idea from |
|---|---|---|
| Hold Super + V and talk | 10 | 🙋 |
| "Hey Priya" wake word (optional) | 10 | 🙋 |
| Understands speech on the laptop | 10 | 🙋 |
| Speaks short answers | 10 | 🙋 |
| Approve by voice | 10 | 🤖 |
| English and Hindi | 10 | 🙋 |
| Your voice stays on the laptop by default | 10 | 🤖 |

## 👀 Priya Vision

| Feature | Phase | Idea from |
|---|---|---|
| "What's on my screen?" | 11 | 🙋 |
| Explains an error, chart or page from a screenshot | 11 | 🙋 |
| Understands pictures you drop or paste | 11 | 🙋 |
| Reads app windows in a smart way (not just pixels) | 11 | 🤖 |
| Can click or fill things on screen, after you approve | 11 | 💬 |
| Reads text from images when no picture-AI is available | 11 | 🤖 |
| Never looks at password or banking windows | 11 | 🤖 |
| Super + Shift + S to ask about part of the screen | 11 | 🤖 |

## ⏰ Automation

| Feature | Phase | Idea from |
|---|---|---|
| Scheduled jobs ("every morning…") | 13 | 💬 |
| Jobs that start when something happens (file changed, test failed) | 13 | 💬 |
| Watches services and reports problems | 13 | 💬 |
| Safe automatic maintenance | 13 | 💬 |
| Daily project summaries | 13 | 💬 |
| Suggests the next step when stuck | 13 | 💬 |
| Notices repeated work and offers to automate it | 13 | 💬 |
| Shareable "recipes" | 13 | 💬 🔎 |
| Teach by showing: do it once, Priya learns it | 13 | 🤖 |
| Limits on every automation | 13 | 💬 |

## 📱 Devices and teams

| Feature | Phase | Idea from |
|---|---|---|
| Approve and watch from your phone | 15 | 💬 🤖 |
| Continue a job on another computer | 15 | 💬 |
| Use a more powerful computer remotely | 15 | 💬 |
| Share chosen projects with teammates | 15 | 💬 |
| Give jobs to people or AI workers | 15 | 💬 |
| Shared team rules | 15 | 💬 |
| One place for team usage and history | 15 | 💬 |
| Personal files never shared | 15 | 💬 |
| Manage several computers from one place | 15 | 💬 |
| Optional "two people must approve" rule for teams | 15 | 🔎 |

## 🌍 Ecosystem

| Feature | Phase | Idea from |
|---|---|---|
| Kit for others to build PriyaOS blocks | 16 | 🤖 |
| A store of trusted add-ons | 16 | 💬 |
| Checks that add-ons are genuine | 16 | 💬 |
| Apps that work for both people and AI | 16 | 💬 |
| Find apps by asking | 16 | 💬 |
| Normal Linux apps work too | 9 | 💬 |
| Install Priya on top of Omarchy or any Arch | 16 | 🤖 |
| Share your custom mini-apps with others | 16 | 🔎 |

## 🎨 Look and feel

| Feature | Phase | Idea from |
|---|---|---|
| One colour-and-style file that themes everything | 0 | 🤖 |
| Light and dark themes | 0 | 🙋 |
| One set of building blocks for every screen, so everything looks consistent | 0 | 🤖 |
| A "showroom" that shows every screen piece in every theme | 0 | 🤖 |
| Ready for other languages from day 1 | 0 | 🤖 |
| Works fully with the keyboard and screen readers; easy-to-read contrast | 0 | 🤖 |
| Model button in the bar and "why this model?" | 1 | 🙋 🤖 |
| A page showing what AI cost, by day and model | 1 | 🤖 |
| How much Priya asks you: "ask me first" / "ask when it matters" / "just do it", per feature | 3 | 🙋 |
| Clear approval boxes and one place to see everything waiting for you | 3 | 🙋 🤖 |
| A big, clear screen when you press the stop button | 3 | 🤖 |
| Switches light/dark automatically at sunrise and sunset, or on a schedule | 5 | 🙋 |
| True-black theme for your OLED screen | 5 | 🤖 |
| High-contrast themes | 5 | 🤖 |
| Pick your accent colour | 5 | 🤖 |
| Every app, terminal, cursor and icon follows the same theme | 5 | 🙋 🤖 |
| Smooth, gentle animations, with an option to turn them down | 5 | 🤖 |
| Bigger text, screen reader, keyboard-only, all fully tested | 5 | 🤖 |
| Protects your OLED screen from burn-in | 5 | 🔎 |
| Start screen, lock screen, login and installer in the same style | 9 | 🤖 |
| PriyaOS screens in Hindi | 10 | 🙋 |
| Accent colour picked from your wallpaper; share themes | 16 | 🤖 |

## ✅ Quality and automatic testing

| Feature | Phase | Idea from |
|---|---|---|
| Tests run automatically on GitHub every time code is pushed | 0 | 🙋 |
| Changes only go in after the tests pass | 0 | 🤖 |
| Quick checks on the computer before code is even sent | 0 | 🤖 |
| A test toolkit: pretend AIs, pretend workers, saved examples | 0 | 🤖 |
| Every finished feature must have tests (checked automatically) | 0 | 🙋 🤖 |
| New code must come with tests (checked automatically) | 0 | 🙋 🤖 |
| Old features' tests run forever, so nothing breaks silently | 0 | 🙋 |
| Catches changes that would break other parts that talk to each other | 0 | 🤖 |
| Screens photographed in every theme and compared, plus accessibility checks | 0 | 🙋 🤖 |
| Checks that the code we use from others is safe and allowed | 0 | 🤖 |
| Unreliable tests get fixed quickly, never ignored | 0 | 🤖 |
| Tests that measure how well Priya AI and the workers do | 1 | 🤖 |
| Small real tests with each AI company, with tiny budgets | 1 | 🤖 |
| Tests that check the tests themselves are strong | 1 | 🤖 |
| Pretend-computer tests run on GitHub | 2 | 🤖 |
| Every night: full install and use in a virtual computer | 2 | 🤖 |
| Speed limits that tests enforce | 2 | 🤖 |
| "Pull the plug" tests | 2 | 🤖 |
| Your Zenbook can run tests that need real hardware | 2 | 🤖 |
| A usability session with you, with a score, before the desktop is called done | 5 | 🔎 |
| Safety trap tests on every change | 3 | 🤖 |
| Automatic, signed releases with the installer | 9 | 🤖 |

## 🏠 The operating system itself

| Feature | Phase | Idea from |
|---|---|---|
| Install it as an app on any Arch | 0 | 🤖 |
| Our own app store server for Priya's parts | 9 | 🤖 |
| Installer USB | 9 | 💬 |
| Installer that never touches Windows | 9 | 🤖 |
| First-start setup: AI accounts, modes, privacy | 9 | 🤖 |
| Desktop, lock screen and sleep set up nicely | 9 | 💬 |
| Wi-Fi, Bluetooth, sound, battery and screen settings | 9 | 🤖 |
| Keyboard languages including Hindi | 9 | 🤖 |
| App installs go through the sandbox | 9 | 🤖 |
| Printing and scanning (optional) | 9 | 🤖 |
| Accessibility (screen reader, bigger text, contrast) | 9 | 🤖 |
| Several user accounts | 9 | 🤖 |
| PriyaOS look: boot screen, login, wallpapers | 9 | 🤖 |
| Optional backup to another drive or the cloud | 9 | 🤖 |
| Disk encryption option, with help keeping the recovery key safe | 9 | 🔎 |
| Firewall switched on by default, with a settings page | 9 | 🔎 |
| Laptop firmware updates | 9 | 🔎 |
| Bring back your old configs after installing | 9 | 🔎 |
| Early test installer so you can use PriyaOS on the laptop sooner | 2 | 🤖 |
| Beautiful graphical installer | 9 | 🤖 |

Next: [The phases](05-the-phases.md)
