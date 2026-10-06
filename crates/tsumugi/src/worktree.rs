//! Sessions in a git worktree of their own: one more Claude Code on the same
//! repository, on its own branch and in its own folder, so two of them
//! never write the same files. The worktree is made beside the repository
//! (`filer` gives `filer-tsumugi-1006-1432`); those tsumugi made are kept in
//! a list beside the state, and when the last session in one ends the
//! window asks whether to remove it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The branch a worktree gets when none is named: `tsumugi/MMDD-HHMM`.
pub fn default_branch(now: chrono::DateTime<chrono::Local>) -> String {
    format!("tsumugi/{}", now.format("%m%d-%H%M"))
}

/// Where the worktree for `branch` of the repository at `root` goes: beside
/// it, named after both; `taken` says a folder is there already.
pub fn place(root: &Path, branch: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let repo = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    let safe: String = branch.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect();
    let base = root.parent().unwrap_or(root).join(format!("{repo}-{safe}"));
    let mut path = base.clone();
    let mut n = 2;
    while taken(&path) {
        path = PathBuf::from(format!("{}-{n}", base.display()));
        n += 1;
    }
    path
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("git did not run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Make a worktree of the repository `folder` is in, on `branch` (made from
/// the current commit when it does not exist yet); its folder.
pub fn add(folder: &Path, branch: &str) -> Result<PathBuf, String> {
    let path = make(folder, branch)?;
    remember(&path, true);
    Ok(path)
}

/// [`add`] without the list.
fn make(folder: &Path, branch: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(git(folder, &["rev-parse", "--show-toplevel"]).map_err(|_| format!("{} is not in a git repository", folder.display()))?);
    let path = place(&root, branch, Path::exists);
    let target = path.to_string_lossy().into_owned();
    let exists = git(&root, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok();
    let made = if exists { git(&root, &["worktree", "add", &target, branch]) } else { git(&root, &["worktree", "add", "-b", branch, &target]) };
    made.map_err(|e| format!("git worktree add: {e}"))?;
    Ok(path)
}

/// Remove the worktree at `path` (its branch stays). Refused by git when it
/// has changes not committed.
pub fn remove(path: &Path) -> Result<(), String> {
    unmake(path)?;
    remember(path, false);
    Ok(())
}

/// [`remove`] without the list.
fn unmake(path: &Path) -> Result<(), String> {
    let target = path.to_string_lossy().into_owned();
    // From the main repository: git will not remove the tree it runs in.
    let common = PathBuf::from(git(path, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).map_err(|e| format!("{}: {e}", path.display()))?);
    let main = common.parent().map_or_else(|| path.to_path_buf(), Path::to_path_buf);
    git(&main, &["worktree", "remove", &target]).map(drop).map_err(|e| format!("git worktree remove: {e}"))
}

fn list_file() -> Option<PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name("worktrees"))
}

/// The worktrees tsumugi made and has not removed.
pub fn ours() -> Vec<PathBuf> {
    let text = list_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    text.lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect()
}

/// Put `path` on the list, or take it off.
pub fn remember(path: &Path, on: bool) {
    let Some(file) = list_file() else { return };
    let mut list = ours();
    list.retain(|p| p != path);
    if on {
        list.push(path.to_path_buf());
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text: String = list.iter().map(|p| format!("{}\n", p.display())).collect();
    let _ = std::fs::write(file, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worktree_goes_beside_its_repository() {
        let root = Path::new("dev").join("filer");
        let none = |_: &Path| false;
        assert_eq!(place(&root, "tsumugi/1006-1432", none), Path::new("dev").join("filer-tsumugi-1006-1432"));
        let first = Path::new("dev").join("filer-fix");
        let taken = |p: &Path| p == first;
        assert_eq!(place(&root, "fix", taken), Path::new("dev").join("filer-fix-2"), "a folder there already");
        let t = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2026, 10, 6, 14, 32, 0).unwrap();
        assert_eq!(default_branch(t), "tsumugi/1006-1432");
    }

    #[test]
    fn a_worktree_is_made_and_removed() {
        let ok = |dir: &Path, args: &[&str]| assert!(git(dir, args).is_ok(), "git {args:?}");
        let base = std::env::temp_dir().join(format!("tsumugi-wt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        if git(&repo, &["init", "-q"]).is_err() {
            return; // no git here
        }
        ok(&repo, &["-c", "user.email=a@b", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "first"]);
        let path = make(&repo, "side").expect("made");
        assert!(path.join(".git").exists());
        assert_eq!(git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(), "side");
        assert!(make(&repo, "side").is_err(), "a branch checked out already is not checked out twice");
        unmake(&path).expect("removed");
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
