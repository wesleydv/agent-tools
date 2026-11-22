# AGENTS.md: agent-tools

This repo builds `work`, the shared git workflow tool for the owner's projects.
Read [README.md](README.md) first: it is the
reference that agents in those projects follow.

- **Rust, and it must stay clean:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  and `cargo test` all pass before every commit (CI runs the same).
- **Every behaviour change gets an end-to-end test** in `tests/cli.rs` (the real binary
  against a throwaway repo). The projects depend on this tool's behaviour, not just on
  its compiling.
- **Nothing project-specific here.** No paths, names or content from the projects:
  those go in each project's `.work.toml`. This repo is on GitHub and some projects
  are not: nothing from a private project may leak into it, not even its name.
- **Compatibility:** projects pin a minimum with `requires` in `.work.toml`. Bump the
  version in `Cargo.toml` for any change a project relies on; keep the record format
  (`<git-common-dir>/work/<slug>/meta|snapshot|files`) readable by older versions,
  or say so in the commit.
- After a change: `cargo install --path . --root ~/.local` to update this Mac.
- This repo follows its own workflow (README, The workflow): `work commit` with explicit
  files, a body on any change to `src/`, no attribution trailers.
- **Open work** is in `todo/<slug>.md`, one file per item, in the same front-matter format the
  projects use (quoted `title`, `systems`, `priority`, `status`, `brief`, `created`); `ls todo/`
  until `work todo` exists. Only what is left; `git rm` the file in the commit that finishes it.
  Todos here are public: no project names beyond "servers", no hosts, no paths.
