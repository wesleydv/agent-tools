//! `.work.toml` at the repo root: what deploys, how paths map to systems, and
//! the oldest `work` that may run here. Every key is optional.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Raw {
    requires: Option<String>,
    deploy_paths: Vec<String>,
    default_system: String,
    log_dir: String,
    meta_dirs: Vec<String>,
    systems: Vec<RawRule>,
    denylist: Option<String>,
}

impl Default for Raw {
    fn default() -> Self {
        Raw {
            requires: None,
            deploy_paths: Vec::new(),
            default_system: "repo".into(),
            log_dir: "log".into(),
            meta_dirs: vec!["log".into(), "todo".into()],
            systems: Vec::new(),
            denylist: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    #[serde(rename = "match")]
    pattern: String,
    systems: Vec<String>,
}

pub struct Rule {
    re: Regex,
    systems: Vec<String>,
}

pub struct Config {
    /// Paths whose commits deploy something: the commit-msg hook wants a body.
    pub deploy: Vec<Regex>,
    /// Path -> systems; the first matching rule wins, `$1` expands captures.
    pub rules: Vec<Rule>,
    pub default_system: String,
    /// Dated `YYYY-MM-DD-slug.md` event entries.
    pub log_dir: String,
    /// Folders whose files carry `systems: [...]` front matter of their own.
    pub meta_dirs: Vec<String>,
    /// A per-repo denylist file, outside the repo or gitignored (`denylist.rs`).
    pub denylist: Option<String>,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

impl Config {
    pub fn load(root: &Path) -> Result<Config> {
        let path = root.join(".work.toml");
        let raw: Raw = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?
        } else {
            Raw::default()
        };
        if let Some(req) = &raw.requires
            && version_lt(VERSION, req)?
        {
            bail!(
                "this repo requires work >= {req}, installed is {VERSION}. Update: \
                 cd ~/code/agent-tools && git pull && cargo install --path . --root ~/.local"
            );
        }
        let re = |p: &String| Regex::new(p).with_context(|| format!(".work.toml: bad regex {p:?}"));
        Ok(Config {
            deploy: raw.deploy_paths.iter().map(re).collect::<Result<_>>()?,
            rules: raw
                .systems
                .iter()
                .map(|r| Ok(Rule { re: re(&r.pattern)?, systems: r.systems.clone() }))
                .collect::<Result<_>>()?,
            default_system: raw.default_system,
            log_dir: raw.log_dir,
            meta_dirs: raw.meta_dirs,
            denylist: raw.denylist,
        })
    }

    pub fn deploys(&self, path: &str) -> bool {
        self.deploy.iter().any(|re| re.is_match(path))
    }

    /// Systems for a path from the rules; None when no rule matches.
    pub fn systems_for(&self, path: &str) -> Option<Vec<String>> {
        let rule = self.rules.iter().find(|r| r.re.is_match(path))?;
        let caps = rule.re.captures(path)?;
        Some(
            rule.systems
                .iter()
                .map(|t| {
                    let mut s = String::new();
                    caps.expand(t, &mut s);
                    s
                })
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    pub fn in_meta_dir(&self, path: &str) -> bool {
        self.meta_dirs.iter().any(|d| path.starts_with(&format!("{d}/")))
    }
}

fn parse_version(v: &str) -> Result<Vec<u64>> {
    v.trim().split('.').map(|p| p.parse().with_context(|| format!("bad version {v:?}"))).collect()
}

pub fn version_lt(a: &str, b: &str) -> Result<bool> {
    Ok(parse_version(a)? < parse_version(b)?)
}

/// `systems: [a, b]` (or `systems: a, b`) from a file's front matter.
pub fn front_matter_systems(text: &str) -> Option<Vec<String>> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("systems:") {
            let v = v.split('#').next().unwrap_or("").trim().trim_start_matches('[');
            let v = v.split(']').next().unwrap_or("");
            return Some(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(version_lt("0.1.0", "0.2.0").unwrap());
        assert!(version_lt("0.9.0", "0.10.0").unwrap());
        assert!(!version_lt("1.0.0", "1.0.0").unwrap());
    }

    #[test]
    fn front_matter() {
        let t = "---\ndate: x\nsystems: [host-a, sensor-1]   # note\n---\n# T\nsystems: [no]\n";
        assert_eq!(front_matter_systems(t).unwrap(), vec!["host-a", "sensor-1"]);
        assert_eq!(front_matter_systems("# no front matter"), None);
    }
}
