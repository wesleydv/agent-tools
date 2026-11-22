---
title: "work drift: diff repo-managed files against their live copies"
systems: [work]
priority: low
status: open
brief: ROADMAP.md
created: 2026-11-22
---

Projects keep files that mirror a live location: host config over ssh (two near-identical per-host diff scripts today), or local files (LaunchAgent plists). One read-only, config-driven tool replaces the scripts.

```toml
[[drift]]
name = "host-a"
dir = "hosts/host-a/host"          # repo side; README.md is skipped
remote = "host-a"                  # ssh host; omit for this machine
root = "/"                         # live path = root + relative path
read = "sudo cat"                  # how to read a live file
special = { "etc/cron/root.crontab" = "sudo crontab -l" }
```

- `work drift [name] [-v]`: per file `same` / `DRIFT` (with `-v` a unified diff) / `MISSING live`; exit 1 on any drift. Never applies anything.
- A project's extra checks that aren't file comparisons stay as its own small scripts.
- End-to-end tests with `remote` omitted (local dirs); ssh is a thin wrapper around the same comparison.
- Examples and tests use made-up hosts only.
