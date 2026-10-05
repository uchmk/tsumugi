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
use crate::proto::{Info, ScrollBy, SessionId, ToClient, ToServer, VERSION};
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
    sessions: Option<Vec<Info>>,
    spawned: VecDeque<Result<SessionId, String>>,
    clipboard: Vec<String>,
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
        self.0.send(ToServer::Spawn { cwd, shell, size, cell });
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
        ToClient::Sessions(list) => st.sessions = Some(list),
        ToClient::Spawned { id } => st.spawned.push_back(Ok(id)),
        ToClient::Screen { id, screen, scrolled_back, win32_input, title } => {
            let r = st.screens.entry(id).or_default();
            *r = Remote { screen, scrolled_back, win32_input, title, exited: false };
        }
        ToClient::Exited { id } => st.screens.entry(id).or_default().exited = true,
        ToClient::Clipboard(text) => st.clipboard.push(text),
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
