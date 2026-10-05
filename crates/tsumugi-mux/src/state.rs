//! What the server writes down so that a restart of the machine does not
//! lose the tabs: each tab's splits and, per pane, the folder, the shell and
//! the Claude Code conversation running in it. The server is gone after a
//! restart, and so are the shells; the next window starts them again from
//! this (docs/v1-scope.md 4, the order's 7).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tsumugi_layout::Node;

use crate::proto::SessionId;

/// Bumped when the shape below changes; a file of another version is left
/// alone rather than misread.
const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub workspaces: Vec<SavedWorkspace>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkspace {
    /// The splits, with the ids the sessions had (only a key into `panes`).
    pub layout: Node<SessionId>,
    pub focus: SessionId,
    pub panes: Vec<SavedPane>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedPane {
    pub id: SessionId,
    pub cwd: PathBuf,
    /// The program and its arguments; `None` for the default shell.
    pub shell: Option<(String, Vec<String>)>,
    /// The Claude Code conversation last seen in it (its hooks' `session_id`),
    /// resumed with `claude --resume`.
    pub claude: Option<String>,
}

/// Where the state is kept: `TSUMUGI_STATE` when set; else
/// `%LOCALAPPDATA%\tsumugi\state` on Windows, `~/Library/Application
/// Support/tsumugi/state` on macOS, `$XDG_STATE_HOME/tsumugi/state` (or
/// `~/.local/state/tsumugi/state`) elsewhere.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TSUMUGI_STATE").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else {
        var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("state")))?
    };
    Some(dir.join("tsumugi").join("state"))
}

/// The saved state, or nothing when there is none or it cannot be read.
pub fn load(path: &Path) -> Option<Saved> {
    let bytes = std::fs::read(path).ok()?;
    let (version, rest) = bytes.split_first_chunk::<4>()?;
    (u32::from_le_bytes(*version) == VERSION).then(|| postcard::from_bytes(rest).ok()).flatten()
}

/// Write the state, or remove the file when there is nothing to keep. The
/// file is replaced whole (written aside, then renamed), so a restart in the
/// middle leaves the old one rather than half of the new.
pub fn store(path: &Path, saved: &Saved) -> io::Result<()> {
    if saved.workspaces.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut bytes = VERSION.to_le_bytes().to_vec();
    bytes.extend(postcard::to_allocvec(saved).map_err(io::Error::other)?);
    let aside = path.with_extension("new");
    std::fs::write(&aside, bytes)?;
    std::fs::rename(&aside, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_comes_back_as_it_was_written() {
        let dir = std::env::temp_dir().join(format!("tsumugi-state-test-{}", std::process::id()));
        let path = dir.join("state");
        let saved = Saved {
            workspaces: vec![SavedWorkspace {
                layout: Node::Leaf(7),
                focus: 7,
                panes: vec![SavedPane { id: 7, cwd: "/tmp".into(), shell: None, claude: Some("abc".into()) }],
            }],
        };
        store(&path, &saved).unwrap();
        assert_eq!(load(&path), Some(saved));
        store(&path, &Saved::default()).unwrap();
        assert!(!path.exists(), "nothing to keep, no file");
        assert_eq!(load(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
