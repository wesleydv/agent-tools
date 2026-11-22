//! End-to-end: the real `work` binary against throwaway git repos.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_work");

struct Repo {
    _tmp: TempDir,
    dir: PathBuf,
    config_home: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> Output {
    // Never the real config home: a real denylist would change the tests.
    let o = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("XDG_CONFIG_HOME", "/nonexistent/agent-tools-tests")
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    o
}

fn write(dir: &Path, path: &str, text: &str) {
    let p = dir.join(path);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

impl Repo {
    /// A repo with one commit, the commit-msg and pre-commit hooks wired to this binary, and
    /// `.work.toml` as given.
    fn new(work_toml: &str) -> Repo {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("proj");
        let config_home = tmp.path().join("config");
        fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "T"),
            ("user.email", "t@example.com"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", ".githooks"),
        ] {
            git(&dir, &["config", k, v]);
        }
        write(&dir, ".githooks/commit-msg", &format!("#!/bin/sh\nexec \"{BIN}\" hook commit-msg \"$@\"\n"));
        write(&dir, ".githooks/pre-commit", &format!("#!/bin/sh\nexec \"{BIN}\" hook pre-commit\n"));
        Command::new("chmod")
            .args(["+x", ".githooks/commit-msg", ".githooks/pre-commit"])
            .current_dir(&dir)
            .status()
            .unwrap();
        write(&dir, ".work.toml", work_toml);
        for f in ["a", "b", "c"] {
            write(&dir, f, f);
        }
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-qm", "init", "--no-verify"]);
        Repo { _tmp: tmp, dir, config_home }
    }

    fn work(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .args(args)
            .current_dir(&self.dir)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env_remove("WORK_AGENT")
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let o = self.work(args);
        assert!(o.status.success(), "work {args:?} failed:\n{}\n{}", out(&o), err(&o));
        out(&o)
    }

    fn fails(&self, args: &[&str]) -> String {
        let o = self.work(args);
        assert!(!o.status.success(), "work {args:?} should fail:\n{}", out(&o));
        format!("{}{}", out(&o), err(&o))
    }

    fn write(&self, path: &str, text: &str) {
        write(&self.dir, path, text);
    }

    fn last_commit_files(&self) -> String {
        out(&git(&self.dir, &["show", "--name-only", "--format=", "HEAD"]))
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const DEPLOYS: &str = r#"deploy_paths = ['^stacks/']
"#;

#[test]
fn commit_takes_only_named_files_and_leaves_other_staging_alone() {
    let r = Repo::new("");
    r.write("a", "a2");
    r.ok(&["start", "one", "--agent", "claude"]); // a was dirty before: not one's
    r.write("b", "b2");
    r.write("new file.md", "x");
    fs::remove_file(r.dir.join("c")).unwrap();
    git(&r.dir, &["add", "a"]); // someone else's staged change
    let o = r.ok(&["commit", "one", "-m", "edit", "--", "b", "c", "new file.md"]);
    assert!(o.contains("no upstream: not pushed"), "{o}");
    let files = r.last_commit_files();
    assert!(files.contains("b") && files.contains("c") && files.contains("new file.md"), "{files}");
    assert!(!files.lines().any(|l| l == "a"), "{files}");
    assert_eq!(out(&git(&r.dir, &["diff", "--cached", "--name-only"])).trim(), "a");
}

#[test]
fn commit_takes_deletions_already_staged_with_git_rm() {
    let r = Repo::new("");
    r.ok(&["start", "s"]);
    git(&r.dir, &["rm", "-q", "b"]);
    r.write("a", "a2");
    r.ok(&["commit", "s", "-m", "drop b", "--", "a", "b"]);
    let files = r.last_commit_files();
    assert!(files.lines().any(|l| l == "b") && files.lines().any(|l| l == "a"), "{files}");
    assert!(r.fails(&["commit", "s", "-m", "x", "--", "never-existed"]).contains("not in the working tree"));
}

#[test]
fn claimed_files_are_refused_to_other_records() {
    let r = Repo::new("");
    r.ok(&["start", "one"]);
    r.ok(&["start", "two", "--agent", "opencode"]);
    r.write("b", "b2");
    r.ok(&["claim", "two", "b"]);
    let e = r.fails(&["commit", "one", "-m", "x", "--", "b"]);
    assert!(e.contains("already named by record 'two' (opencode)"), "{e}");
}

#[test]
fn done_waits_for_dirty_files_and_hold_is_recorded() {
    let r = Repo::new("");
    r.ok(&["start", "one"]);
    r.write("b", "b2");
    r.ok(&["claim", "one", "b"]);
    let e = r.fails(&["done", "one"]);
    assert!(e.contains("still has uncommitted files") && e.contains("  b"), "{e}");
    r.ok(&["hold", "one", "waiting for the owner"]);
    let s = r.ok(&["status"]);
    assert!(s.contains("HOLD: waiting for the owner") && s.contains("    b"), "{s}");
    r.ok(&["commit", "one", "-m", "b", "--", "b"]);
    assert!(r.ok(&["done", "one"]).contains("closed 'one'"));
    assert!(r.ok(&["status"]).contains("no open records"));
}

#[test]
fn commit_lists_unnamed_files_dirty_since_start() {
    let r = Repo::new("");
    r.ok(&["start", "one"]);
    r.write("b", "b2");
    r.write("generated.txt", "g");
    let o = r.ok(&["commit", "one", "-m", "b", "--", "b"]);
    assert!(o.contains("named by no record") && o.contains("  generated.txt"), "{o}");
}

#[test]
fn bad_slug_and_missing_record() {
    let r = Repo::new("");
    assert!(r.fails(&["start", "Bad/Slug"]).contains("slug"));
    assert!(r.fails(&["hold", "nope", "x"]).contains("no record 'nope'"));
}

#[test]
fn deploying_commit_needs_body_or_log_trailer() {
    let r = Repo::new(DEPLOYS);
    r.ok(&["start", "s"]);
    r.write("stacks/x/compose.yml", "v: 1");
    let e = r.fails(&["commit", "s", "-m", "x: bump", "--", "stacks/x/compose.yml"]);
    assert!(e.contains("deploys (stacks/x/compose.yml) but has no body"), "{e}");
    r.ok(&["commit", "s", "-m", "x: bump", "-m", "Because it broke.", "--", "stacks/x/compose.yml"]);

    r.write("stacks/x/compose.yml", "v: 2");
    let e = r.fails(&["commit", "s", "-m", "x: v2", "-m", "Log: log/nope.md", "--", "stacks/x/compose.yml"]);
    assert!(e.contains("Log: trailer names log/nope.md"), "{e}");
    r.write("log/2026-11-22-x.md", "---\nsystems: [x]\n---\n# X\n");
    r.ok(&[
        "commit",
        "s",
        "-m",
        "x: v2",
        "-m",
        "Log: log/2026-11-22-x.md",
        "--",
        "stacks/x/compose.yml",
        "log/2026-11-22-x.md",
    ]);

    // Non-deploying paths may be one line.
    r.write("a", "a2");
    r.ok(&["commit", "s", "-m", "docs", "--", "a"]);
}

#[test]
fn attribution_trailers_are_refused() {
    let r = Repo::new("");
    r.ok(&["start", "s"]);
    r.write("a", "a2");
    let e = r.fails(&["commit", "s", "-m", "x", "-m", "Co-Authored-By: Bot <b@example.com>", "--", "a"]);
    assert!(e.contains("no attribution trailers"), "{e}");
    let e = r.fails(&[
        "commit",
        "s",
        "-m",
        "x",
        "-m",
        "🤖 Generated with [Claude Code](https://claude.com)",
        "--",
        "a",
    ]);
    assert!(e.contains("no attribution trailers"), "{e}");
}

#[test]
fn pushes_when_the_branch_has_an_upstream() {
    let r = Repo::new("");
    let bare = r.dir.parent().unwrap().join("remote.git");
    git(r.dir.parent().unwrap(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(&r.dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git(&r.dir, &["push", "-q", "-u", "origin", "main"]);
    r.ok(&["start", "s"]);
    r.write("a", "a2");
    assert!(r.ok(&["commit", "s", "-m", "a", "--", "a"]).contains("pushed"));
    let remote = out(&git(&bare, &["log", "-1", "--format=%s", "main"]));
    assert_eq!(remote.trim(), "a");
    r.write("b", "b2");
    assert!(r.ok(&["commit", "s", "-m", "b", "--no-push", "--", "b"]).contains("not pushed: --no-push"));
}

#[test]
fn check_reports_orphans_and_idle_records() {
    let r = Repo::new("");
    assert_eq!(r.work(&["check"]).status.code(), Some(0));
    r.write("a", "a2");
    let o = r.work(&["check", "--orphan-age", "0m"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(out(&o).contains("1 dirty file(s) named by no record"), "{}", out(&o));
    r.ok(&["start", "s"]);
    r.ok(&["claim", "s", "a"]);
    let o = r.work(&["check", "--orphan-age", "0m", "--idle", "0m"]);
    assert!(out(&o).contains("record 's' (unknown) idle"), "{}", out(&o));
    r.ok(&["hold", "s", "later"]);
    assert_eq!(r.work(&["check", "--orphan-age", "0m", "--idle", "0m"]).status.code(), Some(0));
}

#[test]
fn watch_covers_every_registered_repo() {
    let r = Repo::new("");
    fs::create_dir_all(r.config_home.join("agent-tools")).unwrap();
    fs::write(r.config_home.join("agent-tools/repos"), format!("{}\n/nonexistent/repo\n", r.dir.display()))
        .unwrap();
    assert_eq!(r.work(&["watch"]).status.code(), Some(1)); // the missing repo
    let o = r.work(&["watch"]);
    assert!(out(&o).contains("/nonexistent/repo: registered but not a git repo"), "{}", out(&o));
}

#[test]
fn history_maps_paths_to_systems() {
    let r = Repo::new(
        r#"
[[systems]]
match = '^hosts/([^/]+)/'
systems = ["$1"]
[[systems]]
match = '^areas/devices/([^/.]+)\.yaml$'
systems = ["devices", "$1"]
"#,
    );
    r.ok(&["start", "h"]);
    r.write("hosts/host-a/x", "1");
    r.ok(&["commit", "h", "-m", "host-a: x", "--", "hosts/host-a/x"]);
    r.write("areas/devices/sensor-1.yaml", "1");
    r.ok(&["commit", "h", "-m", "dev: sensor", "--", "areas/devices/sensor-1.yaml"]);
    r.write("log/2026-11-20-thing.md", "---\nsystems: [host-b, host-a]\n---\n# A thing happened\n");
    r.ok(&["commit", "h", "-m", "log: thing", "--", "log/2026-11-20-thing.md"]);

    let all = r.ok(&["history"]);
    assert!(
        all.contains("host-a: x") && all.contains("dev: sensor") && all.contains("A thing happened"),
        "{all}"
    );
    assert!(!all.contains("log: thing"), "log-only commits are hidden without -a:\n{all}");
    assert!(r.ok(&["history", "-a"]).contains("log: thing"));

    let host_a = r.ok(&["history", "host-a"]);
    assert!(
        host_a.contains("host-a: x")
            && host_a.contains("A thing happened")
            && !host_a.contains("dev: sensor"),
        "{host_a}"
    );
    let sensor = r.ok(&["history", "sensor-1"]);
    assert!(sensor.contains("dev: sensor") && !sensor.contains("host-a: x"), "{sensor}");
    assert!(r.ok(&["history", "-s", "THING HAP"]).contains("A thing happened"));
    assert!(r.ok(&["history", "repo"]).contains("init"));
}

#[test]
fn repo_can_require_a_newer_work() {
    let r = Repo::new("");
    r.write(".work.toml", "requires = \"99.0.0\"\n");
    let e = r.fails(&["status"]);
    assert!(e.contains("requires work >= 99.0.0"), "{e}");
    // The hook fails closed too: no commits under a too-old work.
    r.write("a", "a2");
    let o = Command::new("git")
        .args(["commit", "-qam", "x"])
        .current_dir(&r.dir)
        .env("XDG_CONFIG_HOME", &r.config_home)
        .output()
        .unwrap();
    assert!(!o.status.success());
}

#[test]
fn bad_config_is_reported() {
    let r = Repo::new("");
    r.write(".work.toml", "deploy_pathz = []\n");
    assert!(r.fails(&["status"]).contains(".work.toml"));
}

/// A denylist in the test's config home. The words are made up.
const DENY: &str = "# made-up words\nsecretw[o0]rd\n\nbad-host\n";

impl Repo {
    /// Write a list outside the repo and name it in `.work.toml`.
    fn deny(&self, list: &str) {
        write(&self.config_home, "agent-tools/denylist-public", list);
        let p = self.config_home.join("agent-tools/denylist-public");
        self.write(".work.toml", &format!("denylist = {:?}\n", p.to_str().unwrap()));
    }
}

/// Neither the pattern nor the text it matched may appear in the output.
fn assert_private(e: &str) {
    for w in ["secretw", "SecretWord", "bad-host", "Bad-Host"] {
        assert!(!e.contains(w), "output reveals {w:?}:\n{e}");
    }
}

#[test]
fn denylist_is_opt_in_per_repo() {
    // A list on the machine applies only where `.work.toml` names it: what must
    // not leak depends on the repo it would leak into.
    let r = Repo::new("");
    write(&r.config_home, "agent-tools/denylist", DENY);
    write(&r.config_home, "agent-tools/denylist-public", DENY);
    r.ok(&["start", "s"]);
    r.write("a", "secretword");
    r.ok(&["commit", "s", "-m", "about bad-host", "--", "a"]);
    assert!(r.fails(&["hook", "pre-commit", "--all"]).contains("no denylist configured for this repo"));
}

#[test]
fn denylist_refuses_added_lines_names_and_messages() {
    let r = Repo::new("");
    r.write("old.txt", "keep\nan old SECRETWORD line\n");
    git(&r.dir, &["add", "old.txt"]);
    git(&r.dir, &["commit", "-qm", "before the list"]);
    r.deny(DENY);
    r.ok(&["start", "s"]);
    r.ok(&["commit", "s", "-m", "use the list", "--", ".work.toml"]);

    // An added line, any case; the report names the file, line and list line only.
    r.write("x.txt", "fine\nhello SecretW0rd here\n");
    let e = r.fails(&["commit", "s", "-m", "x", "--", "x.txt"]);
    assert!(e.contains("x.txt:2 matches line 2 of"), "{e}");
    assert!(e.contains("commit refused"), "{e}");
    assert_private(&e);

    // A file name.
    r.write("notes/bad-host.md", "fine\n");
    let e = r.fails(&["commit", "s", "-m", "n", "--", "notes/bad-host.md"]);
    assert!(e.contains("file name notes/") && e.contains("matches line 4 of"), "{e}");
    git(&r.dir, &["rm", "-q", "--cached", "notes/bad-host.md"]);

    // The message, body included.
    r.write("x.txt", "fine\n");
    let e = r.fails(&["commit", "s", "-m", "x", "-m", "about Bad-Host", "--", "x.txt"]);
    assert!(e.contains("commit message line 3 matches line 4 of"), "{e}");
    assert_private(&e);

    // Removing a matching line is fine; --all still finds what is tracked.
    r.write("old.txt", "keep\n");
    r.ok(&["commit", "s", "-m", "x and old", "--", "x.txt", "old.txt"]);
    assert!(r.ok(&["hook", "pre-commit", "--all"]).contains("every tracked file passes"));
    r.write("old.txt", "keep\nsecretword\n");
    git(&r.dir, &["add", "old.txt"]);
    git(&r.dir, &["commit", "-qm", "sneak", "--no-verify"]);
    let e = r.fails(&["hook", "pre-commit", "--all"]);
    assert!(e.contains("old.txt:2 matches line 2 of"), "{e}");
    assert_private(&e);
}

#[test]
fn denylist_per_repo_file_and_bad_entries() {
    let r = Repo::new("");
    r.write(".work.toml", "denylist = \"private/words\"\n");
    let e = r.fails(&["hook", "pre-commit"]);
    assert!(e.contains("denylist") && e.contains("does not exist"), "{e}");
    r.write("private/words", "bad-host\n");
    r.ok(&["start", "s"]);
    r.write("a", "bad-host");
    // Inside the repo and not ignored: refused, it would be committed.
    let e = r.fails(&["commit", "s", "-m", "a", "--", "a"]);
    assert!(e.contains("inside the repo and tracked or not gitignored"), "{e}");
    r.write(".git/info/exclude", "private/\n");
    let e = r.fails(&["commit", "s", "-m", "a", "--", "a"]);
    assert!(e.contains("a:1 matches line 1 of"), "{e}");

    // A bad regex is reported by place, without quoting it.
    r.deny("ok\nbad-(host\n");
    git(&r.dir, &["commit", "-qam", "list", "--no-verify"]);
    let e = r.fails(&["hook", "pre-commit", "--all"]);
    assert!(e.contains("line 2 of") && e.contains("not a valid regex"), "{e}");
    assert_private(&e);
}
