//! Denylist: refuse commits whose added text, file names or message match a
//! local word list, so content from one project can't leak into a repo where
//! it must not be. The list is the secret: it is never committed, and nothing
//! here prints a pattern or the text it matched, only where.
//!
//! Opt-in per repo: `.work.toml` names the list in `denylist` (a path, `~/`
//! expanded, relative to the repo root otherwise), because what must not leak
//! depends on where it would leak to. A repo without the key is not checked.
//! One case-insensitive regex per line; blank lines and `#` lines are skipped.

use crate::{Repo, git};
use anyhow::{Context, Result, bail};
use regex::{Regex, RegexBuilder};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Entry {
    re: Regex,
    /// "line 3 of ~/.config/agent-tools/denylist"
    pub label: String,
}

pub struct Denylist {
    pub entries: Vec<Entry>,
}

fn shown(p: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) if p.starts_with(&h) => format!("~/{}", p.strip_prefix(&h).unwrap().display()),
        _ => p.display().to_string(),
    }
}

fn expand(repo: &Repo, p: &str) -> Result<PathBuf> {
    if let Some(rest) = p.strip_prefix("~/") {
        return Ok(PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join(rest));
    }
    let p = PathBuf::from(p);
    Ok(if p.is_absolute() { p } else { repo.root.join(p) })
}

fn parse(path: &Path, text: &str, entries: &mut Vec<Entry>) -> Result<()> {
    for (i, line) in text.lines().enumerate() {
        let pat = line.trim();
        if pat.is_empty() || pat.starts_with('#') {
            continue;
        }
        let label = format!("line {} of {}", i + 1, shown(path));
        // The regex error would quote the pattern: say only where it is.
        let Ok(re) = RegexBuilder::new(pat).case_insensitive(true).build() else {
            bail!("denylist: {label} is not a valid regex");
        };
        entries.push(Entry { re, label });
    }
    Ok(())
}

impl Denylist {
    /// The repo's list; None when `.work.toml` names none. A named list that
    /// is missing is an error: the check fails closed.
    pub fn load(repo: &Repo) -> Result<Option<Denylist>> {
        let Some(p) = &repo.cfg.denylist else { return Ok(None) };
        let p = expand(repo, p)?;
        if !p.exists() {
            bail!(
                ".work.toml: denylist {} does not exist on this machine (the owner writes it; it is never committed)",
                shown(&p)
            );
        }
        if let Ok(rel) = p.canonicalize()?.strip_prefix(repo.root.canonicalize()?) {
            let rel = rel.to_string_lossy();
            let tracked = git::ok(&["ls-files", "--error-unmatch", "--", &rel]);
            if tracked || !git::ok(&["check-ignore", "-q", "--no-index", "--", &rel]) {
                bail!(
                    ".work.toml: denylist {rel} is inside the repo and tracked or not gitignored: it would be committed"
                );
            }
        }
        let text = fs::read_to_string(&p).with_context(|| format!("reading {}", shown(&p)))?;
        let mut entries = Vec::new();
        parse(&p, &text, &mut entries)?;
        Ok(Some(Denylist { entries }))
    }

    /// Labels of the entries that match `text`.
    pub fn matches(&self, text: &str) -> Vec<&str> {
        self.entries.iter().filter(|e| e.re.is_match(text)).map(|e| e.label.as_str()).collect()
    }

    /// One problem per matching line; `place` names line `n`.
    pub fn check_lines<'a>(
        &self,
        place: impl Fn(usize) -> String,
        lines: impl Iterator<Item = &'a str>,
        out: &mut Vec<String>,
    ) {
        for (i, line) in lines.enumerate() {
            for label in self.matches(line) {
                out.push(format!("{} matches {label}", place(i + 1)));
            }
        }
    }

    pub fn check_name(&self, path: &str, out: &mut Vec<String>) {
        for label in self.matches(path) {
            out.push(format!("file name {path} matches {label}"));
        }
    }
}

/// Added lines of the staged diff: (path, new line number, text).
pub fn staged_added_lines() -> Result<Vec<(String, usize, String)>> {
    let diff = git::out(&[
        "-c",
        "core.quotePath=false",
        "diff",
        "--cached",
        "-U0",
        "--no-color",
        "--no-ext-diff",
        "--diff-filter=d",
    ])?;
    let mut added = Vec::new();
    let mut file = String::new();
    let mut n = 0;
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            let p = p.trim_matches('"');
            file = p.strip_prefix("b/").unwrap_or(p).to_string();
        } else if line.starts_with("@@ ") {
            // @@ -a[,b] +c[,d] @@
            n = line
                .split(' ')
                .find_map(|w| w.strip_prefix('+'))
                .and_then(|w| w.split(',').next())
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
        } else if let Some(text) = line.strip_prefix('+') {
            added.push((file.clone(), n, text.to_string()));
            n += 1;
        }
    }
    Ok(added)
}

fn report(problems: &[String], what: &str) -> i32 {
    const SHOWN: usize = 40;
    for p in problems.iter().take(SHOWN) {
        eprintln!("BLOCKED: {p}");
    }
    if problems.len() > SHOWN {
        eprintln!("BLOCKED: … and {} more", problems.len() - SHOWN);
    }
    eprintln!(
        "{what}: matches a denylist entry (the entry and the matched text are not shown: the list is private). \
         Remove the text, or fix the entry if it is wrong."
    );
    1
}

/// `work hook pre-commit`: the staged added lines and file names. With
/// `all`, every tracked file in the working tree and its name instead.
pub fn pre_commit(repo: &Repo, all: bool) -> Result<i32> {
    let Some(list) = Denylist::load(repo)? else {
        if all {
            bail!("no denylist configured for this repo (`denylist` in .work.toml)");
        }
        return Ok(0);
    };
    let mut problems = Vec::new();
    if all {
        let files = git::out(&["ls-files", "-z"])?;
        for f in files.split('\0').filter(|f| !f.is_empty()) {
            list.check_name(f, &mut problems);
            let Ok(bytes) = fs::read(repo.root.join(f)) else { continue };
            if bytes.contains(&0) {
                continue; // binary
            }
            let text = String::from_utf8_lossy(&bytes);
            list.check_lines(|n| format!("{f}:{n}"), text.lines(), &mut problems);
        }
        if problems.is_empty() {
            let n = list.entries.len();
            println!(
                "denylist: every tracked file passes ({n} {})",
                if n == 1 { "entry" } else { "entries" }
            );
            return Ok(0);
        }
        return Ok(report(&problems, "denylist"));
    }
    let names = git::out(&["diff", "--cached", "--name-only", "--diff-filter=d", "-z"])?;
    for f in names.split('\0').filter(|f| !f.is_empty()) {
        list.check_name(f, &mut problems);
    }
    for (f, n, text) in staged_added_lines()? {
        for label in list.matches(&text) {
            problems.push(format!("{f}:{n} matches {label}"));
        }
    }
    if problems.is_empty() {
        return Ok(0);
    }
    Ok(report(&problems, "pre-commit: commit refused"))
}

/// Problems for a commit message's kept lines (numbered from 1).
pub fn check_message(repo: &Repo, lines: &[&str]) -> Result<Vec<String>> {
    let Some(list) = Denylist::load(repo)? else { return Ok(Vec::new()) };
    let mut problems = Vec::new();
    list.check_lines(|n| format!("commit message line {n}"), lines.iter().copied(), &mut problems);
    Ok(problems)
}
