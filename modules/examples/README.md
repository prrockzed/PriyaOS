# Example modules

Small example modules that show how to plug a feature into PriyaOS without touching the core (sub-phase 0.8). They aren't packaged; they document the SDK and serve the tests.

- [`echo`](echo/): Rust, written with `priya-module-sdk`. Runs as a process (`echo-module`) or, built into priyad, as a builtin.
- `echo-py`: the same module in Python 3, standard library only, with its own framing code.

Each folder has a `module.toml` (02-core §5.1) and a `settings.schema.json` for its `[modules.<name>]` settings.
