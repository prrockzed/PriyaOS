# Icons

[Lucide](https://lucide.dev) icons used by PriyaUI, from the npm package `lucide-static` **1.50.0** (ISC licence, in [LICENSE](LICENSE)). Only icons a component or story uses are kept. The one change from upstream: `stroke-width` is 1.75 instead of 2, as [11 §2.5](../../../docs/claude/architecture/11-design-system-and-ux.md#25-icons-and-imagery) asks. `Icon.qml` tints them with a Theme colour, so the SVGs keep `stroke="currentColor"`.

To add one, take it from the same release and change only the stroke width:

```sh
curl -sSfO https://registry.npmjs.org/lucide-static/-/lucide-static-1.50.0.tgz
# Check it against the registry's integrity value:
# sha512-Pge6F5Au0bEBvjaeBIjfU5PsmA43FHdM3s5g3rYqSNWvBO3o7WwDSNQSavwX76z/OoSCjwSVbQpGA9lqKud/WA==
echo "sha512-$(openssl dgst -sha512 -binary lucide-static-1.50.0.tgz | base64)"
tar -xzf lucide-static-1.50.0.tgz package/icons/<name>.svg
sed 's/stroke-width="2"/stroke-width="1.75"/' package/icons/<name>.svg > ui/PriyaUI/icons/<name>.svg
```

Then add the name to `tst_icon.qml`'s list.

| Icon | Used for |
|---|---|
| `arrow-up` | Send (PromptInput) |
| `box` | Sandbox mode chip |
| `check` | Success toast, selected chip, "Allow once" |
| `cloud-off` | Offline error state |
| `copy` | Copy buttons (code block, answer, details) |
| `eye` | Look mode chip |
| `folder` | Folder chip |
| `history` | Empty Rewind state |
| `inbox` | Empty task list |
| `info` | Info toast |
| `package` | Card header example |
| `refresh-cw` | Try again |
| `sparkles` | Priya, model label, thinking |
| `square` | Stop |
| `triangle-alert` | Errors, Direct raw mode chip |
| `undo-2` | Undo |
| `x` | Close and dismiss |
| `zap` | Direct mode chip |
