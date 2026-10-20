# settings-edit fixtures

Written by hand on 2026-10-20, not recorded from a tool: `commented.toml` is
a settings file as an owner might keep it, with comments on their own lines,
trailing comments, section rulers, odd spacing and a commented-out key. The
golden files under `../../snapshots/` are what `priya_core::settings::set`
and `unset` make of it; review them by eye when they change.
