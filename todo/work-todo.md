---
title: "work todo: port the todo lister to Rust, plus todo new"
systems: [work]
priority: medium
status: open
brief: ROADMAP.md
created: 2026-11-22
---

Two projects carry the same Python `bin/todo` (a table of `todo/*.md`, most urgent first, filter by system or priority); a third has a `todo/` folder without it.

- `work todo [system] [-p urgent|high|medium|low]`: same columns (priority, status, systems, file, title) and order; folder from `meta_dirs` (the entry that isn't `log_dir`, or a new `todo_dir` key, default `todo`). Quoted titles with colons must work (that's why they're quoted).
- `work todo new <slug> --title "…" --systems a,b --priority p [--brief path]`: writes the front matter with `status: open` and today's `created`, refuses an existing slug.
- End-to-end tests for ordering, filters, quoted titles, `new`. Bump the minor version.
- Then in each project (its own workflow): `bin/todo` becomes `exec work todo "$@"` for one release, raise `requires`, later remove it.
