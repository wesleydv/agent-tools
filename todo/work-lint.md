---
title: "work lint + pre-commit check of log/ and todo/ front matter"
systems: [work]
priority: medium
status: open
brief: ROADMAP.md
created: 2026-11-22
---

After `conventions-doc`. Agents drift from the format (a missing `result`, an unquoted title with a colon, a `commits:` field in a new entry, a misspelled system).

- `.work.toml`: `systems_known = [...]`, and optional allowed values for `type`, `result`, `priority`, `status` (defaults: the current conventions).
- `work lint [files…]`: required keys, allowed values, known systems, the `date` key matching the filename's date, quoted todo titles, no `commits:` in new entries.
- In `work hook pre-commit` (shared with `denylist-pre-commit`): lint staged log/todo files only, so old entries never block a commit.
- **Decision for the owner first:** an unknown system is an error or a warning? (Device-level system names keep growing in one project; a warning that names the list to extend may be enough.)
- End-to-end tests per rule. Bump the minor version; projects add `systems_known` and raise `requires`.
