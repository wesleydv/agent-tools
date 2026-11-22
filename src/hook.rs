//! commit-msg: commit messages are the record of repo changes.
//!
//! 1. A commit touching a deploy path (`.work.toml` `deploy_paths`) says why:
//!    a body under the subject, or a `Log: <log file>` trailer.
//! 2. A `Log:` trailer names a file that is staged or in the tree.
//! 3. No attribution trailers (Co-Authored-By, "Generated with ...").
//! 4. Nothing in the message matches the denylist (`denylist.rs`).

use crate::{Repo, denylist, git};
use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

const SCISSORS: &str = "# ------------------------ >8 ------------------------";

pub fn check_message(repo: &Repo, msg: &str, staged: &[String]) -> Result<Vec<String>> {
    let lines: Vec<&str> =
        msg.lines().take_while(|l| *l != SCISSORS).filter(|l| !l.starts_with('#')).collect();
    let subject = lines.first().copied().unwrap_or("");
    let mut problems = denylist::check_message(repo, &lines)?;
    if ["Revert", "fixup!", "squash!", "amend!"].iter().any(|p| subject.starts_with(p)) {
        return Ok(problems);
    }

    let attribution = Regex::new(r"(?im)^co-authored-by:|generated with \[?claude|🤖 generated").unwrap();
    if lines.iter().any(|l| attribution.is_match(l)) {
        problems.push(
            "no attribution trailers in commit messages (Co-Authored-By, \"Generated with ...\")".into(),
        );
    }

    let logs: Vec<&str> = lines.iter().filter_map(|l| l.strip_prefix("Log:")).map(str::trim).collect();
    for l in &logs {
        if !git::ok(&["cat-file", "-e", &format!(":{l}")]) && !Path::new(l).exists() {
            problems.push(format!("Log: trailer names {l}, which is neither staged nor in the tree"));
        }
    }

    let deploys: Vec<&String> = staged.iter().filter(|p| repo.cfg.deploys(p)).collect();
    if !deploys.is_empty() && logs.is_empty() {
        let trailer = Regex::new(r"^(?i)(Log|Refs|Signed-off-by|Co-Authored-By):").unwrap();
        let has_body = lines.iter().skip(1).any(|l| !l.trim().is_empty() && !trailer.is_match(l));
        if !has_body {
            let shown: Vec<&str> = deploys.iter().take(3).map(|s| s.as_str()).collect();
            problems.push(format!(
                "this commit deploys ({}{}) but has no body.\n  Say why under the subject (2-5 lines: what changes \
                 for the running system and why),\n  or add a trailer 'Log: <log file>' when an entry tells the story.",
                shown.join(" "),
                if deploys.len() > 3 { " …" } else { "" }
            ));
        }
    }
    Ok(problems)
}

pub fn commit_msg(repo: &Repo, file: &Path) -> Result<i32> {
    if repo.git_dir.join("MERGE_HEAD").exists() {
        return Ok(0);
    }
    let msg = std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let staged: Vec<String> =
        git::out(&["diff", "--cached", "--name-only"])?.lines().map(String::from).collect();
    let problems = check_message(repo, &msg, &staged)?;
    if problems.is_empty() {
        return Ok(0);
    }
    for p in &problems {
        eprintln!("BLOCKED: {p}");
    }
    eprintln!("commit-msg: commit refused. Message kept in {}", file.display());
    Ok(1)
}
