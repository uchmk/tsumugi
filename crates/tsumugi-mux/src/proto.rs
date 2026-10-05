//! What the client and the server say to each other.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tsumugi_pane::Size;

/// Bumped whenever a message changes shape: a client and a server that
/// disagree say so at `Hello` instead of misreading each other.
pub const VERSION: u32 = 5;

pub type SessionId = u64;
pub type WorkspaceId = u64;

pub use tsumugi_layout::{Dir, Node};

/// A tab of the sidebar: panes, each a session, split in a tree. The server
/// keeps it, so closing the window loses no split.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub layout: Node<SessionId>,
    /// The pane that has the keys.
    pub focus: SessionId,
}

/// Where a new session goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// A workspace of its own: a new tab.
    NewWorkspace,
    /// Beside a session, splitting its pane.
    Split { beside: SessionId, dir: Dir },
}

/// What a session is doing, as its mark in the sidebar shows it
/// (docs/v1-scope.md 2, QUESTIONS.md Q2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Working, or nothing known.
    #[default]
    Running,
    /// Wants a person: an agent said so (`tsumugi notify`, OSC 9 / 99 / 777).
    Waiting,
    /// Probably wants a person: its output stopped a while ago while a
    /// program is still running. The weaker mark.
    MaybeWaiting,
    /// Back at the shell's prompt, or an agent said it finished.
    Done,
    /// An agent said something went wrong.
    Error,
}

impl State {
    /// The word `tsumugi ls` prints and `tsumugi notify --state` takes.
    pub fn word(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Waiting => "waiting",
            State::MaybeWaiting => "waiting?",
            State::Done => "done",
            State::Error => "error",
        }
    }

    pub fn from_word(s: &str) -> Option<Self> {
        Some(match s {
            "running" => State::Running,
            "waiting" => State::Waiting,
            "done" => State::Done,
            "error" => State::Error,
            _ => return None,
        })
    }
}

/// One session, as `tsumugi ls` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    pub id: SessionId,
    /// Where the shell started.
    pub cwd: PathBuf,
    /// What the shell last set the title to.
    pub title: String,
    /// The program that was started (`pwsh`, `claude`).
    pub command: String,
    pub state: State,
    /// What the last notification said, if anything.
    pub note: String,
    /// When `state` began, in Unix milliseconds: the session waiting longest
    /// is the one `Ctrl+Shift+U` goes to first.
    pub since_ms: u64,
}

/// The scrollback moves `Scroll` can ask for; alacritty's own type is not a
/// serde one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollBy {
    Lines(i32),
    PageUp,
    PageDown,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToServer {
    Hello { version: u32 },
    /// Answered with `Sessions`.
    List,
    /// Start a shell (`None`: the default one) in `cwd`; answered with
    /// `Spawned`, and the session is attached.
    Spawn { cwd: PathBuf, shell: Option<(String, Vec<String>)>, size: Size, cell: (u16, u16), place: Place },
    /// Send this session's screen whenever it changes, starting now.
    Attach { id: SessionId },
    Detach { id: SessionId },
    Input { id: SessionId, bytes: Vec<u8> },
    Paste { id: SessionId, text: String },
    Resize { id: SessionId, size: Size, cell: (u16, u16) },
    Scroll { id: SessionId, by: ScrollBy },
    Select { id: SessionId, cell: (usize, usize), right_half: bool, start: bool },
    SelectWord { id: SessionId, cell: (usize, usize) },
    ClearSelection { id: SessionId },
    /// Put the selection on the clipboard: answered with `Clipboard`.
    Copy { id: SessionId },
    /// End the session's shell.
    Kill { id: SessionId },
    /// A workspace's new shape or focus, after a drag, a click or a key.
    SetLayout { id: WorkspaceId, layout: Node<SessionId>, focus: SessionId },
    /// An agent's word on its session (`tsumugi notify`).
    Notify { id: SessionId, state: State, note: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClient {
    Hello { version: u32 },
    Sessions(Vec<Info>),
    Workspaces(Vec<Workspace>),
    Spawned { id: SessionId },
    /// What changed on the screen of an attached session: every row the
    /// first time, then only the rows that changed.
    Screen { id: SessionId, update: crate::diff::Update },
    /// The session's shell is gone; the session is no more.
    Exited { id: SessionId },
    /// Text for the clipboard: a program set it (OSC 52), or `Copy` asked.
    Clipboard(String),
    Error(String),
}
