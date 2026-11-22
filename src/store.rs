//! Work records: `<git-common-dir>/work/<slug>/` with `meta` (key=value),
//! `snapshot` (paths already dirty at start: someone else's) and `files`
//! (paths this record has named). Local, never committed, shared by worktrees.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset, Local};
use std::fs;
use std::path::PathBuf;

pub struct Store {
    pub dir: PathBuf,
}

pub struct Record {
    pub slug: String,
    dir: PathBuf,
    meta: Vec<(String, String)>,
    pub files: Vec<String>,
    pub snapshot: Vec<String>,
}

fn read_lines(p: &PathBuf) -> Result<Vec<String>> {
    if !p.exists() {
        return Ok(Vec::new());
    }
    Ok(fs::read_to_string(p)?.lines().filter(|l| !l.is_empty()).map(String::from).collect())
}

fn write_atomic(p: &PathBuf, text: &str) -> Result<()> {
    let tmp = p.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, p).with_context(|| format!("writing {}", p.display()))
}

pub fn valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(c))
}

pub fn now() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn parse_time(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(s).ok().or_else(|| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z").ok())
}

impl Store {
    pub fn new(dir: PathBuf) -> Result<Store> {
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Store { dir })
    }

    pub fn exists(&self, slug: &str) -> bool {
        self.dir.join(slug).is_dir()
    }

    pub fn load(&self, slug: &str) -> Result<Record> {
        let dir = self.dir.join(slug);
        if !dir.is_dir() {
            bail!("no record '{slug}' (work start {slug})");
        }
        let meta = read_lines(&dir.join("meta"))?
            .into_iter()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect();
        Ok(Record {
            slug: slug.to_string(),
            files: read_lines(&dir.join("files"))?,
            snapshot: read_lines(&dir.join("snapshot"))?,
            meta,
            dir,
        })
    }

    pub fn all(&self) -> Result<Vec<Record>> {
        let mut slugs: Vec<String> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect();
        slugs.sort();
        slugs.iter().map(|s| self.load(s)).collect()
    }

    pub fn create(&self, slug: &str, agent: &str, snapshot: Vec<String>) -> Result<Record> {
        let dir = self.dir.join(slug);
        fs::create_dir_all(&dir)?;
        let t = now();
        let rec = Record {
            slug: slug.to_string(),
            meta: vec![("agent".into(), agent.into()), ("started".into(), t.clone()), ("updated".into(), t)],
            files: Vec::new(),
            snapshot,
            dir,
        };
        write_atomic(&rec.dir.join("snapshot"), &lines(&rec.snapshot))?;
        rec.save()?;
        Ok(rec)
    }

    /// The open record that has named `path`, other than `except`.
    pub fn owner<'a>(records: &'a [Record], path: &str, except: Option<&str>) -> Option<&'a Record> {
        records.iter().find(|r| Some(r.slug.as_str()) != except && r.files.iter().any(|f| f == path))
    }
}

fn lines(v: &[String]) -> String {
    v.iter().map(|l| format!("{l}\n")).collect()
}

impl Record {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.meta.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.meta.retain(|(k, _)| k != key);
        self.meta.push((key.to_string(), value.to_string()));
    }

    pub fn touch(&mut self) {
        self.set("updated", &now());
    }

    pub fn agent(&self) -> &str {
        self.get("agent").unwrap_or("unknown")
    }

    pub fn save(&self) -> Result<()> {
        let meta: String = self.meta.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        write_atomic(&self.dir.join("meta"), &meta)?;
        write_atomic(&self.dir.join("files"), &lines(&self.files))
    }

    pub fn remove(self) -> Result<()> {
        fs::remove_dir_all(&self.dir).with_context(|| format!("removing {}", self.dir.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert!(valid_slug("adopt-orphans-2026-11-22"));
        assert!(valid_slug("b1.x_y"));
        assert!(!valid_slug("Upper"));
        assert!(!valid_slug("-lead"));
        assert!(!valid_slug("a/b"));
        assert!(!valid_slug(""));
    }

    #[test]
    fn times() {
        assert!(parse_time(&now()).is_some());
        assert!(parse_time("2026-11-22T11:54:34+0100").is_some());
    }
}
