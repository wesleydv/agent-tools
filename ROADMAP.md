# Roadmap

What `work` should grow into. Each item is a file in
[`todo/`](todo/) with the detail; this page is the order and the reasoning.

## Which tools belong here

A tool moves into agent-tools when all of these hold:

1. **More than one repo uses it, or would.** One user means it stays where it is.
2. **It's about how agents work in a repo** (workflow, records, conventions, long jobs), not about
   what the repo manages.
3. **Its project-specific parts fit in config** (`.work.toml` or `~/.config/agent-tools/`). Moving
   it must not bring host names, paths or content from a project into this repo.
4. **Any agent can use it.** A part that only works with one agent (a Claude Code hook) may come
   along as an optional adapter, but the core must work for any agent or a person.

## Public

The repo is public since 2026-11-22. Its own `.work.toml` names a local denylist, so a commit
here is refused when it adds a private name (README, Denylist).

## Phase 1: records (small, low risk)

- [`work-todo`](todo/work-todo.md): `work todo` lists `todo/*.md` most urgent first, and
  `work todo new` writes the front matter. Replaces the per-project scripts that do this today.
- [`conventions-doc`](todo/conventions-doc.md): the `log/` and `todo/` formats written once, here;
  each project's AGENTS.md keeps only its own systems list.
- [`work-lint`](todo/work-lint.md), after the conventions doc: `work lint` and a pre-commit check
  of staged `log/` and `todo/` front matter.

## Phase 2: drift and long jobs (bigger, needs generalising)

- [`work-drift`](todo/work-drift.md): diff repo-managed files against their live copies, locally
  or over ssh, configured in `.work.toml`. Read-only. Before `rjob`: smaller, and it proves the
  config-driven pattern.
- [`rjob-move`](todo/rjob-move.md), last and in a quiet window: long remote jobs as systemd
  units, as a second binary in a cargo workspace, with allowed hosts from per-machine config.

## Not now

- **Delegating tasks to other agents in worktrees.** Only one project does it, and its rules are
  about what that project manages. Revisit when a second repo wants it.

Each step is its own commit series here, with end-to-end tests and a version bump, plus one
commit per project that raises its `requires`.
