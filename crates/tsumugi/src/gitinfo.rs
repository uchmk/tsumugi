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
    ask: Sender<Key>,
    found: Arc<Mutex<HashMap<Key, (Instant, Git)>>>,
    asked: Mutex<HashSet<Key>>,
}

impl Watcher {
    /// `wake` is called when something new is found.
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        let (ask, questions) = channel();
        let found = Arc::new(Mutex::new(HashMap::new()));
        let store = found.clone();
        let _ = std::thread::Builder::new().name("git-info".into()).spawn(move || work(&questions, &store, &wake));
        Self { ask, found, asked: Mutex::new(HashSet::new()) }
    }

    /// What is known of `cwd` on `branch`, asking again when it is old.
    pub fn get(&self, cwd: &Path, branch: &str) -> Option<Git> {
        if branch.is_empty() {
            return None;
        }
        let key = (cwd.to_path_buf(), branch.to_owned());
        let known = self.found.lock().ok()?.get(&key).cloned();
        let stale = known.as_ref().is_none_or(|(at, _)| at.elapsed() > FRESH);
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

fn work(questions: &Receiver<Key>, store: &Mutex<HashMap<Key, (Instant, Git)>>, wake: &dyn Fn()) {
    let mut gh = true;
    while let Ok(mut key) = questions.recv() {
        // Only the newest matters: the pane with the keys now.
        while let Ok(newer) = questions.try_recv() {
            key = newer;
        }
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
            s.insert(key, (Instant::now(), Git { ahead, pr }));
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
