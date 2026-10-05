//! What the client and the server say to each other.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tsumugi_pane::{Screen, Size};

/// Bumped whenever a message changes shape: a client and a server that
/// disagree say so at `Hello` instead of misreading each other.
pub const VERSION: u32 = 1;

pub type SessionId = u64;

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
    Spawn { cwd: PathBuf, shell: Option<(String, Vec<String>)>, size: Size, cell: (u16, u16) },
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClient {
    Hello { version: u32 },
    Sessions(Vec<Info>),
    Spawned { id: SessionId },
    /// The visible screen of an attached session, and what goes with it.
    Screen { id: SessionId, screen: Screen, scrolled_back: usize, win32_input: bool, title: String },
    /// The session's shell is gone; the session is no more.
    Exited { id: SessionId },
    /// Text for the clipboard: a program set it (OSC 52), or `Copy` asked.
    Clipboard(String),
    Error(String),
}
