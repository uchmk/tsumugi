//! The client: a connection to the server, and [`RemotePane`], a session the
//! server holds, drawn and typed into like a local `Terminal`.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crossbeam_channel::Sender;
use tsumugi_pane::alacritty_terminal::grid::Scroll;
use tsumugi_pane::{Pane, Screen, Size};

use crate::frame;
use crate::proto::{self, Info, Node, Notice, Place, ScrollBy, SessionId, ToClient, ToServer, Workspace, WorkspaceId, VERSION};
use crate::transport::{self, Address};

/// How long a request that waits for its answer (`list`, `spawn`) waits.
const ANSWER: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Remote {
    screen: Screen,
    scrolled_back: usize,
    win32_input: bool,
    title: String,
    exited: bool,
}

#[derive(Default)]
struct State {
    screens: HashMap<SessionId, Remote>,
    /// The answer to `list`, taken by it.
    sessions: Option<Vec<Info>>,
    /// The latest list the server sent, kept for a sidebar.
    latest: Vec<Info>,
    workspaces: Vec<Workspace>,
    spawned: VecDeque<Result<SessionId, String>>,
    restored: Option<usize>,
    saved: Option<Option<crate::state::Saved>>,
    notices: Vec<Notice>,
    started_ms: u64,
    clipboard: Vec<String>,
    /// `ToClient::Attention`: someone is at a window, and this one tells.
    looking: bool,
    teller: bool,
    muted_tags: Vec<String>,
    /// The connection is gone (the server stopped, or never answered).
    lost: bool,
}

struct Inner {
    tx: Sender<ToServer>,
    state: Mutex<State>,
    answered: Condvar,
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send(&self, msg: ToServer) {
        let _ = self.tx.send(msg);
    }

    /// Wait until `take` finds the answer, or give up.
    fn wait<T>(&self, mut take: impl FnMut(&mut State) -> Option<T>) -> io::Result<T> {
        let mut st = self.lock();
        let deadline = std::time::Instant::now() + ANSWER;
        loop {
            if let Some(v) = take(&mut st) {
                return Ok(v);
            }
            if st.lost {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "the tsumugi server went away"));
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "the tsumugi server did not answer"));
            }
            st = self.answered.wait_timeout(st, left).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

#[derive(Clone)]
pub struct Client(Arc<Inner>);

impl Client {
    /// Connect to the server at `at`. `wake` is called from the reader
    /// thread whenever something arrives, to have a window redraw.
    pub fn connect(at: &Address, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let (mut r, mut w) = transport::connect(at)?.split()?;
        frame::write(&mut w, &ToServer::Hello { version: VERSION })?;
        match frame::read::<_, ToClient>(&mut r)? {
            ToClient::Hello { version: VERSION } => {}
            ToClient::Error(e) => return Err(io::Error::other(e)),
            other => return Err(io::Error::other(format!("the server answered {other:?}"))),
        }
        let (tx, rx) = crossbeam_channel::unbounded::<ToServer>();
        let inner = Arc::new(Inner { tx, state: Mutex::default(), answered: Condvar::new() });
        std::thread::Builder::new().name("mux-write".into()).spawn(move || {
            for msg in rx {
                if frame::write(&mut w, &msg).is_err() {
                    break;
                }
            }
        })?;
        let reader = inner.clone();
        std::thread::Builder::new().name("mux-read".into()).spawn(move || {
            while let Ok(msg) = frame::read::<_, ToClient>(&mut r) {
                receive(&reader, msg);
                reader.answered.notify_all();
                wake();
            }
            reader.lock().lost = true;
            reader.answered.notify_all();
            wake();
        })?;
        Ok(Self(inner))
    }

    /// The server's sessions.
    pub fn list(&self) -> io::Result<Vec<Info>> {
        self.0.lock().sessions = None;
        self.0.send(ToServer::List);
        self.0.wait(|st| st.sessions.take())
    }

    /// Start a shell in a new session and attach to it.
    pub fn spawn(&self, cwd: PathBuf, shell: Option<(String, Vec<String>)>, size: Size, cell: (u16, u16)) -> io::Result<RemotePane> {
        self.spawn_at(cwd, shell, size, cell, Place::NewWorkspace)
    }

    /// [`spawn`](Self::spawn), splitting a pane or in a tab of its own.
    pub fn spawn_at(
        &self,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        size: Size,
        cell: (u16, u16),
        place: Place,
    ) -> io::Result<RemotePane> {
        self.spawn_typing(cwd, shell, size, cell, place, None)
    }

    /// [`spawn_at`](Self::spawn_at), typing `typed` into the shell once its
    /// prompt shows: how Claude Code starts in a new session.
    pub fn spawn_typing(
        &self,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        size: Size,
        cell: (u16, u16),
        place: Place,
        typed: Option<String>,
    ) -> io::Result<RemotePane> {
        self.0.send(ToServer::Spawn { cwd, shell, size, cell, place, typed });
        let id = self.0.wait(|st| st.spawned.pop_front())?.map_err(io::Error::other)?;
        Ok(self.pane(id))
    }

    /// Watch a session that is already running.
    pub fn attach(&self, id: SessionId) -> RemotePane {
        self.0.send(ToServer::Attach { id });
        self.pane(id)
    }

    fn pane(&self, id: SessionId) -> RemotePane {
        self.0.lock().screens.entry(id).or_default();
        RemotePane { id, inner: self.0.clone(), size: None }
    }

    /// Tell the server how session `id` is (`tsumugi notify`). Returns once
    /// the server has it, so a command that exits right after loses nothing.
    pub fn notify(&self, id: SessionId, state: proto::State, note: String, claude: Option<String>) -> io::Result<()> {
        self.0.send(ToServer::Notify { id, state, note, claude });
        self.list().map(drop)
    }

    /// Start again the tabs saved before a restart, if the server has no
    /// session yet. How many sessions came back.
    pub fn restore(&self) -> io::Result<usize> {
        self.restore_only(None)
    }

    /// [`restore`](Self::restore) only the panes listed, by their saved ids.
    pub fn restore_only(&self, only: Option<Vec<SessionId>>) -> io::Result<usize> {
        self.0.lock().restored = None;
        self.0.send(ToServer::Restore { only });
        let n = self.0.wait(|st| st.restored.take())?;
        self.list()?;
        Ok(n)
    }

    /// What was saved before a restart, if anything.
    pub fn saved(&self) -> io::Result<Option<crate::state::Saved>> {
        self.0.lock().saved = None;
        self.0.send(ToServer::Saved);
        self.0.wait(|st| st.saved.take())
    }

    /// The bell's list, oldest first.
    pub fn notices(&self) -> Vec<Notice> {
        self.0.lock().notices.clone()
    }

    /// Whether someone is at one of the windows, and whether this client is
    /// the one to tell them when not (`ToClient::Attention`).
    pub fn attention(&self) -> (bool, bool) {
        let st = self.0.lock();
        (st.looking, st.teller)
    }

    /// This window got or lost the keyboard.
    pub fn focus(&self, focused: bool) {
        self.0.send(ToServer::Focus { focused });
    }

    /// Tell of these sessions only in the bell (`on`), or everywhere again.
    pub fn mute(&self, ids: Vec<SessionId>, on: bool) {
        self.0.send(ToServer::Mute { ids, on });
    }

    /// Put a tab at `to` in the sidebar's order.
    pub fn move_workspace(&self, id: WorkspaceId, to: usize) {
        self.0.send(ToServer::MoveWorkspace { id, to });
    }

    /// Give a session a prompt as if typed and sent: pasted (so its lines
    /// arrive as one, in bracketed paste where the program asked for it),
    /// then Enter. Its notices are read, as with any key.
    pub fn send_prompt(&self, id: SessionId, text: String) {
        self.0.send(ToServer::Paste { id, text });
        self.0.send(ToServer::Input { id, bytes: b"\r".to_vec() });
    }

    /// Name a tab; empty gives it back its pane's name.
    pub fn rename_workspace(&self, id: WorkspaceId, name: String) {
        self.0.send(ToServer::RenameWorkspace { id, name });
    }

    /// Keep a tab at the top of the sidebar, or not.
    pub fn pin_workspace(&self, id: WorkspaceId, on: bool) {
        self.0.send(ToServer::PinWorkspace { id, on });
    }

    /// Start the session's shell again in its place.
    pub fn restart(&self, id: SessionId) {
        self.0.send(ToServer::Restart { id });
    }

    /// Put a tag on these sessions (`on`), or take it off.
    pub fn tag(&self, ids: Vec<SessionId>, tag: String, on: bool) {
        self.0.send(ToServer::Tag { ids, tag, on });
    }

    /// Tell of the sessions with this tag only in the bell (`on`), or
    /// everywhere again.
    pub fn mute_tag(&self, tag: String, on: bool) {
        self.0.send(ToServer::MuteTag { tag, on });
    }

    /// The tags muted, as the server last told them.
    pub fn muted_tags(&self) -> Vec<String> {
        self.0.lock().muted_tags.clone()
    }

    /// Mark notifications read: these, or all.
    pub fn read_notices(&self, ids: Option<Vec<u64>>) {
        self.0.send(ToServer::ReadNotices { ids });
    }

    /// When the server started, in Unix milliseconds.
    pub fn started_ms(&self) -> u64 {
        self.0.lock().started_ms
    }

    /// The tabs and their splits, as the server last told them.
    pub fn workspaces(&self) -> Vec<Workspace> {
        self.0.lock().workspaces.clone()
    }

    /// Tell the server a workspace's new shape or focus.
    pub fn set_layout(&self, id: WorkspaceId, layout: Node<SessionId>, focus: SessionId) {
        self.0.send(ToServer::SetLayout { id, layout, focus });
    }

    /// The sessions as the server last told them, without asking.
    pub fn sessions(&self) -> Vec<Info> {
        self.0.lock().latest.clone()
    }

    /// Text that arrived for the clipboard since the last call.
    pub fn take_clipboard(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().clipboard)
    }

    /// Whether the server has gone away.
    pub fn lost(&self) -> bool {
        self.0.lock().lost
    }
}

fn receive(inner: &Inner, msg: ToClient) {
    let mut st = inner.lock();
    match msg {
        ToClient::Hello { .. } => {}
        ToClient::Workspaces(list) => st.workspaces = list,
        ToClient::Sessions(list) => st.latest = list,
        ToClient::Listed(list) => {
            st.latest = list.clone();
            st.sessions = Some(list);
        }
        ToClient::Spawned { id } => st.spawned.push_back(Ok(id)),
        ToClient::Restored(n) => st.restored = Some(n),
        ToClient::Saved(s) => st.saved = Some(s),
        ToClient::Notices(list) => st.notices = list,
        ToClient::Started { at_ms } => st.started_ms = at_ms,
        ToClient::Screen { id, update } => {
            let r = st.screens.entry(id).or_default();
            let mut extra = crate::diff::Extra::default();
            crate::diff::apply(&mut r.screen, &mut extra, update);
            (r.scrolled_back, r.win32_input, r.title) = (extra.scrolled_back, extra.win32_input, extra.title);
        }
        ToClient::Exited { id } => st.screens.entry(id).or_default().exited = true,
        ToClient::Clipboard(text) => st.clipboard.push(text),
        ToClient::Attention { looking, teller } => (st.looking, st.teller) = (looking, teller),
        ToClient::MutedTags(list) => st.muted_tags = list,
        // An error answers the request waiting on one (a spawn), if any.
        ToClient::Error(e) => st.spawned.push_back(Err(e)),
    }
}

/// A session the server holds.
pub struct RemotePane {
    id: SessionId,
    inner: Arc<Inner>,
    /// What the server was last told the grid's shape is.
    size: Option<(Size, (u16, u16))>,
}

impl RemotePane {
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The shell has ended (or the server has gone).
    pub fn exited(&self) -> bool {
        let st = self.inner.lock();
        st.lost || st.screens.get(&self.id).is_some_and(|r| r.exited)
    }

    pub fn title(&self) -> String {
        self.inner.lock().screens.get(&self.id).map(|r| r.title.clone()).unwrap_or_default()
    }

    /// End the session's shell.
    pub fn kill(&self) {
        self.inner.send(ToServer::Kill { id: self.id });
    }

    fn with<T: Default>(&self, f: impl FnOnce(&Remote) -> T) -> T {
        self.inner.lock().screens.get(&self.id).map(f).unwrap_or_default()
    }
}

/// Leaving a pane stops its screen being sent here; the session goes on.
impl Drop for RemotePane {
    fn drop(&mut self) {
        self.inner.send(ToServer::Detach { id: self.id });
    }
}

impl Pane for RemotePane {
    fn resize(&mut self, size: Size, cell: (u16, u16)) {
        if self.size != Some((size, cell)) {
            self.size = Some((size, cell));
            self.inner.send(ToServer::Resize { id: self.id, size, cell });
        }
    }

    fn screen(&self) -> Screen {
        self.with(|r| r.screen.clone())
    }

    fn scrolled_back(&self) -> usize {
        self.with(|r| r.scrolled_back)
    }

    fn scroll(&self, by: Scroll) {
        let by = match by {
            Scroll::Delta(n) => ScrollBy::Lines(n),
            Scroll::PageUp => ScrollBy::PageUp,
            Scroll::PageDown => ScrollBy::PageDown,
            Scroll::Top => ScrollBy::Top,
            Scroll::Bottom => ScrollBy::Bottom,
        };
        self.inner.send(ToServer::Scroll { id: self.id, by });
    }

    fn select(&self, cell: (usize, usize), right_half: bool, start: bool) {
        self.inner.send(ToServer::Select { id: self.id, cell, right_half, start });
    }

    fn select_word(&self, cell: (usize, usize)) {
        self.inner.send(ToServer::SelectWord { id: self.id, cell });
    }

    fn clear_selection(&self) {
        self.inner.send(ToServer::ClearSelection { id: self.id });
    }

    /// Asks the server, whose answer arrives as clipboard text
    /// ([`Client::take_clipboard`]); nothing to hand here and now.
    fn selection(&self) -> Option<String> {
        self.inner.send(ToServer::Copy { id: self.id });
        None
    }

    fn send(&self, bytes: Vec<u8>) {
        self.inner.send(ToServer::Input { id: self.id, bytes });
    }

    fn paste(&self, text: &str) {
        self.inner.send(ToServer::Paste { id: self.id, text: text.to_owned() });
    }

    fn win32_input(&self) -> bool {
        self.with(|r| r.win32_input)
    }
}
