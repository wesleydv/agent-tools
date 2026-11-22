//! start / claim / commit / hold / done / status / check.

use crate::store::{self, Record, Store};
use crate::{Repo, git, watch};
use anyhow::{Result, bail};
use chrono::Local;
use std::path::Path;
use std::time::SystemTime;

fn norm(files: &[String]) -> Vec<String> {
    files.iter().map(|f| f.trim_start_matches("./").to_string()).collect()
}

fn claim_into(repo: &Repo, rec: &mut Record, files: &[String]) -> Result<()> {
    let others = repo.store.all()?;
    for f in files {
        if let Some(o) = Store::owner(&others, f, Some(&rec.slug)) {
            bail!(
                "{f} is already named by record '{}' ({}); leave it, or ask whoever owns it",
                o.slug,
                o.agent()
            );
        }
        if !rec.files.contains(f) {
            rec.files.push(f.clone());
        }
    }
    Ok(())
}

/// Dirty now, not dirty when `rec` started, named by no record: probably this
/// agent's forgotten edits (a shell edit, a generated file), or a concurrent
/// agent's it hasn't claimed yet.
fn unnamed_since(repo: &Repo, rec: &Record) -> Result<Vec<String>> {
    let all = repo.store.all()?;
    Ok(git::dirty()?
        .into_iter()
        .filter(|p| !rec.snapshot.contains(p) && Store::owner(&all, p, None).is_none())
        .collect())
}

fn list(header: &str, paths: &[String]) {
    if !paths.is_empty() {
        println!("{header}");
        for p in paths {
            println!("  {p}");
        }
    }
}

pub fn start(repo: &Repo, slug: &str, agent: &str) -> Result<i32> {
    if !store::valid_slug(slug) {
        bail!("slug: lowercase letters, digits, . _ - only, starting with a letter or digit");
    }
    if repo.store.exists(slug) {
        let mut rec = repo.store.load(slug)?;
        println!("resumed '{slug}' (opened {} by {})", rec.get("started").unwrap_or("?"), rec.agent());
        if let Some(h) = rec.get("hold") {
            println!("  hold: {h}");
        }
        for f in &rec.files {
            println!("  named: {f}");
        }
        if agent != "unknown" {
            rec.set("agent", agent);
        }
        rec.touch();
        rec.save()?;
    } else {
        let rec = repo.store.create(slug, agent, git::dirty()?)?;
        println!("opened '{slug}' ({agent}); {} path(s) already dirty are not yours", rec.snapshot.len());
    }
    Ok(0)
}

pub fn claim(repo: &Repo, slug: &str, files: &[String]) -> Result<i32> {
    let mut rec = repo.store.load(slug)?;
    claim_into(repo, &mut rec, &norm(files))?;
    rec.touch();
    rec.save()?;
    Ok(0)
}

pub fn commit(repo: &Repo, slug: &str, message: &[String], no_push: bool, files: &[String]) -> Result<i32> {
    let mut rec = repo.store.load(slug)?;
    let files = norm(files);
    claim_into(repo, &mut rec, &files)?;
    rec.touch();
    rec.save()?;

    // Stage exactly these paths (adds, edits, deletions), then commit only them:
    // whatever else is staged in the shared index stays staged and out of this commit.
    // A deletion already staged (`git rm`) is in neither the tree nor the index, so
    // `git add` can't match it; `git commit -- <path>` still takes it from HEAD.
    let mut to_add = Vec::new();
    for f in &files {
        if Path::new(f).symlink_metadata().is_ok() || git::ok(&["ls-files", "--error-unmatch", "--", f]) {
            to_add.push(f.clone());
        } else if !git::ok(&["cat-file", "-e", &format!("HEAD:{f}")]) {
            bail!("{f}: not in the working tree, the index or HEAD; nothing committed");
        }
    }
    if !to_add.is_empty() {
        let mut add = vec!["add".to_string(), "-A".into(), "--".into()];
        add.extend(to_add);
        if !git::run(&add)? {
            bail!("git add failed; nothing committed");
        }
    }
    let mut c = vec!["commit".to_string()];
    for m in message {
        c.push("-m".into());
        c.push(m.clone());
    }
    c.push("--".into());
    c.extend(files.iter().cloned());
    if !git::run(&c)? {
        bail!("commit refused (see above); nothing committed, the files stay named by '{slug}'");
    }

    let head = git::out(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
    if no_push {
        println!("committed {head} (not pushed: --no-push)");
    } else if git::has_upstream() {
        if !git::run(&["push", "--quiet"])? {
            eprintln!("work: push refused (the remote has commits this tree lacks). When no other agent is");
            eprintln!("      mid-edit: git pull --rebase --autostash && git push");
            return Ok(1);
        }
        println!("pushed {head}");
    } else {
        println!("committed {head} (branch has no upstream: not pushed)");
    }
    list(
        &format!(
            "dirty since '{slug}' started, named by no record (yours? commit or claim them; another agent's? leave them):"
        ),
        &unnamed_since(repo, &rec)?,
    );
    Ok(0)
}

pub fn hold(repo: &Repo, slug: &str, reason: &str) -> Result<i32> {
    let mut rec = repo.store.load(slug)?;
    rec.set("hold", reason);
    rec.touch();
    rec.save()?;
    println!("'{slug}' on hold: {reason}");
    Ok(0)
}

pub fn done(repo: &Repo, slug: &str) -> Result<i32> {
    let rec = repo.store.load(slug)?;
    let dirty = git::dirty()?;
    let left: Vec<_> = rec.files.iter().filter(|f| dirty.contains(f)).cloned().collect();
    if !left.is_empty() {
        bail!(
            "'{slug}' still has uncommitted files:\n{}\ncommit them, or `work hold {slug} \"reason\"` and keep the record open",
            left.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
        );
    }
    list(
        &format!("note: dirty since '{slug}' started and named by no record (check they aren't yours):"),
        &unnamed_since(repo, &rec)?,
    );
    rec.remove()?;
    println!("closed '{slug}'");
    Ok(0)
}

pub fn age(secs: i64) -> String {
    let s = secs.max(0);
    if s >= 86400 {
        format!("{}d{}h", s / 86400, s % 86400 / 3600)
    } else if s >= 3600 {
        format!("{}h{}m", s / 3600, s % 3600 / 60)
    } else {
        format!("{}m", s / 60)
    }
}

pub fn span(s: &str) -> Result<i64> {
    let (n, mult) = match s.chars().last() {
        Some('d') => (&s[..s.len() - 1], 86400),
        Some('h') => (&s[..s.len() - 1], 3600),
        Some('m') => (&s[..s.len() - 1], 60),
        Some('s') => (&s[..s.len() - 1], 1),
        _ => (s, 1),
    };
    Ok(n.parse::<i64>().map_err(|_| anyhow::anyhow!("bad time span {s:?} (use 30m, 2h, 1d)"))? * mult)
}

fn now_epoch() -> i64 {
    Local::now().timestamp()
}

/// Seconds since a path was last changed. A deleted file has no mtime of its
/// own; its directory's is the closest thing.
fn idle_secs(path: &str) -> i64 {
    let p = Path::new(path);
    let m = std::fs::metadata(p).or_else(|_| {
        std::fs::metadata(p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new(".")))
    });
    match m.and_then(|m| m.modified()) {
        Ok(t) => SystemTime::now().duration_since(t).map(|d| d.as_secs() as i64).unwrap_or(0),
        Err(_) => 0,
    }
}

fn rec_idle(rec: &Record) -> i64 {
    rec.get("updated").and_then(store::parse_time).map(|t| now_epoch() - t.timestamp()).unwrap_or(0)
}

pub fn status(repo: &Repo) -> Result<i32> {
    let recs = repo.store.all()?;
    let dirty = git::dirty()?;
    if recs.is_empty() {
        println!("no open records");
    }
    for r in &recs {
        let mine: Vec<_> = r.files.iter().filter(|f| dirty.contains(f)).collect();
        let hold = r.get("hold").map(|h| format!("  HOLD: {h}")).unwrap_or_default();
        println!("{:<32} {:<10} idle {:<7} {} dirty{hold}", r.slug, r.agent(), age(rec_idle(r)), mine.len());
        for f in mine {
            println!("    {f}");
        }
    }
    let orphans: Vec<_> = dirty.iter().filter(|p| Store::owner(&recs, p, None).is_none()).collect();
    if !orphans.is_empty() {
        println!("\ndirty, named by no record (age of last edit):");
        for p in orphans {
            println!("  {:<7} {p}", age(idle_secs(p)));
        }
        println!(
            "adopt: read each diff (for config, compare with what runs), then `work start adopt-<slug>` and commit"
        );
    }
    Ok(0)
}

/// Problems for the watcher: dirty files without an owner, untouched for
/// `orphan_age`; records without a hold, not updated for `idle`.
pub fn problems(repo: &Repo, orphan_age: i64, idle: i64) -> Result<Vec<String>> {
    let recs = repo.store.all()?;
    let dirty = git::dirty()?;
    let mut out = Vec::new();
    for r in &recs {
        let n = r.files.iter().filter(|f| dirty.contains(f)).count();
        let i = rec_idle(r);
        if r.get("hold").is_none() && i >= idle {
            out.push(format!("record '{}' ({}) idle {}, {n} dirty, no hold", r.slug, r.agent(), age(i)));
        }
    }
    let orphans: Vec<_> = dirty
        .iter()
        .filter(|p| Store::owner(&recs, p, None).is_none() && idle_secs(p) >= orphan_age)
        .cloned()
        .collect();
    if !orphans.is_empty() {
        let more = if orphans.len() > 3 { " …" } else { "" };
        out.push(format!(
            "{} dirty file(s) named by no record, untouched > {}: {}{more}",
            orphans.len(),
            age(orphan_age),
            orphans[..orphans.len().min(3)].join(" ")
        ));
    }
    Ok(out)
}

pub fn check(repo: &Repo, orphan_age: &str, idle: &str, notify: bool) -> Result<i32> {
    let found = problems(repo, span(orphan_age)?, span(idle)?)?;
    if found.is_empty() {
        return Ok(0);
    }
    for p in &found {
        println!("{p}");
    }
    if notify {
        let name = repo.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        watch::notify_throttled(
            &repo.store.dir.join(".notified"),
            &found,
            &format!("{} item(s) left behind in {name}: run work status", found.len()),
        )?;
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_and_ages() {
        assert_eq!(span("2h").unwrap(), 7200);
        assert_eq!(span("30m").unwrap(), 1800);
        assert_eq!(span("1d").unwrap(), 86400);
        assert!(span("x").is_err());
        assert_eq!(age(3 * 3600 + 12 * 60), "3h12m");
        assert_eq!(age(90000), "1d1h");
    }
}
