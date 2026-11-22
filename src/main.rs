mod config;
mod denylist;
mod git;
mod history;
mod hook;
mod store;
mod watch;
mod work;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Shared git workflow for repos that coding agents work in.
///
/// Approving a change approves its commit and push; `work` records which agent
/// owns which uncommitted files in a shared tree, commits exactly those, and
/// flags work left behind. See the README for the workflow.
#[derive(Parser)]
#[command(name = "work", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open a work record before your first edit (resumes an existing one)
    Start {
        slug: String,
        /// Who is working: claude, opencode, agy, a person's name, ...
        #[arg(long, env = "WORK_AGENT", default_value = "unknown")]
        agent: String,
    },
    /// Name files as yours before committing them
    Claim {
        slug: String,
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Commit exactly the files after `--`, then push when the branch has an upstream
    Commit {
        slug: String,
        /// Subject, then body paragraphs (repeat -m)
        #[arg(short = 'm', required = true)]
        message: Vec<String>,
        #[arg(long)]
        no_push: bool,
        #[arg(last = true, required = true)]
        files: Vec<String>,
    },
    /// Record a deliberate hold: the record's changes stay uncommitted
    Hold { slug: String, reason: String },
    /// Close a record (refused while its files are dirty)
    Done { slug: String },
    /// Open records, holds, and dirty files nobody owns
    Status,
    /// What's left behind; exit 1 if anything
    Check {
        /// Dirty files without an owner, untouched this long
        #[arg(long, default_value = "2h")]
        orphan_age: String,
        /// Records without a hold, not updated this long
        #[arg(long, default_value = "6h")]
        idle: String,
        #[arg(long)]
        notify: bool,
    },
    /// Commits and log entries as one timeline, newest first
    History {
        /// Only this system
        system: Option<String>,
        /// Regex (any case) in commit messages and log entries
        #[arg(short = 's')]
        text: Option<String>,
        #[arg(short = 'n', default_value_t = 40)]
        n: usize,
        /// YYYY-MM-DD
        #[arg(long)]
        since: Option<String>,
        /// Include commits that only touch the log folder
        #[arg(short = 'a')]
        all: bool,
    },
    /// Git hooks; call from the repo's own hook files
    Hook {
        #[command(subcommand)]
        hook: HookCmd,
    },
    /// `work check` in every registered repo, one notification (run by launchd)
    Watch {
        #[arg(long)]
        notify: bool,
    },
    /// Register this repo with the watcher and load it; --check only reports
    Setup {
        #[arg(long)]
        check: bool,
    },
}

#[derive(Subcommand)]
enum HookCmd {
    /// Message rules: a body or Log: trailer on deploying commits, no attribution trailers, the denylist
    CommitMsg { file: PathBuf },
    /// Refuse staged added lines and file names that match the local denylist
    PreCommit {
        /// Check every tracked file instead of the staged changes
        #[arg(long)]
        all: bool,
    },
}

pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub store: store::Store,
    pub cfg: config::Config,
}

impl Repo {
    fn open() -> Result<Repo> {
        let root =
            PathBuf::from(git::out(&["rev-parse", "--show-toplevel"]).context("not in a git repo")?.trim());
        std::env::set_current_dir(&root)?;
        let abs = |p: String| {
            let p = PathBuf::from(p.trim());
            if p.is_absolute() { p } else { root.join(p) }
        };
        let common = abs(git::out(&["rev-parse", "--git-common-dir"])?);
        let git_dir = abs(git::out(&["rev-parse", "--git-dir"])?);
        Ok(Repo {
            store: store::Store::new(common.join("work"))?,
            cfg: config::Config::load(&root)?,
            git_dir,
            root,
        })
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match run(cli.cmd) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("work: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run(cmd: Cmd) -> Result<i32> {
    match cmd {
        Cmd::Watch { notify } => return watch::watch(notify),
        Cmd::Setup { check } => return watch::setup(check),
        _ => {}
    }
    let repo = Repo::open()?;
    match cmd {
        Cmd::Start { slug, agent } => work::start(&repo, &slug, &agent),
        Cmd::Claim { slug, files } => work::claim(&repo, &slug, &files),
        Cmd::Commit { slug, message, no_push, files } => {
            work::commit(&repo, &slug, &message, no_push, &files)
        }
        Cmd::Hold { slug, reason } => work::hold(&repo, &slug, &reason),
        Cmd::Done { slug } => work::done(&repo, &slug),
        Cmd::Status => work::status(&repo),
        Cmd::Check { orphan_age, idle, notify } => work::check(&repo, &orphan_age, &idle, notify),
        Cmd::History { system, text, n, since, all } => {
            history::run(&repo, system.as_deref(), text.as_deref(), n, since.as_deref(), all)
        }
        Cmd::Hook { hook: HookCmd::CommitMsg { file } } => hook::commit_msg(&repo, &file),
        Cmd::Hook { hook: HookCmd::PreCommit { all } } => denylist::pre_commit(&repo, all),
        Cmd::Watch { .. } | Cmd::Setup { .. } => unreachable!(),
    }
}
