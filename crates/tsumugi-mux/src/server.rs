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
    /// Those of them that have nothing yet and get every row next.
    fresh: BTreeSet<ClientId>,
    /// The screen the watchers have, which the next update is the change from.
    sent: Option<(tsumugi_pane::Screen, crate::diff::Extra)>,
    /// What was started, for starting it again after a restart.
    shell: Option<(String, Vec<String>)>,
    /// The Claude Code conversation its hooks last named.
    claude: Option<String>,
    /// A line to type once the shell is ready: `claude --resume` in a
    /// restored session.
    pending: Option<Vec<u8>>,
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
    /// The state file (`state.rs`), and when it is next to be written.
    state: Option<PathBuf>,
    save_due: Mutex<Option<std::time::Instant>>,
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
                _ => break,
            }
        }
        // The state is written a moment after the last session ends, not at
        // once: a machine shutting down ends every shell before it ends the
        // server, and writing then would forget the tabs it should restore.
        let deadline = std::time::Instant::now() + SAVE_AFTER_END + Duration::from_secs(2);
        while lock(&self.shared.save_due).is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Whether the server has stopped, without waiting.
    pub fn stopped(&self) -> bool {
        !self.done.is_empty()
    }
}

/// Start a server listening at `at`, on threads of its own.
/// How a server runs.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Where to keep the tabs across a restart; `None` keeps nothing.
    pub state: Option<PathBuf>,
}

/// Start this user's server: listening at `at`, keeping its state where
/// `state::default_path` says.
pub fn start(at: &Address) -> io::Result<ServerHandle> {
    start_with(at, Options { state: crate::state::default_path() })
}

pub fn start_with(at: &Address, options: Options) -> io::Result<ServerHandle> {
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
        state: options.state,
        save_due: Mutex::new(None),
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
        s.fresh.remove(&client);
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
            save_soon(shared, SAVE_AFTER_CHANGE);
        }
        ToServer::Restore => {
            let n = if sessions.is_empty() { restore(shared, &mut sessions) } else { 0 };
            let _ = tx.send(ToClient::Restored(n));
        }
        ToServer::Spawn { cwd, shell, size, cell, place } => {
            match spawn_session(shared, &mut sessions, Some(client), cwd, shell, size, cell) {
                Ok(id) => {
                    let mut workspaces = lock(&shared.workspaces);
                    place_session(shared, &mut workspaces, id, place);
                    broadcast_workspaces(shared, &workspaces);
                    drop(workspaces);
                    let _ = tx.send(ToClient::Spawned { id });
                    broadcast(shared, &sessions);
                    let _ = shared.dirty.send(id);
                    save_soon(shared, SAVE_AFTER_CHANGE);
                }
                Err(e) => {
                    let _ = tx.send(ToClient::Error(format!("the shell did not start: {e}")));
                }
            }
        }
        ToServer::Attach { id } => match sessions.get_mut(&id) {
            Some(s) => {
                s.watchers.insert(client);
                s.fresh.insert(client);
                let _ = shared.dirty.send(id);
            }
            None => {
                let _ = tx.send(ToClient::Exited { id });
            }
        },
        ToServer::Detach { id } => {
            if let Some(s) = sessions.get_mut(&id) {
                s.watchers.remove(&client);
                s.fresh.remove(&client);
            }
        }
        ToServer::Kill { id } => end(shared, sessions, id),
        ToServer::Notify { id, state, note, claude } => {
            if let Some(s) = sessions.get_mut(&id) {
                if claude.is_some() && claude != s.claude {
                    s.claude = claude;
                    save_soon(shared, SAVE_AFTER_CHANGE);
                }
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

/// How long after a change the state is written: soon after an ordinary one,
/// later after a session ends (see `ServerHandle::wait`).
const SAVE_AFTER_CHANGE: Duration = Duration::from_millis(500);
const SAVE_AFTER_END: Duration = Duration::from_secs(3);

/// Ask for the state to be written `after` from now, unless it already is to
/// be written sooner.
fn save_soon(shared: &Shared, after: Duration) {
    if shared.state.is_none() {
        return;
    }
    let at = std::time::Instant::now() + after;
    let mut due = lock(&shared.save_due);
    if due.is_none_or(|d| d > at) {
        *due = Some(at);
    }
}

/// Write the state now, if it is due.
fn save_if_due(shared: &Shared) {
    let due = *lock(&shared.save_due);
    let (Some(path), Some(at)) = (&shared.state, due) else { return };
    if std::time::Instant::now() < at {
        return;
    }
    let sessions = lock(&shared.sessions);
    let workspaces = lock(&shared.workspaces);
    let saved = crate::state::Saved {
        workspaces: workspaces
            .values()
            .map(|w| crate::state::SavedWorkspace {
                layout: w.layout.clone(),
                focus: w.focus,
                panes: w
                    .layout
                    .leaves()
                    .into_iter()
                    .filter_map(|id| sessions.get(&id).map(|s| (id, s)))
                    .map(|(id, s)| crate::state::SavedPane { id, cwd: s.info.cwd.clone(), shell: s.shell.clone(), claude: s.claude.clone() })
                    .collect(),
            })
            .collect(),
    };
    drop(workspaces);
    drop(sessions);
    let _ = crate::state::store(path, &saved);
    *lock(&shared.save_due) = None;
}

/// Start a shell in a session of its own. `client`, when there is one, is
/// watching it from the start.
fn spawn_session(
    shared: &Arc<Shared>,
    sessions: &mut BTreeMap<SessionId, Session>,
    client: Option<ClientId>,
    cwd: PathBuf,
    shell: Option<(String, Vec<String>)>,
    size: tsumugi_pane::Size,
    cell: (u16, u16),
) -> io::Result<SessionId> {
    let id = shared.next.fetch_add(1, Ordering::Relaxed);
    let command = shell.as_ref().map_or_else(tsumugi_pane::default_program, |(p, _)| p.clone());
    let dirty = shared.dirty.clone();
    let log = std::env::var_os("TSUMUGI_PTY_LOG").map(PathBuf::from);
    let env = vec![
        ("TSUMUGI_SESSION".to_owned(), id.to_string()),
        ("TSUMUGI_ADDRESS".to_owned(), shared.address.to_string_lossy().into_owned()),
    ];
    let mut term = Terminal::spawn_with_env(&cwd, size, cell, shell.clone(), log.as_deref(), env, move || {
        let _ = dirty.send(id);
    })?;
    shared.ever.store(true, Ordering::Relaxed);
    // What a program asking for the colours (OSC 10 / 11) is told: the
    // window's default palette, filer's colours.
    term.set_colors([0xc8, 0xcd, 0xd8], [0x1b, 0x1e, 0x24]);
    let info = Info { id, cwd, title: String::new(), command, state: State::Running, note: String::new(), since_ms: now_ms() };
    let watchers: BTreeSet<ClientId> = client.into_iter().collect();
    sessions.insert(
        id,
        Session { term, info, notice: None, fresh: watchers.clone(), watchers, sent: None, shell, claude: None, pending: None },
    );
    Ok(id)
}

/// Start again the tabs the state file has, after a restart: each pane's
/// shell in its folder (the home folder when that is gone), and Claude Code
/// resumed where it ran. The number of sessions started.
fn restore(shared: &Arc<Shared>, sessions: &mut BTreeMap<SessionId, Session>) -> usize {
    let Some(saved) = shared.state.as_deref().and_then(crate::state::load) else { return 0 };
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut started = 0;
    let mut workspaces = lock(&shared.workspaces);
    for w in saved.workspaces {
        let mut ids = BTreeMap::new();
        for p in &w.panes {
            let cwd = if p.cwd.is_dir() { p.cwd.clone() } else { home.clone().unwrap_or_else(|| p.cwd.clone()) };
            let Ok(id) = spawn_session(shared, sessions, None, cwd, p.shell.clone(), tsumugi_pane::Size::new(80, 24), (8, 16)) else {
                continue;
            };
            if let Some(conversation) = &p.claude {
                let s = sessions.get_mut(&id).expect("just started");
                s.claude = Some(conversation.clone());
                s.pending = Some(format!("claude --resume {conversation}\r").into_bytes());
            }
            ids.insert(p.id, id);
            started += 1;
        }
        let Some(layout) = w.layout.retain(&|old| ids.contains_key(old)) else { continue };
        let layout = layout.map(&mut |old| ids[&old]);
        let focus = ids.get(&w.focus).copied().unwrap_or_else(|| layout.leaves()[0]);
        let ws = shared.next.fetch_add(1, Ordering::Relaxed);
        workspaces.insert(ws, Workspace { id: ws, layout, focus });
    }
    broadcast_workspaces(shared, &workspaces);
    drop(workspaces);
    broadcast(shared, sessions);
    started
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
    save_soon(shared, SAVE_AFTER_END);
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
                // Where the shell is now, for the sidebar and for a restore:
                // a bash that never says (no OSC 7) still has a folder.
                if let Some(cwd) = s.term.current_dir().filter(|c| *c != s.info.cwd) {
                    s.info.cwd = cwd;
                    changed = true;
                    save_soon(&shared, SAVE_AFTER_CHANGE);
                }
                // A restored session's `claude --resume`, once the shell
                // has shown its prompt (or gone quiet after its first output).
                let ready = s.term.prompt_seen() || (s.term.last_output().is_some() && s.term.quiet_for(Duration::from_millis(800)));
                if ready {
                    if let Some(line) = s.pending.take() {
                        s.term.send(line);
                    }
                }
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
            if before.1 != s.info.cwd {
                save_soon(&shared, SAVE_AFTER_CHANGE);
            }
            let changed = (before != (s.info.title.clone(), s.info.cwd.clone())) | (noticed && settle(s, shared.quiet));
            if s.term.exited {
                ended.push(id);
                continue;
            }
            // Only what changed since the watchers' copy; a watcher that has
            // no copy yet gets every row.
            let screen = s.term.screen();
            let extra = crate::diff::Extra {
                scrolled_back: s.term.scrolled_back(),
                win32_input: s.term.win32_input(),
                title: s.term.title.clone(),
            };
            let change = crate::diff::diff(s.sent.as_ref().map(|(sc, ex)| (sc, ex)), &screen, &extra);
            let whole = (!s.fresh.is_empty()).then(|| crate::diff::diff(None, &screen, &extra)).flatten();
            let clients = lock(&shared.clients);
            for c in &s.watchers {
                if let Some(tx) = clients.get(c) {
                    for text in &clipboard {
                        let _ = tx.send(ToClient::Clipboard(text.clone()));
                    }
                    let update = if s.fresh.contains(c) { whole.clone() } else { change.clone() };
                    if let Some(update) = update {
                        let _ = tx.send(ToClient::Screen { id, update });
                    }
                }
            }
            drop(clients);
            s.fresh.clear();
            s.sent = Some((screen, extra));
            if changed {
                broadcast(&shared, &sessions);
            }
        }
        for id in ended {
            end(&shared, sessions, id);
            sessions = lock(&shared.sessions);
        }
        drop(sessions);
        save_if_due(&shared);
    }
}
