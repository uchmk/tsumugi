//! The server: it owns the sessions, each a [`Terminal`], and sends each
//! attached client the screen of the sessions it watches whenever one changes.
//!
//! Threads: one accepting clients, two per client (reading its requests and
//! writing to it), and one pump that turns a session's "something happened"
//! into the messages for its watchers. The sessions sit behind one lock, held
//! briefly.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use tsumugi_pane::{Pane, Terminal};

use crate::frame;
use crate::proto::{Info, ScrollBy, SessionId, ToClient, ToServer, VERSION};
use crate::transport::{Address, Conn, Listener};

type ClientId = u64;

struct Session {
    term: Terminal,
    info: Info,
    /// The clients this session's screen goes to.
    watchers: BTreeSet<ClientId>,
}

struct Shared {
    sessions: Mutex<BTreeMap<SessionId, Session>>,
    clients: Mutex<BTreeMap<ClientId, Sender<ToClient>>>,
    next: AtomicU64,
    /// A session to look at: its shell said something, or a client asked.
    dirty: Sender<SessionId>,
    /// Told once, when the last session has ended.
    done: Sender<()>,
    /// A session has been started at some point.
    ever: std::sync::atomic::AtomicBool,
}

/// A running server; [`ServerHandle::wait`] returns when it stops.
pub struct ServerHandle {
    done: Receiver<()>,
    shared: Arc<Shared>,
}

impl ServerHandle {
    /// Block until the last session has ended (the server stops then, as
    /// docs/v1-scope.md has it). A server no session was ever started on
    /// stops after `idle` too, so one started by a window that then failed
    /// does not stay for ever.
    pub fn wait(&self, idle: Duration) {
        loop {
            match self.done.recv_timeout(idle) {
                Err(crossbeam_channel::RecvTimeoutError::Timeout) if self.shared.ever.load(Ordering::Relaxed) => {}
                _ => return,
            }
        }
    }

    /// Whether the server has stopped, without waiting.
    pub fn stopped(&self) -> bool {
        !self.done.is_empty()
    }
}

/// Start a server listening at `at`, on threads of its own.
pub fn start(at: &Address) -> io::Result<ServerHandle> {
    let listener = Listener::bind(at)?;
    let (dirty_tx, dirty_rx) = crossbeam_channel::unbounded();
    let (done_tx, done_rx) = crossbeam_channel::bounded(1);
    let shared = Arc::new(Shared {
        sessions: Mutex::new(BTreeMap::new()),
        clients: Mutex::new(BTreeMap::new()),
        next: AtomicU64::new(1),
        dirty: dirty_tx,
        done: done_tx,
        ever: false.into(),
    });
    let pump = shared.clone();
    std::thread::Builder::new().name("mux-pump".into()).spawn(move || run_pump(pump, dirty_rx))?;
    let accept = shared.clone();
    std::thread::Builder::new().name("mux-accept".into()).spawn(move || {
        while let Ok(conn) = listener.accept() {
            let id = accept.next.fetch_add(1, Ordering::Relaxed);
            let shared = accept.clone();
            let _ = std::thread::Builder::new().name(format!("mux-client-{id}")).spawn(move || serve(shared, id, conn));
        }
    })?;
    Ok(ServerHandle { done: done_rx, shared })
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// One client, from its `Hello` to its going away.
fn serve(shared: Arc<Shared>, client: ClientId, conn: Conn) {
    let Ok((mut r, mut w)) = conn.split() else { return };
    match frame::read::<_, ToServer>(&mut r) {
        Ok(ToServer::Hello { version: VERSION }) => {}
        Ok(ToServer::Hello { version }) => {
            let _ = frame::write(&mut w, &ToClient::Error(format!("the server speaks version {VERSION}, the client {version}")));
            return;
        }
        _ => return,
    }
    if frame::write(&mut w, &ToClient::Hello { version: VERSION }).is_err() {
        return;
    }
    let (tx, rx) = crossbeam_channel::unbounded::<ToClient>();
    lock(&shared.clients).insert(client, tx.clone());
    let writer = std::thread::spawn(move || {
        for msg in rx {
            if frame::write(&mut w, &msg).is_err() {
                break;
            }
        }
    });
    while let Ok(msg) = frame::read::<_, ToServer>(&mut r) {
        handle(&shared, client, &tx, msg);
    }
    // Gone: it watches nothing now, and its writer ends with the channel.
    lock(&shared.clients).remove(&client);
    for s in lock(&shared.sessions).values_mut() {
        s.watchers.remove(&client);
    }
    drop(tx);
    let _ = writer.join();
}

fn handle(shared: &Arc<Shared>, client: ClientId, tx: &Sender<ToClient>, msg: ToServer) {
    let mut sessions = lock(&shared.sessions);
    match msg {
        ToServer::Hello { .. } => {}
        ToServer::List => {
            let _ = tx.send(ToClient::Sessions(sessions.values().map(|s| s.info.clone()).collect()));
        }
        ToServer::Spawn { cwd, shell, size, cell } => {
            let id = shared.next.fetch_add(1, Ordering::Relaxed);
            let command = shell.as_ref().map_or_else(tsumugi_pane::default_program, |(p, _)| p.clone());
            let dirty = shared.dirty.clone();
            let log = std::env::var_os("TSUMUGI_PTY_LOG").map(PathBuf::from);
            match Terminal::spawn(&cwd, size, cell, shell, log.as_deref(), move || {
                let _ = dirty.send(id);
            }) {
                Ok(mut term) => {
                    shared.ever.store(true, Ordering::Relaxed);
                    // What a program asking for the colours (OSC 10 / 11) is
                    // told: the window's default palette, filer's colours.
                    term.set_colors([0xc8, 0xcd, 0xd8], [0x1b, 0x1e, 0x24]);
                    let info = Info { id, cwd, title: String::new(), command };
                    sessions.insert(id, Session { term, info, watchers: BTreeSet::from([client]) });
                    let _ = tx.send(ToClient::Spawned { id });
                    let _ = shared.dirty.send(id);
                }
                Err(e) => {
                    let _ = tx.send(ToClient::Error(format!("the shell did not start: {e}")));
                }
            }
        }
        ToServer::Attach { id } => match sessions.get_mut(&id) {
            Some(s) => {
                s.watchers.insert(client);
                let _ = shared.dirty.send(id);
            }
            None => {
                let _ = tx.send(ToClient::Exited { id });
            }
        },
        ToServer::Detach { id } => {
            if let Some(s) = sessions.get_mut(&id) {
                s.watchers.remove(&client);
            }
        }
        ToServer::Kill { id } => {
            if let Some(s) = sessions.remove(&id) {
                end(shared, &mut sessions, s, id);
            }
        }
        other => {
            let Some(id) = target(&other) else { return };
            let Some(s) = sessions.get_mut(&id) else { return };
            let term = &mut s.term;
            match other {
                ToServer::Input { bytes, .. } => term.send(bytes),
                ToServer::Paste { text, .. } => term.paste(&text),
                ToServer::Resize { size, cell, .. } => Pane::resize(term, size, cell),
                ToServer::Scroll { by, .. } => term.scroll(scroll(by)),
                ToServer::Select { cell, right_half, start, .. } => term.select(cell, right_half, start),
                ToServer::SelectWord { cell, .. } => term.select_word(cell),
                ToServer::ClearSelection { .. } => term.clear_selection(),
                ToServer::Copy { .. } => {
                    if let Some(text) = term.selection() {
                        let _ = tx.send(ToClient::Clipboard(text));
                    }
                }
                _ => {}
            }
            let _ = shared.dirty.send(id);
        }
    }
}

fn target(msg: &ToServer) -> Option<SessionId> {
    Some(match msg {
        ToServer::Input { id, .. }
        | ToServer::Paste { id, .. }
        | ToServer::Resize { id, .. }
        | ToServer::Scroll { id, .. }
        | ToServer::Select { id, .. }
        | ToServer::SelectWord { id, .. }
        | ToServer::ClearSelection { id }
        | ToServer::Copy { id } => *id,
        _ => return None,
    })
}

fn scroll(by: ScrollBy) -> tsumugi_pane::alacritty_terminal::grid::Scroll {
    use tsumugi_pane::alacritty_terminal::grid::Scroll;
    match by {
        ScrollBy::Lines(n) => Scroll::Delta(n),
        ScrollBy::PageUp => Scroll::PageUp,
        ScrollBy::PageDown => Scroll::PageDown,
        ScrollBy::Top => Scroll::Top,
        ScrollBy::Bottom => Scroll::Bottom,
    }
}

/// A session is over: tell its watchers, and stop the server after the last.
fn end(shared: &Shared, sessions: &mut BTreeMap<SessionId, Session>, s: Session, id: SessionId) {
    let clients = lock(&shared.clients);
    for c in &s.watchers {
        if let Some(tx) = clients.get(c) {
            let _ = tx.send(ToClient::Exited { id });
        }
    }
    drop(clients);
    drop(s);
    if sessions.is_empty() {
        let _ = shared.done.try_send(());
    }
}

/// Turn "session N changed" into messages, at most one screen per session
/// every few milliseconds however fast its shell writes.
fn run_pump(shared: Arc<Shared>, dirty: Receiver<SessionId>) {
    while let Ok(first) = dirty.recv() {
        std::thread::sleep(Duration::from_millis(8));
        let mut ids = BTreeSet::from([first]);
        ids.extend(dirty.try_iter());
        let mut sessions = lock(&shared.sessions);
        for id in ids {
            let Some(s) = sessions.get_mut(&id) else { continue };
            let clipboard = s.term.drain();
            s.info.title = s.term.title.clone();
            if s.term.exited {
                let s = sessions.remove(&id).expect("just found");
                end(&shared, &mut sessions, s, id);
                continue;
            }
            let msg = ToClient::Screen {
                id,
                screen: s.term.screen(),
                scrolled_back: s.term.scrolled_back(),
                win32_input: s.term.win32_input(),
                title: s.term.title.clone(),
            };
            let clients = lock(&shared.clients);
            for c in &s.watchers {
                if let Some(tx) = clients.get(c) {
                    for text in &clipboard {
                        let _ = tx.send(ToClient::Clipboard(text.clone()));
                    }
                    let _ = tx.send(msg.clone());
                }
            }
        }
    }
}
