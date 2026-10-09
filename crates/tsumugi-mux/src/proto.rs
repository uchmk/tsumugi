//! What the client and the server say to each other.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tsumugi_pane::Size;

/// Bumped whenever a message changes shape: a client and a server that
/// disagree say so at `Hello` instead of misreading each other.
pub const VERSION: u32 = 29;

/// A `Hello` with this version asks the server to stop, writing down its
/// tabs first so the next window can bring them back. `Hello` stays the
/// first message with its one number, so this reads the same in every
/// version: any window can stop a server of another version, the one thing
/// it can still say to it.
pub const STOP: u32 = 0;

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
    /// A name given by hand; empty, the tab is called after its pane.
    pub name: String,
    /// Kept at the top of the sidebar, whatever the order.
    pub pinned: bool,
    /// A line written on it by hand (what it is for), shown on its card.
    pub note: String,
}

impl Workspace {
    pub fn new(id: WorkspaceId, layout: Node<SessionId>, focus: SessionId) -> Self {
        Self { id, layout, focus, name: String::new(), pinned: false, note: String::new() }
    }
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
    /// The git branch of `cwd`, when it is in a repository.
    pub branch: String,
    /// The repository `cwd` is in (its top folder), or `cwd` itself: what
    /// the sidebar groups and filters "by folder".
    pub project: PathBuf,
    /// Its notices go to the bell only: no system notification, no number
    /// on the taskbar.
    pub muted: bool,
    /// Name tags put on it (`review`, `ci`), at most [`MAX_TAGS`], in the
    /// order they were put on.
    pub tags: Vec<String>,
    /// Claude Code runs in it: its hooks named a conversation.
    pub claude: bool,
    /// That conversation's id (its transcript's name), or empty.
    pub conversation: String,
    /// The character set its program reads and writes: `UTF-8`, `Shift_JIS`.
    pub charset: String,
    /// The AI program running in it (`claude`, `codex`), by its process;
    /// empty for a plain shell.
    pub agent: String,
    /// The TCP ports its programs listen on (a dev server's `3000`).
    pub ports: Vec<u16>,
    /// The `.cast` file it is being recorded into, or empty.
    pub recording: String,
}

/// How many tags a session can have (the design's 1o).
pub const MAX_TAGS: usize = 5;

/// A tag as it is kept: without a leading `#`, spaces made dashes, at most
/// 24 characters. `None` when nothing is left.
pub fn tag_name(s: &str) -> Option<String> {
    let t: String = s.trim().trim_start_matches('#').trim().chars().map(|c| if c.is_whitespace() { '-' } else { c }).take(24).collect();
    (!t.is_empty()).then_some(t)
}

/// One entry of the notification list (the bell): a session that came to
/// want a person, failed, or finished a long run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub id: u64,
    pub session: SessionId,
    pub state: State,
    /// The session's title then, and what it said.
    pub title: String,
    pub note: String,
    pub at_ms: u64,
    pub read: bool,
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
    /// To the prompt before the view's top (OSC 133), or after it.
    Prompt { back: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToServer {
    Hello { version: u32 },
    /// Answered with `Sessions`.
    List,
    /// Start a shell (`None`: the default one) in `cwd`; answered with
    /// `Spawned`, and the session is attached.
    /// `typed`: a line typed into the shell once its prompt shows (`claude`).
    Spawn { cwd: PathBuf, shell: Option<(String, Vec<String>)>, size: Size, cell: (u16, u16), place: Place, typed: Option<String> },
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
    /// An agent's word on its session (`tsumugi notify`); `claude` is the
    /// Claude Code conversation its hook named, kept for `claude --resume`.
    Notify { id: SessionId, state: State, note: String, claude: Option<String> },
    /// What was saved before a restart, to show before restoring it;
    /// answered with `Saved`.
    Saved,
    /// Start again the tabs saved before a restart -- only the panes listed,
    /// by their saved ids, or all of them -- when the server has no session
    /// yet; answered with `Restored`.
    Restore { only: Option<Vec<SessionId>> },
    /// Mark notifications read: these, or all of them.
    ReadNotices { ids: Option<Vec<u64>> },
    /// The window got or lost the keyboard; answered, to every client, with
    /// `Attention`.
    Focus { focused: bool },
    /// Tell of these sessions only in the bell, or again everywhere.
    Mute { ids: Vec<SessionId>, on: bool },
    /// Put a tag on these sessions (`on`), or take it off.
    Tag { ids: Vec<SessionId>, tag: String, on: bool },
    /// Tell of the sessions with this tag only in the bell, or again
    /// everywhere; answered, to every client, with `MutedTags`.
    MuteTag { tag: String, on: bool },
    /// Put a tab at `to` in the sidebar's own order (dragged there).
    MoveWorkspace { id: WorkspaceId, to: usize },
    /// Take the pane out of its split into a tab of its own, after the
    /// one it was in (a pane dropped on the sidebar).
    OwnTab { id: SessionId },
    /// Record a session into an asciinema `.cast` file at `path` (its
    /// folder made if need be); `None` stops.
    Record { id: SessionId, path: Option<PathBuf> },
    /// Name a tab; an empty name gives it back its pane's.
    RenameWorkspace { id: WorkspaceId, name: String },
    /// Keep a tab at the top of the sidebar, or not.
    PinWorkspace { id: WorkspaceId, on: bool },
    /// Write a note on a tab; empty takes it off.
    NoteWorkspace { id: WorkspaceId, note: String },
    /// End the session's shell and start it again in its place, resuming
    /// the Claude Code conversation that ran in it.
    Restart { id: SessionId },
    /// Find `query` in every session's scrollback and screen; answered
    /// with `FoundAll`.
    SearchAll { query: String },
    /// Put a line found on the session's screen, the match selected.
    Reveal { id: SessionId, line: i32, col: usize, len: usize },
    /// The session's whole buffer as text; answered with `Text`.
    AllText { id: SessionId },
    /// Read and write the session's bytes in this character set (one of
    /// `tsumugi_pane::CHARSETS`).
    SetCharset { id: SessionId, name: String },
    /// Find `needle` (plain text, case aside) in the session's buffer from
    /// the last match on, and put the match on screen, selected; answered
    /// with `Found`. An empty needle forgets where the search got to.
    Find { id: SessionId, needle: String, back: bool },
    /// Put the last command's output (OSC 133 `C` to the next prompt) on
    /// the clipboard: answered with `Clipboard`, or not at all when the
    /// shell marks none.
    CopyOutput { id: SessionId },
    /// A picture's pixels, by the key its placement has: answered with
    /// `Picture`.
    Picture { id: SessionId, key: u64 },
    /// This client's line is slow (ssh): send it a pane's screen at most
    /// every `ms` milliseconds, the latest then, and pictures of at most
    /// `picture_bytes` of RGBA, scaled down. Zero for either is no limit.
    Pace { ms: u32, picture_bytes: u64 },
}

/// A line `SearchAll` found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub id: SessionId,
    /// The scrollback above zero.
    pub line: i32,
    /// The cell the match starts at.
    pub col: usize,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClient {
    Hello { version: u32 },
    Sessions(Vec<Info>),
    /// The answer to `List`: the sessions after everything sent before it.
    /// Not `Sessions`, which any change sends and which could answer a
    /// `List` with a list from before.
    Listed(Vec<Info>),
    Workspaces(Vec<Workspace>),
    Spawned { id: SessionId },
    /// How many sessions `Restore` started.
    Restored(usize),
    /// What `SearchAll` found for `query`, newest first in each session.
    FoundAll { query: String, hits: Vec<Hit> },
    Saved(Option<crate::state::Saved>),
    /// The notification list, newest last, whenever it changes.
    Notices(Vec<Notice>),
    /// When the server started, in Unix milliseconds (the status bar's "up").
    Started { at_ms: u64 },
    /// What changed on the screen of an attached session: every row the
    /// first time, then only the rows that changed.
    Screen { id: SessionId, update: crate::diff::Update },
    /// The session's shell is gone; the session is no more.
    Exited { id: SessionId },
    /// Text for the clipboard: a program set it (OSC 52), or `Copy` asked.
    Clipboard(String),
    /// Whether someone is at one of the windows (`looking`), and whether this
    /// client is the one to tell them otherwise (`teller`: the window that
    /// had the keyboard last), so two windows do not tell twice.
    Attention { looking: bool, teller: bool },
    /// The tags whose sessions are told of in the bell only.
    MutedTags(Vec<String>),
    /// Keeps its place (the 16th) for good: a server of another version
    /// answers `Hello` with it, and a client must read it whatever its own
    /// version. New messages go after it.
    Error(String),
    /// A session ended, to every client: what it was, and the last lines on
    /// its screen (the history of closed sessions).
    Ended { info: Box<Info>, last: Vec<String> },
    /// The answer to `AllText`: the scrollback and the screen.
    Text { id: SessionId, text: String },
    /// The answer to `Find`: `None` when nothing matched, else whether the
    /// search ran off the end and started again to find it.
    Found { id: SessionId, wrapped: Option<bool> },
    /// The answer to `ToServer::Picture`: `None` when the session has let
    /// the picture go.
    Picture { id: SessionId, key: u64, picture: Option<tsumugi_pane::Picture> },
}
