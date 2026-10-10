//! What the server writes down so that a restart of the machine does not
//! lose the tabs: each tab's splits and, per pane, the folder, the shell and
//! the Claude Code conversation running in it. The server is gone after a
//! restart, and so are the shells; the next window starts them again from
//! this (docs/v1-scope.md 4, the order's 7).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ito_layout::Node;

use crate::proto::SessionId;

/// Bumped when the shape below changes; a file of another version is left
/// alone rather than misread.
const VERSION: u32 = 8;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub workspaces: Vec<SavedWorkspace>,
    /// When it was written, in Unix milliseconds: "when tsumugi stopped".
    pub at_ms: u64,
    /// Tags whose sessions are told of in the bell only (v4).
    pub muted_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkspace {
    /// Its name given by hand, and whether it is pinned (v5).
    pub name: String,
    pub pinned: bool,
    /// The note written on it (v6).
    pub note: String,
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
    /// Its title and state when it was saved, for the "Welcome back" list.
    pub title: String,
    pub state: crate::proto::State,
    /// Told of in the bell only (v3).
    pub muted: bool,
    /// Its tags (v4).
    pub tags: Vec<String>,
    /// Its character set, `UTF-8` or another (v7).
    pub charset: String,
    /// The AI program that ran in it, to take up again (v8).
    pub agent: String,
}

/// The shapes before this one, read so that an update does not lose the
/// tabs: v2 (tsumugi 0.5 to 0.7) had no `muted`, v3 (0.8) no tags, v4
/// (0.9 to 0.13) no tab names or pins, v5 (0.14 to 0.45) no notes, v6
/// (0.46) no character sets, v7 (0.47 to 0.48) no AI program's name.
mod old {
    use super::*;

    #[derive(Deserialize)]
    pub struct Saved<P> {
        workspaces: Vec<SavedWorkspace<P>>,
        at_ms: u64,
    }

    #[derive(Deserialize)]
    struct SavedWorkspace<P> {
        layout: Node<SessionId>,
        focus: SessionId,
        panes: Vec<P>,
    }

    #[derive(Deserialize)]
    pub struct V2 {
        id: SessionId,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        claude: Option<String>,
        title: String,
        state: crate::proto::State,
    }

    /// Field by field: postcard is not self-describing, so `flatten` would
    /// not read it.
    #[derive(Deserialize)]
    pub struct V3 {
        id: SessionId,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        claude: Option<String>,
        title: String,
        state: crate::proto::State,
        muted: bool,
    }

    impl From<V2> for super::SavedPane {
        fn from(p: V2) -> Self {
            Self { id: p.id, cwd: p.cwd, shell: p.shell, claude: p.claude, title: p.title, state: p.state, muted: false, tags: Vec::new(), charset: String::new(), agent: String::new() }
        }
    }

    impl From<V3> for super::SavedPane {
        fn from(p: V3) -> Self {
            Self { id: p.id, cwd: p.cwd, shell: p.shell, claude: p.claude, title: p.title, state: p.state, muted: p.muted, tags: Vec::new(), charset: String::new(), agent: String::new() }
        }
    }

    impl<P: Into<super::SavedPane>> From<SavedWorkspace<P>> for super::SavedWorkspace {
        fn from(w: SavedWorkspace<P>) -> Self {
            let panes = w.panes.into_iter().map(Into::into).collect();
            Self { name: String::new(), pinned: false, note: String::new(), layout: w.layout, focus: w.focus, panes }
        }
    }

    impl<P: Into<super::SavedPane>> From<Saved<P>> for super::Saved {
        fn from(s: Saved<P>) -> Self {
            Self { workspaces: s.workspaces.into_iter().map(Into::into).collect(), at_ms: s.at_ms, muted_tags: Vec::new() }
        }
    }

    /// A pane from v4 to v6: with tags, without a character set.
    #[derive(Deserialize)]
    pub struct V4Pane {
        id: SessionId,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        claude: Option<String>,
        title: String,
        state: crate::proto::State,
        muted: bool,
        tags: Vec<String>,
    }

    impl From<V4Pane> for super::SavedPane {
        fn from(p: V4Pane) -> Self {
            Self { id: p.id, cwd: p.cwd, shell: p.shell, claude: p.claude, title: p.title, state: p.state, muted: p.muted, tags: p.tags, charset: String::new(), agent: String::new() }
        }
    }

    /// v4: the tabs without names or pins.
    #[derive(Deserialize)]
    pub struct V4 {
        workspaces: Vec<SavedWorkspace<V4Pane>>,
        at_ms: u64,
        muted_tags: Vec<String>,
    }

    /// v5: the tabs with names and pins, without notes.
    #[derive(Deserialize)]
    pub struct V5 {
        workspaces: Vec<V5Workspace>,
        at_ms: u64,
        muted_tags: Vec<String>,
    }

    #[derive(Deserialize)]
    struct V5Workspace {
        name: String,
        pinned: bool,
        layout: Node<SessionId>,
        focus: SessionId,
        panes: Vec<V4Pane>,
    }

    impl From<V5> for super::Saved {
        fn from(s: V5) -> Self {
            let workspaces = s.workspaces.into_iter().map(|w| super::SavedWorkspace { name: w.name, pinned: w.pinned, note: String::new(), layout: w.layout, focus: w.focus, panes: w.panes.into_iter().map(Into::into).collect() }).collect();
            Self { workspaces, at_ms: s.at_ms, muted_tags: s.muted_tags }
        }
    }

    /// A pane of v7: with a character set, without an AI program's name.
    #[derive(Deserialize)]
    pub struct V7Pane {
        id: SessionId,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        claude: Option<String>,
        title: String,
        state: crate::proto::State,
        muted: bool,
        tags: Vec<String>,
        charset: String,
    }

    impl From<V7Pane> for super::SavedPane {
        fn from(p: V7Pane) -> Self {
            Self { id: p.id, cwd: p.cwd, shell: p.shell, claude: p.claude, title: p.title, state: p.state, muted: p.muted, tags: p.tags, charset: p.charset, agent: String::new() }
        }
    }

    /// v7: as now, but the panes' AI program.
    #[derive(Deserialize)]
    pub struct V7 {
        workspaces: Vec<V7Workspace>,
        at_ms: u64,
        muted_tags: Vec<String>,
    }

    #[derive(Deserialize)]
    struct V7Workspace {
        name: String,
        pinned: bool,
        note: String,
        layout: Node<SessionId>,
        focus: SessionId,
        panes: Vec<V7Pane>,
    }

    impl From<V7> for super::Saved {
        fn from(s: V7) -> Self {
            let workspaces = s.workspaces.into_iter().map(|w| super::SavedWorkspace { name: w.name, pinned: w.pinned, note: w.note, layout: w.layout, focus: w.focus, panes: w.panes.into_iter().map(Into::into).collect() }).collect();
            Self { workspaces, at_ms: s.at_ms, muted_tags: s.muted_tags }
        }
    }

    /// v6: the tabs with notes, the panes without character sets.
    #[derive(Deserialize)]
    pub struct V6 {
        workspaces: Vec<V6Workspace>,
        at_ms: u64,
        muted_tags: Vec<String>,
    }

    #[derive(Deserialize)]
    struct V6Workspace {
        name: String,
        pinned: bool,
        note: String,
        layout: Node<SessionId>,
        focus: SessionId,
        panes: Vec<V4Pane>,
    }

    impl From<V6> for super::Saved {
        fn from(s: V6) -> Self {
            let workspaces = s.workspaces.into_iter().map(|w| super::SavedWorkspace { name: w.name, pinned: w.pinned, note: w.note, layout: w.layout, focus: w.focus, panes: w.panes.into_iter().map(Into::into).collect() }).collect();
            Self { workspaces, at_ms: s.at_ms, muted_tags: s.muted_tags }
        }
    }

    impl From<V4> for super::Saved {
        fn from(s: V4) -> Self {
            Self { workspaces: s.workspaces.into_iter().map(Into::into).collect(), at_ms: s.at_ms, muted_tags: s.muted_tags }
        }
    }
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
    match u32::from_le_bytes(*version) {
        VERSION => postcard::from_bytes(rest).ok(),
        7 => postcard::from_bytes::<old::V7>(rest).ok().map(Into::into),
        6 => postcard::from_bytes::<old::V6>(rest).ok().map(Into::into),
        5 => postcard::from_bytes::<old::V5>(rest).ok().map(Into::into),
        4 => postcard::from_bytes::<old::V4>(rest).ok().map(Into::into),
        3 => postcard::from_bytes::<old::Saved<old::V3>>(rest).ok().map(Into::into),
        2 => postcard::from_bytes::<old::Saved<old::V2>>(rest).ok().map(Into::into),
        _ => None,
    }
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
    // To the disk before it takes the old one's place: a power cut right
    // after a save leaves the old file or the new, never an empty one.
    let aside = path.with_extension("new");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&aside)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&aside, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file from before `muted` (v2) still brings the tabs back.
    #[test]
    fn a_version_2_file_is_read() {
        #[derive(Serialize)]
        struct Pane {
            id: SessionId,
            cwd: PathBuf,
            shell: Option<(String, Vec<String>)>,
            claude: Option<String>,
            title: String,
            state: crate::proto::State,
        }
        let pane = |id| Pane { id, cwd: "/tmp".into(), shell: None, claude: None, title: "t".into(), state: crate::proto::State::Done };
        let old = (vec![(Node::Split { dir: crate::proto::Dir::Right, ratio: 0.5, first: Box::new(Node::Leaf(1)), second: Box::new(Node::Leaf(2)) }, 2u64, vec![pane(1), pane(2)])], 9u64);
        let dir = std::env::temp_dir().join(format!("tsumugi-state-v2-test-{}", std::process::id()));
        let path = dir.join("state");
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend(postcard::to_allocvec(&old).unwrap());
        std::fs::write(&path, bytes).unwrap();
        let saved = load(&path).expect("read");
        assert_eq!(saved.at_ms, 9);
        let ids: Vec<_> = saved.workspaces[0].panes.iter().map(|p| (p.id, p.muted)).collect();
        assert_eq!(ids, vec![(1, false), (2, false)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Files from before notes (v5) and before character sets (v6) keep
    /// what they had; a pane's character set reads as none (UTF-8).
    #[test]
    fn version_5_and_6_files_are_read() {
        #[derive(Serialize)]
        struct Pane {
            id: SessionId,
            cwd: PathBuf,
            shell: Option<(String, Vec<String>)>,
            claude: Option<String>,
            title: String,
            state: crate::proto::State,
            muted: bool,
            tags: Vec<String>,
        }
        #[derive(Serialize)]
        struct Tab5 {
            name: String,
            pinned: bool,
            layout: Node<SessionId>,
            focus: SessionId,
            panes: Vec<Pane>,
        }
        #[derive(Serialize)]
        struct Tab6 {
            name: String,
            pinned: bool,
            note: String,
            layout: Node<SessionId>,
            focus: SessionId,
            panes: Vec<Pane>,
        }
        let pane = || Pane { id: 3, cwd: "/tmp".into(), shell: None, claude: None, title: "t".into(), state: crate::proto::State::Done, muted: false, tags: vec!["a".into()] };
        let dir = std::env::temp_dir().join(format!("tsumugi-state-v5-test-{}", std::process::id()));
        let path = dir.join("state");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |version: u32, body: Vec<u8>| {
            let mut bytes = version.to_le_bytes().to_vec();
            bytes.extend(body);
            std::fs::write(&path, bytes).unwrap();
        };
        write(5, postcard::to_allocvec(&(vec![Tab5 { name: "kept".into(), pinned: true, layout: Node::Leaf(3), focus: 3, panes: vec![pane()] }], 5u64, vec!["ci".to_string()])).unwrap());
        let saved = load(&path).expect("read v5");
        let w = &saved.workspaces[0];
        assert_eq!((w.name.as_str(), w.pinned, w.note.as_str(), saved.muted_tags.clone()), ("kept", true, "", vec!["ci".to_string()]));
        assert_eq!((w.panes[0].tags.clone(), w.panes[0].charset.as_str()), (vec!["a".to_string()], ""));
        write(6, postcard::to_allocvec(&(vec![Tab6 { name: "n".into(), pinned: false, note: "the note".into(), layout: Node::Leaf(3), focus: 3, panes: vec![pane()] }], 6u64, Vec::<String>::new())).unwrap());
        let saved = load(&path).expect("read v6");
        assert_eq!((saved.workspaces[0].note.as_str(), saved.workspaces[0].panes[0].id), ("the note", 3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file from before the AI program's name (v7) keeps its character sets.
    #[test]
    fn a_version_7_file_is_read() {
        #[derive(Serialize)]
        struct Pane {
            id: SessionId,
            cwd: PathBuf,
            shell: Option<(String, Vec<String>)>,
            claude: Option<String>,
            title: String,
            state: crate::proto::State,
            muted: bool,
            tags: Vec<String>,
            charset: String,
        }
        #[derive(Serialize)]
        struct Tab {
            name: String,
            pinned: bool,
            note: String,
            layout: Node<SessionId>,
            focus: SessionId,
            panes: Vec<Pane>,
        }
        let pane = Pane { id: 4, cwd: "/tmp".into(), shell: None, claude: None, title: "t".into(), state: crate::proto::State::Done, muted: false, tags: vec![], charset: "EUC-JP".into() };
        let old = (vec![Tab { name: "n".into(), pinned: false, note: "x".into(), layout: Node::Leaf(4), focus: 4, panes: vec![pane] }], 7u64, Vec::<String>::new());
        let dir = std::env::temp_dir().join(format!("tsumugi-state-v7-test-{}", std::process::id()));
        let path = dir.join("state");
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = 7u32.to_le_bytes().to_vec();
        bytes.extend(postcard::to_allocvec(&old).unwrap());
        std::fs::write(&path, bytes).unwrap();
        let p = &load(&path).expect("read v7").workspaces[0].panes[0];
        assert_eq!((p.charset.as_str(), p.agent.as_str()), ("EUC-JP", ""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_state_comes_back_as_it_was_written() {
        let dir = std::env::temp_dir().join(format!("tsumugi-state-test-{}", std::process::id()));
        let path = dir.join("state");
        let saved = Saved {
            workspaces: vec![SavedWorkspace {
                name: "mine".into(),
                pinned: true,
                note: "the release".into(),
                layout: Node::Leaf(7),
                focus: 7,
                panes: vec![SavedPane {
                    id: 7,
                    cwd: "/tmp".into(),
                    shell: None,
                    claude: Some("abc".into()),
                    title: "t".into(),
                    state: crate::proto::State::Waiting,
                    muted: true,
                    tags: vec!["review".into()],
                    charset: "Shift_JIS".into(),
                    agent: "codex".into(),
                }],
            }],
            at_ms: 1,
            muted_tags: vec!["ci".into()],
        };
        store(&path, &saved).unwrap();
        assert_eq!(load(&path), Some(saved));
        store(&path, &Saved::default()).unwrap();
        assert!(!path.exists(), "nothing to keep, no file");
        assert_eq!(load(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
