<!-- Copy into a project's AGENTS.md (agent-tools templates/AGENTS-workflow.md).
     Adjust: what deploys here, the log folder's name, and replace <owner> with
     whoever gives the go in this project. -->

## Git workflow

Every agent working here follows these rules, whatever tool runs it. The tool is
`work` from agent-tools (its README has the details); `work --version` checks it is
installed.

- **Approving a change approves its commit and push.** Ask <owner> *before making* a
  change that needs their go. Once they say yes (or the change needed no go), commit and
  push it without asking again.
- **Work is done when it is committed.** The change, its todo update and its log entry
  (if any) go in one commit, pushed right away. Every follow-up (a scheduled check, a
  test result) is its own commit.
- **Holding back is deliberate.** When <owner> says "don't commit yet", record it:
  `work hold <slug> "reason"`. Silence is never a hold.
- **Commit only your own files, named explicitly.** Several agents share this working
  tree; a dirty file you didn't change is someone else's work.

```bash
work status                                             # at session start: open work, orphans
work start <slug> --agent <claude|opencode|agy|…>       # before your first edit
work commit <slug> -m "subject" -m "body" -- <files…>   # commits exactly these files, pushes
work hold <slug> "reason"                               # deliberate hold
work done <slug>                                        # at the end
work history <system>                                   # commits + log entries, one timeline
```

- Use the todo or log slug as the work slug when there is one, so a later session (or
  a follow-up) resumes it.
- `work status` lists dirty files nobody owns. Mention them at session start and offer
  to adopt them: read the diff, compare config with what actually runs, then commit
  under a `work start adopt-<slug>` record.

## History: commit messages and log entries

| | Commit message | Log entry |
|---|---|---|
| **Records** | The change: what this diff does and why | The event: what git can't show |
| **Needed for** | Every commit | Incidents, investigations, measurements, changes made outside the repo |
| **Written** | With the change, in the same commit | When it happens; each follow-up committed when written |

- **Subject** `<area>: <what changed>`. **Body** (2-5 lines, what changes and why)
  required on commits that deploy (`deploy_paths` in `.work.toml`); the commit-msg
  hook refuses them without a body or a `Log:` trailer.
- **`Log: log/<file>.md`** trailer when an entry tells the story. The link goes from
  the commit to the entry: entries have no `commits:` field.
- No attribution trailers (`Co-Authored-By`, "Generated with …"); the hook refuses them.
