//! What the status bar says about the focused pane's repository beyond its
//! branch (the design's 1d): commits not pushed yet, and the branch's pull
//! request with its checks. `git` and `gh` run on a thread of their own,
//! the newest question first; the bar shows what was last found.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long what was found stands before it is asked again.
const FRESH: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Git {
    /// Commits on the branch its upstream does not have, and the other way.
    /// `None` without an upstream.
    pub ahead: Option<(u32, u32)>,
    pub pr: Option<Pr>,
    /// What is changed and not committed (the cheap watcher's).
    pub changes: Option<Changes>,
}

/// The working tree against its last commit: files changed or new, and the
/// lines added and removed in those git knows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// `git status --porcelain` lines: two letters, then the path.
    pub files: Vec<String>,
    pub added: u32,
    pub removed: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pr {
    pub number: u32,
    /// `OPEN`, `MERGED`, `CLOSED`.
    pub state: String,
    pub url: String,
    pub checks: Checks,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Checks {
    None,
    Pass,
    Running,
    Fail,
}

type Key = (PathBuf, String);

pub struct Watcher {
    /// Commits ahead and the pull request (one folder, the focused), or the
    /// changes not committed (every tab's).
    full: bool,
    ask: Sender<Key>,
    found: Arc<Mutex<HashMap<Key, (Instant, Git)>>>,
    asked: Mutex<HashSet<Key>>,
}

impl Watcher {
    /// `wake` is called when something new is found.
    pub fn new(full: bool, wake: impl Fn() + Send + 'static) -> Self {
        let (ask, questions) = channel();
        let found = Arc::new(Mutex::new(HashMap::new()));
        let store = found.clone();
        let _ = std::thread::Builder::new().name("git-info".into()).spawn(move || work(full, &questions, &store, &wake));
        Self { full, ask, found, asked: Mutex::new(HashSet::new()) }
    }

    /// What is known of `cwd` on `branch`, without asking: the cards show a
    /// pull request found while their pane had the keys.
    pub fn known(&self, cwd: &Path, branch: &str) -> Option<Git> {
        let key = (cwd.to_path_buf(), branch.to_owned());
        self.found.lock().ok()?.get(&key).map(|(_, g)| g.clone())
    }

    /// What is known of `cwd` on `branch`, asking again when it is old.
    pub fn get(&self, cwd: &Path, branch: &str) -> Option<Git> {
        if branch.is_empty() {
            return None;
        }
        let key = (cwd.to_path_buf(), branch.to_owned());
        let known = self.found.lock().ok()?.get(&key).cloned();
        // The changes are cheap and change often; the pull request is neither.
        let fresh = if self.full { FRESH } else { Duration::from_secs(10) };
        let stale = known.as_ref().is_none_or(|(at, _)| at.elapsed() > fresh);
        let mut asked = self.asked.lock().ok()?;
        if stale && !asked.contains(&key) {
            asked.insert(key.clone());
            let _ = self.ask.send(key.clone());
        } else if !stale {
            asked.remove(&key);
        }
        known.map(|(_, g)| g)
    }
}

fn work(full: bool, questions: &Receiver<Key>, store: &Mutex<HashMap<Key, (Instant, Git)>>, wake: &dyn Fn()) {
    let mut gh = true;
    while let Ok(first) = questions.recv() {
        let mut keys = vec![first];
        while let Ok(more) = questions.try_recv() {
            keys.push(more);
        }
        if full {
            // Only the newest matters: the pane with the keys now.
            keys.drain(..keys.len() - 1);
        } else {
            keys.dedup();
            for key in keys {
                let changes = changes(&key.0);
                if let Ok(mut s) = store.lock() {
                    s.insert(key, (Instant::now(), Git { changes, ..Git::default() }));
                }
            }
            wake();
            continue;
        }
        let key = keys.pop().expect("one");
        let (cwd, _) = &key;
        let ahead = run(Command::new("git").args(["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]).current_dir(cwd)).and_then(|o| counts(&o));
        let pr = if gh {
            let out = run(Command::new("gh").args(["pr", "view", "--json", "number,state,url,statusCheckRollup", "--jq", JQ]).current_dir(cwd));
            if out.is_none() && !exists("gh") {
                gh = false;
            }
            out.and_then(|o| pr(&o))
        } else {
            None
        };
        if let Ok(mut s) = store.lock() {
            s.insert(key, (Instant::now(), Git { ahead, pr, changes: None }));
        }
        wake();
    }
}

/// The pull request as one tab-separated line: number, state, url, then
/// each check's result (a check run's conclusion, else its status; a
/// commit status's state), comma-separated.
const JQ: &str = r#"[.number, .state, .url, ([.statusCheckRollup[] | (if (.conclusion // "") != "" then .conclusion else (.state // .status // "") end)] | join(","))] | @tsv"#;

/// What a command printed, when it ran and said yes.
fn run(cmd: &mut Command) -> Option<String> {
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up for it.
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The changes not committed in the repository `cwd` is in.
fn changes(cwd: &Path) -> Option<Changes> {
    let status = run(Command::new("git").args(["status", "--porcelain=v1", "--untracked-files=normal"]).current_dir(cwd))?;
    let numstat = run(Command::new("git").args(["diff", "--numstat", "HEAD"]).current_dir(cwd)).unwrap_or_default();
    let (added, removed) = sum_numstat(&numstat);
    Some(Changes { files: status.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect(), added, removed })
}

/// The most of a diff that is shown (a lock file's can be megabytes).
const DIFF_MOST: usize = 512 * 1024;

/// What is changed and not committed, as `git diff HEAD` says it, then the
/// files git does not know yet (a thread's work: git can take a moment).
pub fn diff(cwd: &Path) -> Result<String, String> {
    let mut text = run(Command::new("git").args(["-c", "core.quotepath=off", "diff", "HEAD", "--no-color", "--no-ext-diff"]).current_dir(cwd)).ok_or_else(|| format!("{} is not in a git repository with a commit", cwd.display()))?;
    if text.len() > DIFF_MOST {
        let mut cut = DIFF_MOST;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n… (cut here: the diff is larger than 512 KB)\n");
    }
    let status = run(Command::new("git").args(["-c", "core.quotepath=off", "status", "--porcelain=v1", "--untracked-files=normal"]).current_dir(cwd)).unwrap_or_default();
    let new: Vec<&str> = status.lines().filter_map(|l| l.strip_prefix("?? ")).collect();
    if !new.is_empty() {
        text.push_str(&format!("new files, not added to git yet ({})\n", new.len()));
        for f in new {
            text.push_str(&format!("?? {f}\n"));
        }
    }
    Ok(text)
}

/// Push the branch `cwd` is on and open a pull request for it with `gh`,
/// titled and described from its commits (a thread's work): its address.
pub fn create_pr(cwd: &Path) -> Result<String, String> {
    let said = |cmd: &mut Command| -> Result<String, String> {
        cmd.current_dir(cwd).stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let out = cmd.output().map_err(|e| format!("{e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    };
    let branch = said(Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"])).map_err(|e| format!("not a git repository: {e}"))?;
    if branch == "HEAD" {
        return Err("no branch checked out here (a detached HEAD)".into());
    }
    said(Command::new("git").args(["push", "-u", "origin", "HEAD"])).map_err(|e| format!("git push: {e}"))?;
    let out = said(Command::new("gh").args(["pr", "create", "--fill"])).map_err(|e| if e.is_empty() { "gh did not run: is the GitHub CLI installed and signed in?".to_owned() } else { format!("gh pr create: {e}") })?;
    out.lines().rev().find(|l| l.starts_with("https://")).map(str::to_owned).ok_or_else(|| format!("gh said: {out}"))
}

/// `git diff --numstat`: added, removed and path a line; `-` for binary.
fn sum_numstat(out: &str) -> (u32, u32) {
    out.lines().fold((0, 0), |(a, r), l| {
        let mut f = l.split('\t');
        let n = |s: Option<&str>| s.and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        (a + n(f.next()), r + n(f.next()))
    })
}

fn exists(program: &str) -> bool {
    run(Command::new(program).arg("--version")).is_some()
}

/// `git rev-list --left-right --count @{upstream}...HEAD`: behind, then
/// ahead; given back as (ahead, behind).
fn counts(out: &str) -> Option<(u32, u32)> {
    let mut it = out.split_whitespace().map(|n| n.parse::<u32>().ok());
    let behind = it.next()??;
    let ahead = it.next()??;
    Some((ahead, behind))
}

fn pr(out: &str) -> Option<Pr> {
    let line = out.lines().next()?;
    let mut f = line.split('\t');
    let number = f.next()?.parse().ok()?;
    let state = f.next()?.to_owned();
    let url = f.next()?.to_owned();
    let results: Vec<&str> = f.next().unwrap_or("").split(',').filter(|s| !s.is_empty()).collect();
    let fail = ["FAILURE", "ERROR", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"];
    let done = ["SUCCESS", "NEUTRAL", "SKIPPED"];
    let checks = if results.is_empty() {
        Checks::None
    } else if results.iter().any(|r| fail.contains(r)) {
        Checks::Fail
    } else if results.iter().all(|r| done.contains(r)) {
        Checks::Pass
    } else {
        Checks::Running
    };
    Some(Pr { number, state, url, checks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_changed_lines_are_added_up() {
        assert_eq!(sum_numstat("10\t2\tsrc/a.rs\n-\t-\timg.png\n3\t0\tb.md\n"), (13, 2));
        assert_eq!(sum_numstat(""), (0, 0));
    }

    #[test]
    fn the_counts_and_the_pull_request_are_read() {
        assert_eq!(counts("1\t3\n"), Some((3, 1)));
        assert_eq!(counts(""), None);
        let p = pr("42\tOPEN\thttps://github.com/u/r/pull/42\tSUCCESS,IN_PROGRESS,SKIPPED\n").unwrap();
        assert_eq!((p.number, p.state.as_str(), p.checks), (42, "OPEN", Checks::Running));
        assert_eq!(pr("7\tMERGED\tu\tSUCCESS,NEUTRAL").unwrap().checks, Checks::Pass);
        assert_eq!(pr("7\tOPEN\tu\tSUCCESS,FAILURE,PENDING").unwrap().checks, Checks::Fail);
        assert_eq!(pr("7\tOPEN\tu\t").unwrap().checks, Checks::None);
        assert_eq!(pr("no pull requests found"), None);
    }
}
