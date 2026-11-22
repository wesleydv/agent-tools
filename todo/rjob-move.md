---
title: "Move rjob (long remote jobs as systemd units) into this repo as a second binary"
systems: [rjob]
priority: low
status: open
brief: ROADMAP.md
created: 2026-11-22
---

Last of the moves, in a quiet window: sessions in its current project route every shell call through its guard. The detail on its current home and coupling is in servers' brief `agent-tools-more-tools.md` (owner-only).

- Cargo workspace here: `work` and `rjob` as two binaries.
- **Import fresh, no `git subtree split`:** its history has the other project's commit messages (hosts, incidents). Rewrite tests and examples with made-up hosts.
- Allowed hosts from `~/.config/agent-tools/rjob.toml` (per machine), not compiled in.
- The Claude Code hook entry points (`_guard`, `_arm`, `_session-start`) stay as subcommands documented as the Claude adapter. Without them `rjob status`/`check` still work, there are just no wake-ups; say so in the README. Unverified: whether opencode/agy sessions manage fine without them.
- The old project keeps its `bin/rjob` as a shim to the installed binary, with a fallback to its in-repo build until one real job has run through the new one; then remove the in-repo copy.
