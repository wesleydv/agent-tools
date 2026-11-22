//! One timeline of commits and log entries, newest first. A commit's systems
//! come from its paths (`.work.toml` rules); a file in a meta dir (log/, todo/)
//! contributes the systems in its own front matter.

use crate::config::front_matter_systems;
use crate::{Repo, git};
use anyhow::Result;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::fs;

struct Row {
    date: String,
    kind: String,
    systems: Vec<String>,
    text: String,
}

fn push_unique(v: &mut Vec<String>, items: impl IntoIterator<Item = String>) {
    for i in items {
        if !v.contains(&i) {
            v.push(i);
        }
    }
}

fn meta_systems(repo: &Repo) -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    for d in &repo.cfg.meta_dirs {
        let Ok(rd) = fs::read_dir(repo.root.join(d)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".md") {
                continue;
            }
            if let Some(s) = fs::read_to_string(e.path()).ok().as_deref().and_then(front_matter_systems) {
                m.insert(format!("{d}/{name}"), s);
            }
        }
    }
    m
}

fn log_rows(
    repo: &Repo,
    meta: &HashMap<String, Vec<String>>,
    re: Option<&Regex>,
    since: Option<&str>,
) -> Vec<Row> {
    let dated = Regex::new(r"^\d{4}-\d{2}-\d{2}-.*\.md$").unwrap();
    let dir = &repo.cfg.log_dir;
    let Ok(rd) = fs::read_dir(repo.root.join(dir)) else { return Vec::new() };
    let mut rows = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !dated.is_match(&name) {
            continue;
        }
        let date = name[..10].to_string();
        if since.is_some_and(|s| date.as_str() < s) {
            continue;
        }
        let Ok(text) = fs::read_to_string(e.path()) else { continue };
        if re.is_some_and(|re| !re.is_match(&text)) {
            continue;
        }
        let title = text.lines().find_map(|l| l.strip_prefix("# ")).unwrap_or(&name).to_string();
        let path = format!("{dir}/{name}");
        rows.push(Row {
            date,
            kind: "log".into(),
            systems: meta.get(&path).cloned().unwrap_or_default(),
            text: format!("{title}  ({path})"),
        });
    }
    rows.sort_by(|a, b| b.text.cmp(&a.text));
    rows
}

fn commit_rows(
    repo: &Repo,
    meta: &HashMap<String, Vec<String>>,
    text: Option<&str>,
    since: Option<&str>,
    all: bool,
) -> Result<Vec<Row>> {
    let mut args = vec![
        "log".to_string(),
        "--format=%x1e%h%x1f%ad%x1f%s".into(),
        "--date=short".into(),
        "--name-only".into(),
    ];
    if let Some(s) = since {
        args.push(format!("--since={s}"));
    }
    if let Some(t) = text {
        args.extend(["-i".into(), "-E".into(), format!("--grep={t}")]);
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let raw = git::out(&refs)?;
    let log_prefix = format!("{}/", repo.cfg.log_dir);
    let mut rows = Vec::new();
    for chunk in raw.split('\x1e').filter(|c| !c.trim().is_empty()) {
        let mut lines = chunk.lines();
        let header = lines.next().unwrap_or("");
        let mut f = header.split('\x1f');
        let (hash, date, subject) = (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""));
        let paths: Vec<&str> = lines.filter(|l| !l.is_empty()).collect();
        if !all && !paths.is_empty() && paths.iter().all(|p| p.starts_with(&log_prefix)) {
            continue;
        }
        let mut systems = Vec::new();
        for p in &paths {
            if repo.cfg.in_meta_dir(p) {
                if let Some(s) = meta.get(*p) {
                    push_unique(&mut systems, s.iter().cloned());
                }
            } else if let Some(s) = repo.cfg.systems_for(p) {
                push_unique(&mut systems, s);
            } else {
                push_unique(&mut systems, [repo.cfg.default_system.clone()]);
            }
        }
        if systems.is_empty() {
            systems.push(repo.cfg.default_system.clone());
        }
        rows.push(Row { date: date.into(), kind: hash.into(), systems, text: subject.into() });
    }
    Ok(rows)
}

pub fn run(
    repo: &Repo,
    system: Option<&str>,
    text: Option<&str>,
    n: usize,
    since: Option<&str>,
    all: bool,
) -> Result<i32> {
    let re = text.map(|t| RegexBuilder::new(t).case_insensitive(true).build()).transpose()?;
    let meta = meta_systems(repo);
    // Commits first (git order is exact within a day), then that day's log entries.
    let mut rows = commit_rows(repo, &meta, text, since, all)?;
    rows.extend(log_rows(repo, &meta, re.as_ref(), since));
    rows.retain(|r| system.is_none_or(|s| r.systems.iter().any(|x| x == s)));
    rows.sort_by(|a, b| b.date.cmp(&a.date)); // stable: keeps the order above within a date
    for r in rows.iter().take(n) {
        let mut s = r.systems.join(",");
        if s.chars().count() > 26 {
            s = s.chars().take(25).collect::<String>() + "…";
        }
        println!("{}  {:<7}  {:<26}  {}", r.date, r.kind, s, r.text);
    }
    Ok(0)
}
