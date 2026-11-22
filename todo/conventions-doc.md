---
title: "One canonical doc for the log/ and todo/ formats"
systems: [repo]
priority: medium
status: open
brief: ROADMAP.md
created: 2026-11-22
---

The log and todo formats (front matter keys and values, one file per event or item, follow-ups in the same file, `git rm` when done, no `commits:` field, `Log:` trailers) are restated in each project's AGENTS.md, and the copies drift (one pointed at a section that no longer existed).

- Write them once here: a README section or `templates/conventions.md`, generic (no project systems).
- Each project's AGENTS.md then keeps only its own `systems` list and links here. Do that in each project with its own workflow; list them in the commit message here, not by path.
- Pairs with `work-lint`, which enforces what this describes.
