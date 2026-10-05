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
use crate::proto::{Info, Place, ScrollBy, SessionId, State, ToClient, ToServer, Workspace, WorkspaceId, VERSION};
use crate::transport::{Address, Conn, Listener};

type ClientId = u64;

struct Session {
    term: Terminal,
    info: Info,
    /// The last notification and when it came: an agent's own word on its
    /// state, which stands until the next key.
    notice: Option<(State, std::time::Instant)>,
    /// The clients this session's screen goes to.
    watchers: BTreeSet<ClientId>,
}

struct Shared {
    sessions: Mutex<BTreeMap<SessionId, Session>>,
    /// The tabs and their splits. Locked after `sessions` when both are.
    workspaces: Mutex<BTreeMap<WorkspaceId, Workspace>>,
    clients: Mutex<BTreeMap<ClientId, Sender<ToClient>>>,
    next: AtomicU64,
    /// A session to look at: its shell said something, or a client asked.
    dirty: Sender<SessionId>,
    /// Told once, when the last session has ended.
    done: Sender<()>,
    /// A session has been started at some point.
    ever: std::sync::atomic::AtomicBool,
    /// Where this server listens, told to its shells (`TSUMUGI_ADDRESS`) so
    /// that `tsumugi notify` inside them finds it.
    address: PathBuf,
    /// Seconds of silence after which a running program reads as probably
    /// waiting (Q2: Konsole's 10). 0 turns the guess off.
    quiet: Duration,
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
        workspaces: Mutex::new(BTreeMap::new()),
        clients: Mutex::new(BTreeMap::new()),
        next: AtomicU64::new(1),
        dirty: dirty_tx,
        done: done_tx,
        ever: false.into(),
        address: at.0.clone(),
        quiet: Duration::from_secs(
            std::env::var("TSUMUGI_QUIET_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10),
        ),
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
            let _ = tx.send(ToClient::Workspaces(lock(&shared.workspaces).values().cloned().collect()));
            let _ = tx.send(ToClient::Sessions(sessions.values().map(|s| s.info.clone()).collect()));
        }
        ToServer::SetLayout { id, layout, focus } => {
            // Only panes that are still sessions: the window may have sent
            // this before it heard that one ended.
            let mut workspaces = lock(&shared.workspaces);
            if let (Some(w), Some(layout)) = (workspaces.get_mut(&id), layout.retain(&|s| sessions.contains_key(s))) {
                w.focus = if layout.contains(&focus) { focus } else { layout.leaves()[0] };
                w.layout = layout;
            }
            broadcast_workspaces(shared, &workspaces);
        }
        ToServer::Spawn { cwd, shell, size, cell, place } => {
            let id = shared.next.fetch_add(1, Ordering::Relaxed);
            let command = shell.as_ref().map_or_else(tsumugi_pane::default_program, |(p, _)| p.clone());
            let dirty = shared.dirty.clone();
            let log = std::env::var_os("TSUMUGI_PTY_LOG").map(PathBuf::from);
            let env = vec![
                ("TSUMUGI_SESSION".to_owned(), id.to_string()),
                ("TSUMUGI_ADDRESS".to_owned(), shared.address.to_string_lossy().into_owned()),
            ];
            match Terminal::spawn_with_env(&cwd, size, cell, shell, log.as_deref(), env, move || {
                let _ = dirty.send(id);
            }) {
                Ok(mut term) => {
                    shared.ever.store(true, Ordering::Relaxed);
                    // What a program asking for the colours (OSC 10 / 11) is
                    // told: the window's default palette, filer's colours.
                    term.set_colors([0xc8, 0xcd, 0xd8], [0x1b, 0x1e, 0x24]);
                    let info = Info { id, cwd, title: String::new(), command, state: State::Running, note: String::new(), since_ms: now_ms() };
                    sessions.insert(id, Session { term, info, notice: None, watchers: BTreeSet::from([client]) });
                    let mut workspaces = lock(&shared.workspaces);
                    place_session(shared, &mut workspaces, id, place);
                    broadcast_workspaces(shared, &workspaces);
                    drop(workspaces);
                    let _ = tx.send(ToClient::Spawned { id });
                    broadcast(shared, &sessions);
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
        ToServer::Kill { id } => end(shared, sessions, id),
        ToServer::Notify { id, state, note } => {
            if let Some(s) = sessions.get_mut(&id) {
                s.notice = Some((state, std::time::Instant::now()));
                if !note.is_empty() {
                    s.info.note = note;
                }
                if settle(s, shared.quiet) {
                    broadcast(shared, &sessions);
                }
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

/// Tell every client the sessions as they are now: one was added, ended, or
/// changed its title or folder. A sidebar is drawn from this.
fn broadcast(shared: &Shared, sessions: &BTreeMap<SessionId, Session>) {
    let list: Vec<Info> = sessions.values().map(|s| s.info.clone()).collect();
    for tx in lock(&shared.clients).values() {
        let _ = tx.send(ToClient::Sessions(list.clone()));
    }
}

/// Put a new session in the workspace it is meant for, splitting the pane
/// it goes beside, or in a workspace of its own; it gets the keys.
fn place_session(shared: &Shared, workspaces: &mut BTreeMap<WorkspaceId, Workspace>, id: SessionId, place: Place) {
    if let Place::Split { beside, dir } = place {
        if let Some(w) = workspaces.values_mut().find(|w| w.layout.contains(&beside)) {
            w.layout.split(&beside, dir, id);
            w.focus = id;
            return;
        }
    }
    let ws = shared.next.fetch_add(1, Ordering::Relaxed);
    workspaces.insert(ws, Workspace { id: ws, layout: tsumugi_layout::Node::Leaf(id), focus: id });
}

/// Tell every client the workspaces as they are now.
fn broadcast_workspaces(shared: &Shared, workspaces: &BTreeMap<WorkspaceId, Workspace>) {
    let list: Vec<Workspace> = workspaces.values().cloned().collect();
    for tx in lock(&shared.clients).values() {
        let _ = tx.send(ToClient::Workspaces(list.clone()));
    }
}

/// A session is over: tell its watchers and every sidebar, then let the
/// session go -- outside the lock, since ending a shell can take a moment --
/// and stop the server after the last.
fn end(shared: &Shared, mut sessions: std::sync::MutexGuard<'_, BTreeMap<SessionId, Session>>, id: SessionId) {
    let Some(s) = sessions.remove(&id) else { return };
    // Its pane goes, and its sibling takes the room; a tab with no pane left goes too.
    let mut workspaces = lock(&shared.workspaces);
    let holder = workspaces.values().find(|w| w.layout.contains(&id)).map(|w| w.id);
    if let Some(wid) = holder {
        let w = workspaces.remove(&wid).expect("just found");
        if let Some(layout) = w.layout.remove(&id) {
            let focus = if w.focus == id { layout.leaves()[0] } else { w.focus };
            workspaces.insert(wid, Workspace { id: wid, layout, focus });
        }
    }
    broadcast_workspaces(shared, &workspaces);
    drop(workspaces);
    let list: Vec<Info> = sessions.values().map(|s| s.info.clone()).collect();
    let empty = sessions.is_empty();
    drop(sessions);
    for (c, tx) in lock(&shared.clients).iter() {
        if s.watchers.contains(c) {
            let _ = tx.send(ToClient::Exited { id });
        }
        let _ = tx.send(ToClient::Sessions(list.clone()));
    }
    drop(s);
    if empty {
        let _ = shared.done.try_send(());
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Work out a session's state again; true when it changed.
///
/// In order of how sure each sign is (QUESTIONS.md Q2): an agent's own word
/// (`tsumugi notify`, OSC 9 / 99 / 777) since the last key; the shell's
/// prompt back (OSC 133) since the last key with nothing running under it;
/// nothing running and nothing said for two seconds, which is a shell sitting
/// at its prompt; and output stopped for `quiet` while a program runs and
/// has written since the last key -- the guess, marked more faintly.
fn settle(s: &mut Session, quiet: Duration) -> bool {
    let input = s.term.last_input();
    let after_input = |t: Option<std::time::Instant>| t.is_some_and(|t| input.is_none_or(|i| t > i));
    let busy = s.term.busy();
    let state = match s.notice {
        Some((state, at)) if after_input(Some(at)) => state,
        _ if !busy && after_input(s.term.last_prompt()) => State::Done,
        _ if !busy && s.term.quiet_for(Duration::from_secs(2)) => State::Done,
        _ if busy && !quiet.is_zero() && s.term.quiet_for(quiet) && after_input(s.term.last_output()) => State::MaybeWaiting,
        _ => State::Running,
    };
    if state == s.info.state {
        return false;
    }
    if state == State::Running {
        s.info.note.clear();
    }
    s.info.state = state;
    s.info.since_ms = now_ms();
    true
}

/// Turn "session N changed" into messages, at most one screen per session
/// every few milliseconds however fast its shell writes.
fn run_pump(shared: Arc<Shared>, dirty: Receiver<SessionId>) {
    let mut last_settle = std::time::Instant::now();
    loop {
        let mut ids = BTreeSet::new();
        match dirty.recv_timeout(Duration::from_millis(500)) {
            Ok(first) => {
                std::thread::sleep(Duration::from_millis(8));
                ids.insert(first);
                ids.extend(dirty.try_iter());
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        let mut sessions = lock(&shared.sessions);
        // Once a second, every session's state again: the guesses are about
        // time passing, which no output announces.
        if last_settle.elapsed() >= Duration::from_secs(1) {
            last_settle = std::time::Instant::now();
            let mut changed = false;
            for s in sessions.values_mut() {
                changed |= settle(s, shared.quiet);
            }
            if changed {
                broadcast(&shared, &sessions);
            }
        }
        let mut ended = Vec::new();
        for id in ids {
            let Some(s) = sessions.get_mut(&id) else { continue };
            let clipboard = s.term.drain();
            let noticed = s.term.take_notices().pop().map(|note| {
                s.notice = Some((State::Waiting, std::time::Instant::now()));
                s.info.note = note;
            }).is_some();
            let before = (s.info.title.clone(), s.info.cwd.clone());
            s.info.title = s.term.title.clone();
            // Where the shell says it is (OSC 7), when it says so.
            if let Some(cwd) = &s.term.shell_cwd {
                s.info.cwd = cwd.clone();
            }
            // A notification settles the state at once; the rest waits for
            // the once-a-second pass, since asking whether anything runs under
            // the shell reads the process table.
            let changed = (before != (s.info.title.clone(), s.info.cwd.clone())) | (noticed && settle(s, shared.quiet));
            if s.term.exited {
                ended.push(id);
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
            drop(clients);
            if changed {
                broadcast(&shared, &sessions);
            }
        }
        for id in ended {
            end(&shared, sessions, id);
            sessions = lock(&shared.sessions);
        }
    }
}
