//! The client: a connection to the server, and [`RemotePane`], a session the
//! server holds, drawn and typed into like a local `Terminal`.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crossbeam_channel::Sender;
use ito_pane::alacritty_terminal::grid::Scroll;
use ito_pane::{Pane, Screen, Size};

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
    bracketed_paste: bool,
    title: String,
    links: Vec<ito_pane::Hyperlink>,
    blocks: Vec<ito_pane::Block>,
    pictures: Vec<ito_pane::Placement>,
    /// Pictures' pixels by key, as they came; `None` while asked for and
    /// not come, or let go by the server.
    pixels: HashMap<u64, Option<Arc<ito_pane::Picture>>>,
    exited: bool,
    /// Something is selected in the server's pane, on the screen or not.
    selected: bool,
    /// The answer to `find`, not taken yet.
    found: Option<Option<bool>>,
}

#[derive(Default)]
struct State {
    screens: HashMap<SessionId, Remote>,
    /// The answer to `list`, taken by it.
    sessions: Option<Vec<Info>>,
    /// The last answer to `search_all`.
    found_all: Option<(String, Vec<crate::proto::Hit>)>,
    /// The latest list the server sent, kept for a sidebar.
    latest: Vec<Info>,
    workspaces: Vec<Workspace>,
    spawned: VecDeque<Result<SessionId, String>>,
    restored: Option<usize>,
    saved: Option<Option<crate::state::Saved>>,
    notices: Vec<Notice>,
    started_ms: u64,
    /// The server's tsumugi version, once it has said.
    server_build: String,
    clipboard: Vec<String>,
    /// The latest selection for Linux's primary selection.
    primary: Option<String>,
    /// Sessions that ended since the last `take_ended`, with their last lines.
    ended: Vec<(Info, Vec<String>)>,
    /// Whole buffers `all_text` asked for, not taken yet.
    texts: Vec<(SessionId, String)>,
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

/// The `ssh` a [`Client::over_ssh`] runs, ended with the last clone of the
/// client: the reader thread holds the connection, not this.
struct Ssh(Mutex<std::process::Child>);

impl Drop for Ssh {
    fn drop(&mut self) {
        let mut child = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[derive(Clone)]
pub struct Client(Arc<Inner>, Option<Arc<Ssh>>);

impl Client {
    /// Connect to the server at `at`. `wake` is called from the reader
    /// thread whenever something arrives, to have a window redraw.
    pub fn connect(at: &Address, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let (r, w) = transport::connect(at)?.split()?;
        Self::over(r, w, wake)
    }

    /// Connect to the server on another machine: `ssh HOST tsumugi proxy`,
    /// which starts that machine's server if need be and carries the
    /// connection over ssh's standard input and output. The OS's `ssh`, so
    /// keys, the agent and `~/.ssh/config` work as they do in a shell; it
    /// never asks for a password (`BatchMode`), since nobody would see the
    /// question. The server stays when the line drops, like tmux.
    /// `command` is what runs there in place of `tsumugi` (the settings'
    /// `[remote]`). `TSUMUGI_SSH` names another `ssh` (the tests), and the
    /// error says what to do when it did not get through ([`explain`]).
    /// The line may be slow: the server is asked to send screens less
    /// often and pictures smaller ([`ToServer::Pace`]).
    pub fn over_ssh(host: &str, command: &str, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let (mut child, r, w, mut said) = run_ssh(host, command)?;
        match Self::over(Box::new(r), Box::new(w), wake) {
            Ok(mut c) => {
                // Warnings ssh prints later must not fill its pipe and stop it.
                std::thread::Builder::new().name("ssh-stderr".into()).spawn(move || io::copy(&mut said, &mut io::sink()))?;
                c.1 = Some(Arc::new(Ssh(Mutex::new(child))));
                c.0.send(ToServer::Pace { ms: SSH_PACE_MS, picture_bytes: SSH_PICTURE_BYTES });
                Ok(c)
            }
            Err(e) => {
                // ssh is going: let it, for its exit code (127, the command
                // was not found), then whatever it said.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                let code = loop {
                    match child.try_wait() {
                        Ok(Some(st)) => break st.code(),
                        Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(20)),
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break None;
                        }
                    }
                };
                let mut text = String::new();
                let _ = io::Read::read_to_string(&mut said, &mut text);
                Err(io::Error::new(e.kind(), explain(host, command, &e, code, text.trim())))
            }
        }
    }

    /// Ask the server on `host` to stop ([`stop`](Self::stop) over ssh): to
    /// replace one of another version, or one that went wrong. Its tabs
    /// stay for the next. Returns once it has gone (the server holds the
    /// line until then, and `tsumugi proxy` ends with it), or after 10 s.
    pub fn stop_over_ssh(host: &str, command: &str) -> io::Result<()> {
        let (mut child, _r, mut w, _said) = run_ssh(host, command)?;
        // `w` stays open: proxy ends when its stdin does.
        let sent = frame::write(&mut w, &ToServer::Hello { version: proto::STOP });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(50)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
            }
        }
        sent
    }

    /// Speak to a server over any byte stream both ways.
    pub fn over(
        mut r: Box<dyn io::Read + Send>,
        mut w: Box<dyn io::Write + Send>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Self> {
        frame::write(&mut w, &ToServer::Hello { version: VERSION })?;
        // An answer that does not read is a server of another version too
        // (one from before `Error` kept its place).
        let answer = frame::read::<_, ToClient>(&mut r).map_err(|e| match e.kind() {
            io::ErrorKind::InvalidData => io::Error::new(io::ErrorKind::InvalidData, "the server speaks another version"),
            _ => e,
        })?;
        match answer {
            ToClient::Hello { version: VERSION } => {}
            // A server of another version: told apart, so a window can offer
            // to stop it (`stop`).
            ToClient::Error(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
            ToClient::Hello { version } => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("the server speaks version {version}, the client {VERSION}"))),
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
        Ok(Self(inner, None))
    }

    /// Ask the server at `at` to stop, whatever its version, keeping its
    /// tabs for the next window to bring back (`proto::STOP`).
    pub fn stop(at: &Address) -> io::Result<()> {
        let (_r, mut w) = transport::connect(at)?.split()?;
        frame::write(&mut w, &ToServer::Hello { version: proto::STOP })
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
        self.spawn_typing(cwd, shell, size, cell, place, Vec::new())
    }

    /// [`spawn_at`](Self::spawn_at), typing the command `typed` (its words,
    /// or one word that is a line) into the shell once its prompt shows:
    /// how Claude Code starts in a new session.
    pub fn spawn_typing(
        &self,
        cwd: PathBuf,
        shell: Option<(String, Vec<String>)>,
        size: Size,
        cell: (u16, u16),
        place: Place,
        typed: Vec<String>,
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

    /// Take the pane out of its split into a tab of its own.
    pub fn own_tab(&self, id: SessionId) {
        self.0.send(ToServer::OwnTab { id });
    }

    /// Record a session into a `.cast` file at `path`; `None` stops. What
    /// went wrong comes back as an error.
    pub fn record(&self, id: SessionId, path: Option<PathBuf>) {
        self.0.send(ToServer::Record { id, path });
    }

    /// Write a session's work log to `path` as it goes; `None` finishes it.
    /// Where it went is the session's `logging`.
    pub fn log(&self, id: SessionId, path: Option<PathBuf>) {
        self.0.send(ToServer::Log { id, path });
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

    /// Look for `query` in every session's scrollback; the answer comes to
    /// `found_all`, without waiting for it here.
    pub fn search_all(&self, query: String) {
        self.0.send(ToServer::SearchAll { query });
    }

    /// The last answer to `search_all`: what was looked for, and the lines.
    pub fn found_all(&self) -> Option<(String, Vec<crate::proto::Hit>)> {
        self.0.lock().found_all.clone()
    }

    /// Show a line `search_all` found, on its session's screen.
    pub fn reveal(&self, id: SessionId, line: i32, col: usize, len: usize) {
        self.0.send(ToServer::Reveal { id, line, col, len });
    }

    /// Name a tab; empty gives it back its pane's name.
    pub fn rename_workspace(&self, id: WorkspaceId, name: String) {
        self.0.send(ToServer::RenameWorkspace { id, name });
    }

    /// Write a note on a tab; empty takes it off.
    pub fn note_workspace(&self, id: WorkspaceId, note: String) {
        self.0.send(ToServer::NoteWorkspace { id, note });
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

    /// The server's tsumugi version; empty until it has said.
    pub fn server_build(&self) -> String {
        self.0.lock().server_build.clone()
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

    /// Sessions that ended since the last call: what each was, and the last
    /// lines on its screen.
    pub fn take_ended(&self) -> Vec<(Info, Vec<String>)> {
        std::mem::take(&mut self.0.lock().ended)
    }

    /// Read and write the session's bytes in this character set.
    pub fn set_charset(&self, id: SessionId, name: String) {
        self.0.send(ToServer::SetCharset { id, name });
    }

    /// Ask for a session's whole buffer as text; it comes to `take_texts`.
    pub fn all_text(&self, id: SessionId) {
        self.0.send(ToServer::AllText { id });
    }

    /// The whole buffers that arrived since the last call.
    pub fn take_texts(&self) -> Vec<(SessionId, String)> {
        std::mem::take(&mut self.0.lock().texts)
    }

    /// Text that arrived for the clipboard since the last call.
    pub fn take_clipboard(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().clipboard)
    }

    /// The selection that arrived for the primary selection since the last
    /// call ([`RemotePane::ask_primary`]); only the latest counts.
    pub fn take_primary(&self) -> Option<String> {
        self.0.lock().primary.take()
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
        ToClient::FoundAll { query, hits } => st.found_all = Some((query, hits)),
        ToClient::Saved(s) => st.saved = Some(s),
        ToClient::Notices(list) => st.notices = list,
        ToClient::Started { at_ms, build } => {
            st.started_ms = at_ms;
            st.server_build = build;
        }
        ToClient::Screen { id, update } => {
            let r = st.screens.entry(id).or_default();
            let mut extra = crate::diff::Extra::default();
            crate::diff::apply(&mut r.screen, &mut extra, update);
            (r.scrolled_back, r.win32_input, r.bracketed_paste, r.title, r.links, r.blocks, r.pictures, r.selected) = (extra.scrolled_back, extra.win32_input, extra.bracketed_paste, extra.title, extra.links, extra.blocks, extra.pictures, extra.selected);
            // Pixels of pictures no longer on the screen are let go; the
            // view keeps its own textures a while, should one come back.
            let on: std::collections::HashSet<u64> = r.pictures.iter().map(|p| p.key).collect();
            r.pixels.retain(|k, _| on.contains(k));
        }
        ToClient::Exited { id } => st.screens.entry(id).or_default().exited = true,
        ToClient::Ended { info, last } => st.ended.push((*info, last)),
        ToClient::Text { id, text } => st.texts.push((id, text)),
        ToClient::Found { id, wrapped } => st.screens.entry(id).or_default().found = Some(wrapped),
        ToClient::Clipboard(text) => st.clipboard.push(text),
        ToClient::Primary(text) => st.primary = Some(text),
        ToClient::Picture { id, key, picture } => {
            let r = st.screens.entry(id).or_default();
            if r.pictures.iter().any(|p| p.key == key) {
                r.pixels.insert(key, picture.map(Arc::new));
            }
        }
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

    /// Put the last command's output on the clipboard (OSC 133); the text
    /// comes back as clipboard text, as a copy's does.
    pub fn copy_output(&self) {
        self.inner.send(ToServer::CopyOutput { id: self.id });
    }

    /// Ask for the selection as Linux's primary selection; it comes back
    /// through [`Client::take_primary`].
    pub fn ask_primary(&self) {
        self.inner.send(ToServer::Copy { id: self.id, primary: true });
    }

    /// Scroll to the prompt before the view's top, or after it (OSC 133).
    pub fn jump_prompt(&self, back: bool) {
        self.inner.send(ToServer::Scroll { id: self.id, by: ScrollBy::Prompt { back } });
    }

    /// Find `needle` in the buffer from the last match on (see
    /// `ToServer::Find`); the answer comes to `take_found`. Empty: forget
    /// the search.
    pub fn find(&self, needle: &str, back: bool) {
        self.inner.send(ToServer::Find { id: self.id, needle: needle.to_owned(), back });
    }

    /// The answer to the last `find`, once: `Some(None)` when nothing
    /// matched, `Some(Some(wrapped))` when something did.
    pub fn take_found(&self) -> Option<Option<bool>> {
        self.inner.lock().screens.get_mut(&self.id).and_then(|r| r.found.take())
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
        self.inner.send(ToServer::Select { id: self.id, cell, right_half, start, block: false });
    }

    fn select_block(&self, cell: (usize, usize), right_half: bool) {
        self.inner.send(ToServer::Select { id: self.id, cell, right_half, start: true, block: true });
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
        self.inner.send(ToServer::Copy { id: self.id, primary: false });
        None
    }

    fn has_selection(&self) -> bool {
        self.with(|r| r.selected)
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

    fn bracketed_paste(&self) -> bool {
        self.with(|r| r.bracketed_paste)
    }

    fn focus_report(&self) -> bool {
        self.with(|r| r.screen.focus_report)
    }

    fn kitty_flags(&self) -> u8 {
        self.with(|r| r.screen.kitty)
    }

    fn hyperlinks(&self) -> Vec<ito_pane::Hyperlink> {
        self.with(|r| r.links.clone())
    }

    fn blocks(&self) -> Vec<ito_pane::Block> {
        self.with(|r| r.blocks.clone())
    }

    fn pictures(&self) -> Vec<ito_pane::Placement> {
        self.with(|r| r.pictures.clone())
    }

    /// Asked for the first time it is wanted; the answer wakes the window,
    /// which draws it then.
    fn picture(&self, key: u64) -> Option<Arc<ito_pane::Picture>> {
        let mut st = self.inner.lock();
        let r = st.screens.entry(self.id).or_default();
        match r.pixels.get(&key) {
            Some(p) => p.clone(),
            None => {
                r.pixels.insert(key, None);
                drop(st);
                self.inner.send(ToServer::Picture { id: self.id, key });
                None
            }
        }
    }
}

/// What `ssh` is given to reach `host`'s server: no terminal (`-T`, the bytes
/// are frames), no questions (`BatchMode`), a dead line noticed in about half
/// a minute, and `--` so a host starting with `-` is not read as an option.
/// `command` is read by the shell there, so a path with spaces is quoted
/// in it.
pub fn ssh_args(host: &str, command: &str) -> Vec<String> {
    ["-T", "-o", "BatchMode=yes", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3", "--", host, command, "proxy"].map(String::from).to_vec()
}

/// How often a server sends a client over ssh a pane's screen, at most
/// (milliseconds): ten a second is enough to read, and a slow line keeps up.
const SSH_PACE_MS: u32 = 100;
/// How big a picture a server sends a client over ssh, at most (bytes of
/// RGBA): larger ones come scaled down.
const SSH_PICTURE_BYTES: u64 = 1_000_000;

type SshPipes = (std::process::Child, std::process::ChildStdout, std::process::ChildStdin, std::process::ChildStderr);

/// `ssh HOST COMMAND proxy`, its three pipes taken.
fn run_ssh(host: &str, command: &str) -> io::Result<SshPipes> {
    use std::process::{Command, Stdio};
    let ssh = std::env::var_os("TSUMUGI_SSH").filter(|s| !s.is_empty()).unwrap_or_else(|| "ssh".into());
    let mut cmd = Command::new(ssh);
    cmd.args(ssh_args(host, command)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // The window has no console, and ssh would open one of its own.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| io::Error::new(e.kind(), format!("could not run ssh: {e}")))?;
    match (child.stdout.take(), child.stdin.take(), child.stderr.take()) {
        (Some(r), Some(w), Some(said)) => Ok((child, r, w, said)),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            Err(io::Error::other("ssh gave no pipes"))
        }
    }
}

/// Where tsumugi is had, for a machine without it.
pub const RELEASES: &str = "https://github.com/uchmk/tsumugi/releases";

/// Why `ssh HOST COMMAND proxy` gave no server, and what to do: `e` is how
/// the talk failed, `code` ssh's exit code (255 is ssh's own failure, 127 a
/// shell's "not found"), `said` what it printed.
pub fn explain(host: &str, command: &str, e: &io::Error, code: Option<i32>, said: &str) -> String {
    let lower = said.to_lowercase();
    let missing = code != Some(255)
        && (code == Some(127) || ["not found", "not recognized", "no such file", "cannot find"].iter().any(|w| lower.contains(w)));
    let here = env!("CARGO_PKG_VERSION");
    if missing {
        let it_said = match said.is_empty() {
            true => String::new(),
            false => format!(" (it said: {said})"),
        };
        format!("{host}: tsumugi is not there (\"{command}\" was not found){it_said}. Install it from {RELEASES} and put it on the PATH, or give its path in Settings → Advanced → Command there.")
    } else if e.kind() == io::ErrorKind::InvalidData {
        format!("{host}: {e}. Install tsumugi {here} there, the same as here; if it is, restart its server from the machine's menu (its tabs stay).")
    } else if said.is_empty() {
        format!("{host}: {e}")
    } else {
        format!("{host}: {said}")
    }
}
