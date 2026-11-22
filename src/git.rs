//! Thin wrappers around the `git` CLI. Shelling out (rather than libgit2) keeps
//! hooks, config and the index behaving exactly as they do for plain git.

use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};

/// Run git and return stdout; git's stderr goes to ours.
pub fn out(args: &[&str]) -> Result<String> {
    let o = Command::new("git")
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Run git quietly; true when it exits 0.
pub fn ok(args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run git with the terminal attached (hook output, commit summary); true on exit 0.
pub fn run<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> Result<bool> {
    Ok(Command::new("git").args(args).status().context("running git")?.success())
}

/// Dirty paths relative to the repo root: modified, staged, deleted and
/// untracked (every file, not just its directory).
pub fn dirty() -> Result<Vec<String>> {
    parse_porcelain_z(&out(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?)
}

pub fn parse_porcelain_z(raw: &str) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    let mut it = raw.split('\0');
    while let Some(entry) = it.next() {
        if entry.is_empty() {
            continue;
        }
        if entry.len() < 4 {
            bail!("unexpected git status entry {entry:?}");
        }
        paths.push(entry[3..].to_string());
        // A rename or copy is followed by its original path.
        if matches!(entry.as_bytes()[0], b'R' | b'C') {
            it.next();
        }
    }
    Ok(paths)
}

pub fn has_upstream() -> bool {
    ok(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_renames_and_spaces() {
        let raw = " M a.txt\0R  new name.md\0old name.md\0?? dir/f g\0D  gone\0";
        assert_eq!(parse_porcelain_z(raw).unwrap(), vec!["a.txt", "new name.md", "dir/f g", "gone"]);
    }
}
