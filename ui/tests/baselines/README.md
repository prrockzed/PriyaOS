# Screenshot baselines

`<Component>/<state>/<theme>@<scale>.png` for every gallery story, light and dark, scale 1 and 2, rendered in the CI image by `cargo xtask test ui --shots --update-baselines` (sub-phase 0.11). `cargo xtask test ui --shots` compares new pictures with these.

A pull request that changes or removes a picture here needs the owner's `visual-change-approved` label; the PR report shows the before, after and diff pictures. New pictures for new stories don't need it.
