# agent-tools

One git workflow for every repo that coding agents work in (Claude Code, opencode,
agy, or a person). `work` is a single Rust binary that shells out to `git`, so hooks,
config and the index behave exactly as with plain git. Nothing here depends on one
agent's features.

## The workflow

- **Approving a change approves its commit and push.** Agents ask before *making* a
  change that needs a go. Once it's approved, they commit and push it without asking again.
- **Work is done when it is committed.** The change, its todo update and its log entry
  (if any) go in one commit, pushed right away. Follow-ups (a scheduled check, a test
  result) are their own commits.
- **Holding back is deliberate:** `work hold <slug> "reason"`. Silence is never a hold.
- **Commit only your own files, named explicitly.** Agents share a working tree; a
  dirty file you didn't change is someone else's work.
- **Commit messages record changes; the log folder records events.** A deploying
  commit has a body that says why (the hook enforces it). A log entry is for what git
  can't show: incidents, investigations, measurements, changes made outside the repo.
  The commit links to the entry with a `Log: log/<file>.md` trailer, never the
  other way round.

`templates/AGENTS-workflow.md` is that text in AGENTS.md form, for projects to copy.

## Commands

```bash
work start <slug> --agent claude              # before your first edit; resumes if it exists
work claim <slug> <files…>                    # optional: name files as yours early
work commit <slug> -m "subject" -m "body" -- <files…>
                                              # commits exactly these files, pushes if the branch has an upstream
work hold <slug> "reason"                     # deliberate hold
work done <slug>                              # refused while the record's files are dirty
work status                                   # open records, holds, dirty files nobody owns
work check [--orphan-age 2h] [--idle 6h]      # exit 1 if work was left behind
work history [system] [-s regex] [-n 40] [--since YYYY-MM-DD] [-a]
work watch [--notify]                         # check every registered repo (launchd runs this hourly)
work setup [--check]                          # register this repo, load the watcher
work hook pre-commit [--all]                  # denylist check (below); --all: every tracked file
```

`--agent` defaults to `$WORK_AGENT`. Records live in `<git-common-dir>/work/<slug>/`:
local, never committed, shared by a repo's worktrees.

- `commit` refuses a file another open record has named, and leaves whatever else is
  staged alone. Afterwards it lists files that became dirty since the record started
  and nobody named (a forgotten shell edit, or another agent's work).
- If the push is refused, it stops and says so. It never rebases under other agents.
- **Adopting orphans:** dirty files named by no record are left from a session that
  never finished. Read the diff (for config, compare with what actually runs), then
  `work start adopt-<slug>` and commit them with a message that says they were adopted.

## Denylist

A guard against one project's content leaking into a repo where it must not be (a
public repo, or one shared with others). `work hook pre-commit` refuses a commit when a
staged **added** line or a staged file name matches the denylist; `work hook commit-msg`
checks the message the same way. Removed lines never match, so cleaning up is never blocked.

- **Opt-in per repo:** `.work.toml` names the list, `denylist = "~/.config/agent-tools/denylist-public"`
  (`~/` expands; a relative path is from the repo root). What must not leak depends on
  where it would leak to: a public repo refuses the private projects' names, which those
  projects use all the time. So a repo without the key is not checked, and several repos
  can share one list by naming the same file.
- **The list:** one case-insensitive regex per line, blank and `#` lines skipped; wrap
  common words in `\b`. It must be outside the repo or gitignored: `work` refuses one that
  would be committed. Each machine has its own copy, written by the owner; never commit it.
- **The output** names the file, the line and which line of which list matched. It never
  shows the pattern or the matched text: the list is the secret, and the output ends up in
  agent transcripts.
- **Fail closed:** a list the repo names but this machine doesn't have, or a bad regex,
  refuses the commit (a bad regex is reported by line, without quoting it).
- `work hook pre-commit --all` checks every tracked file (its working copy) and its name,
  for a one-off check of the whole tree.

## Install

```bash
cargo install --git https://github.com/wesleydv/agent-tools --root ~/.local   # just the binary
```

Or from a clone, to work on it:

```bash
git clone git@github.com:wesleydv/agent-tools.git ~/code/agent-tools
cd ~/code/agent-tools && cargo install --path . --root ~/.local   # -> ~/.local/bin/work
```

To update, run the same install again (from a clone, after `git pull`). A repo whose `.work.toml` has `requires = "x.y.z"`
refuses an older `work`, the commit-msg hook included, so a stale install can't go unnoticed.

## Adding a repo

1. `.work.toml` at the root (below).
2. `.githooks/commit-msg` from `templates/commit-msg` (it runs `work hook commit-msg`) and
   `.githooks/pre-commit` from `templates/pre-commit` (`work hook pre-commit`), next to or
   inside any hooks of the repo's own, and `git config core.hooksPath .githooks`.
3. The workflow text from `templates/AGENTS-workflow.md` in the repo's AGENTS.md.
4. `work setup` from inside the repo: registers it with the watcher
   (`~/.config/agent-tools/repos`) and loads the launchd job `local.agent-tools.watch`.
   `work setup --check` is the read-only version for a repo's doctor script.

## Configuration: `.work.toml`

Every key is optional.

```toml
requires = "0.2.1"                   # oldest work allowed here

# Commits touching these paths deploy something: the commit-msg hook wants a body
# or a Log: trailer.
deploy_paths = ['^hosts/[^/]+/(stacks|host)/', '^areas/devices/']

default_system = "repo"              # paths no rule matches
log_dir = "log"                      # dated YYYY-MM-DD-slug.md event entries
meta_dirs = ["log", "todo"]          # files that name their own `systems: [...]` in front matter
denylist = "~/.config/agent-tools/denylist-public"   # opt-in word list, never committed (Denylist)

# Path -> systems for `work history`. The first matching rule wins; $1 expands captures.
# Tables come last: a key after a [[systems]] table belongs to that table.
[[systems]]
match = '^hosts/([^/]+)/'
systems = ["$1"]

[[systems]]
match = '^areas/devices/([^/.]+)\.yaml$'
systems = ["devices", "$1"]
```

## Development

`cargo test` runs the unit tests and the end-to-end tests: the real binary against
throwaway repos, covering ownership, the hook, pushing, the watcher and history. CI
runs fmt, clippy (`-D warnings`) and the tests on every push.

## License

MIT OR Apache-2.0, at your option: [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE).
