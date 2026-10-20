# Gallery

Shows every PriyaUI component in every state, in light and dark, for design review and screenshot tests (sub-phase 0.11).

```sh
cargo xtask gallery              # open it
cargo xtask gallery --scale 2    # everything twice as large (QT_SCALE_FACTOR=2)
```

The sidebar lists the components; the chips at the top switch light and dark, text size 200 %, Hindi samples and reduced motion. Everything works with the keyboard: Tab moves through the sidebar and the stories.

- `Main.qml`: the window, and shot mode for screenshots (`--story <Component>/<state> --theme <t> --out <png>`, `--all --out-dir <dir>`, `--list`). Shot mode turns animations off, renders and quits.
- `Catalog.js`: the components the gallery lists, in order.
- `stories/<Component>.stories.qml`: a `StoryBook` of named `Story` states with fixed props and fixed text, no timers, so a picture of a story never changes by itself.

A new component gets a stories file and a line in `Catalog.js`; `ui/tests/gallery/tst_stories.qml` loads every story and runs the accessibility audit on it.
