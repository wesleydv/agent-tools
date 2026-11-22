//! The watcher (`work watch`), its setup (`work setup`) and notifications.
//! Registered repos: one absolute path per line in
//! `$XDG_CONFIG_HOME/agent-tools/repos` (default `~/.config/agent-tools/repos`).

use crate::{config, git};
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

const LABEL: &str = "local.agent-tools.watch";
/// Same problems: notify again after this long.
const RENOTIFY_SECS: i64 = 4 * 3600;

pub fn config_dir() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(std::env::var_os("HOME").context("HOME not set")?).join(".config"),
    };
    Ok(base.join("agent-tools"))
}

fn repos_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("repos"))
}

pub fn registered() -> Result<Vec<PathBuf>> {
    let f = repos_file()?;
    if !f.exists() {
        return Ok(Vec::new());
    }
    Ok(fs::read_to_string(f)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(PathBuf::from)
        .collect())
}

/// Notify when the set of problems changes, or every 4 h while it stays.
pub fn notify_throttled(state: &Path, problems: &[String], msg: &str) -> Result<()> {
    let volatile = Regex::new(r"idle \S+|untouched > \S+").unwrap();
    let mut h = DefaultHasher::new();
    for p in problems {
        volatile.replace_all(p, "").hash(&mut h);
    }
    let sum = h.finish().to_string();
    let now = chrono::Local::now().timestamp();
    let last = fs::read_to_string(state).unwrap_or_default();
    let mut it = last.split_whitespace();
    let (last_sum, last_t) =
        (it.next().unwrap_or(""), it.next().and_then(|t| t.parse::<i64>().ok()).unwrap_or(0));
    if last_sum == sum && now - last_t < RENOTIFY_SECS {
        return Ok(());
    }
    if let Some(d) = state.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(state, format!("{sum} {now}\n"))?;
    notify("Uncommitted work", msg);
    Ok(())
}

fn notify(title: &str, msg: &str) {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    if cfg!(target_os = "macos") {
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"default\"",
            esc(msg),
            esc(title)
        );
        let _ = Command::new("osascript").args(["-e", &script]).status();
    } else {
        let _ = Command::new("notify-send").args([title, msg]).status();
    }
}

pub fn watch(notify: bool) -> Result<i32> {
    let exe = std::env::current_exe()?;
    let mut problems = Vec::new();
    let mut names = Vec::new();
    for repo in registered()? {
        let name = repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !repo.join(".git").exists() {
            problems.push(format!("{}: registered but not a git repo", repo.display()));
            continue;
        }
        let o = Command::new(&exe).arg("check").current_dir(&repo).output()?;
        match o.status.code() {
            Some(0) => {}
            Some(1) if o.stderr.is_empty() => {
                for l in String::from_utf8_lossy(&o.stdout).lines() {
                    problems.push(format!("{name}: {l}"));
                }
                names.push(name);
            }
            _ => {
                problems.push(format!("{name}: check failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
                names.push(name);
            }
        }
    }
    if problems.is_empty() {
        return Ok(0);
    }
    for p in &problems {
        println!("{p}");
    }
    if notify {
        let msg =
            format!("{} item(s) left behind in {}: run work status there", problems.len(), names.join(", "));
        notify_throttled(&config_dir()?.join("notified"), &problems, &msg)?;
    }
    Ok(1)
}

fn uid() -> Result<String> {
    Ok(String::from_utf8(Command::new("id").arg("-u").output()?.stdout)?.trim().to_string())
}

fn launchd_loaded() -> Result<bool> {
    Ok(Command::new("launchctl")
        .args(["print", &format!("gui/{}/{LABEL}", uid()?)])
        .output()?
        .status
        .success())
}

fn plist(exe: &Path) -> String {
    let dir = exe.parent().map(|d| d.display().to_string()).unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>watch</string><string>--notify</string></array>
  <key>EnvironmentVariables</key>
  <dict><key>PATH</key><string>{dir}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string></dict>
  <key>StartInterval</key><integer>3600</integer>
  <key>StandardOutPath</key><string>/tmp/{LABEL}.log</string>
  <key>StandardErrorPath</key><string>/tmp/{LABEL}.log</string>
</dict>
</plist>
"#,
        exe.display()
    )
}

/// The repo's hooks dir has a commit-msg that runs `work hook commit-msg`.
fn hook_installed(root: &Path) -> bool {
    let hp = git::out(&["config", "core.hooksPath"]).unwrap_or_default();
    let dir = if hp.trim().is_empty() { root.join(".git/hooks") } else { root.join(hp.trim()) };
    fs::read_to_string(dir.join("commit-msg")).is_ok_and(|t| t.contains("work hook commit-msg"))
}

pub fn setup(check_only: bool) -> Result<i32> {
    let mut fail = false;
    let mut report = |ok: bool, what: String| {
        println!("{}  {what}", if ok { "OK     " } else { "MISSING" });
        fail |= !ok;
    };
    let exe = fs::canonicalize(std::env::current_exe()?)?;
    report(true, format!("work {} ({})", config::VERSION, exe.display()));

    let root = git::out(&["rev-parse", "--show-toplevel"]).ok().map(|s| PathBuf::from(s.trim()));
    if let Some(root) = &root {
        config::Config::load(root)?; // fails when .work.toml requires a newer work
        let mut repos = registered()?;
        if repos.contains(root) {
            report(true, format!("{} registered with the watcher", root.display()));
        } else if check_only {
            report(false, format!("{} not registered (run work setup)", root.display()));
        } else {
            repos.push(root.clone());
            let f = repos_file()?;
            fs::create_dir_all(f.parent().unwrap())?;
            fs::write(&f, repos.iter().map(|r| format!("{}\n", r.display())).collect::<String>())?;
            report(true, format!("{} registered with the watcher", root.display()));
        }
        report(
            hook_installed(root),
            "commit-msg hook calls `work hook commit-msg` (see README, Adding a repo)".into(),
        );
    }

    if cfg!(target_os = "macos") {
        let home = PathBuf::from(std::env::var_os("HOME").context("HOME not set")?);
        let path = home.join(format!("Library/LaunchAgents/{LABEL}.plist"));
        let want = plist(&exe);
        let current = fs::read_to_string(&path).unwrap_or_default();
        let loaded = launchd_loaded()?;
        if current == want && loaded {
            report(true, format!("watcher {LABEL} loaded (hourly)"));
        } else if check_only {
            report(false, format!("watcher {LABEL} not loaded or out of date (run work setup)"));
        } else {
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(&path, want)?;
            let target = format!("gui/{}", uid()?);
            let _ = Command::new("launchctl").args(["bootout", &format!("{target}/{LABEL}")]).output();
            let ok = Command::new("launchctl").args(["bootstrap", &target]).arg(&path).status()?.success();
            if !ok {
                bail!("launchctl bootstrap {} failed", path.display());
            }
            report(true, format!("watcher {LABEL} loaded (hourly work watch --notify)"));
        }
    } else {
        println!("NOTE     no launchd here: run `{} watch --notify` hourly from cron", exe.display());
    }
    Ok(if fail { 1 } else { 0 })
}
