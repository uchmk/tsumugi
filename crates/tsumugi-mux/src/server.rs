//! The server: it owns the sessions, each a [`Terminal`], and sends each
//! attached client the screen of the sessions it watches whenever one changes.
//!
//! Threads: one accepting clients, two per client (reading its requests and
//! writing to it), and one pump that turns a session's "something happened"
//! into the messages for its watchers. The sessions sit behind one lock, held
//! briefly.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use tsumugi_pane::{Pane, Terminal};

use crate::frame;
use crate::proto::{Info, Notice, Place, ScrollBy, SessionId, State, ToClient, ToServer, Workspace, WorkspaceId, VERSION};
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
    /// When the wait last sent to the webhook began (`Info::since_ms`).
    webhooked: Option<u64>,
    /// The screen's text, hashed, and since when it has read so: a program
    /// that keeps writing the same screen (a redrawn prompt, a cursor shown
    /// and hidden) is still quiet to a person.
    screen: Option<(u64, std::time::Instant)>,
}

struct Shared {
    sessions: Mutex<BTreeMap<SessionId, Session>>,
    /// The tabs and their splits. Locked after `sessions` when both are.
    workspaces: Mutex<BTreeMap<WorkspaceId, Workspace>>,
    clients: Mutex<BTreeMap<ClientId, Sender<ToClient>>>,
    /// Which windows have the keyboard, and the order they last had it in.
    /// Locked after `sessions`, before `clients`.
    attention: Mutex<Attention>,
    /// The tabs in the order they were dragged into; a tab not in it goes
    /// after, by id. Locked after `workspaces`.
    order: Mutex<Vec<WorkspaceId>>,
    /// Tags muted (`ToServer::MuteTag`), kept across a restart.
    muted_tags: Mutex<BTreeSet<String>>,
    next: AtomicU64,
    /// A session to look at: its shell said something, or a client asked.
    dirty: Sender<SessionId>,
    /// Told once, when the last session has ended.
    done: Sender<()>,
    /// A session has been started at some point.
    ever: std::sync::atomic::AtomicBool,
    /// Asked to stop (`STOP`): leave at once, the tabs already written.
    stopping: std::sync::atomic::AtomicBool,
    /// Where this server listens, told to its shells (`TSUMUGI_ADDRESS`) so
    /// that `tsumugi notify` inside them finds it.
    address: PathBuf,
    /// The bell's list, oldest first, and the next notice's id.
    notices: Mutex<(u64, std::collections::VecDeque<Notice>)>,
    /// When the server started, in Unix milliseconds.
    started_ms: u64,
    /// The state file (`state.rs`), and when it is next to be written.
    state: Option<PathBuf>,
    save_due: Mutex<Option<std::time::Instant>>,
    /// The settings file and the tag rules read from it. Locked after
    /// `sessions`.
    settings: Option<PathBuf>,
    rules: Mutex<Rules>,
    /// Seconds of silence after which a running program reads as probably
    /// waiting (Q2: Konsole's 10). 0 turns the guess off.
    quiet: Duration,
    /// Each folder's branch and repository as last read, and the folders
    /// to read, by a thread of its own: `.git` on a slow share is never
    /// read while `sessions` is held (the source review, 2026-10-07).
    git: Mutex<BTreeMap<PathBuf, (String, PathBuf)>>,
    git_ask: crossbeam_channel::Sender<PathBuf>,
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
        if self.shared.stopping.load(Ordering::Relaxed) {
            return;
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
    /// The settings file, for its tag rules; `None` reads none.
    pub settings: Option<PathBuf>,
}

/// Start this user's server: listening at `at`, keeping its state where
/// `state::default_path` says.
pub fn start(at: &Address) -> io::Result<ServerHandle> {
    start_with(at, Options { state: crate::state::default_path(), settings: crate::settings::default_path() })
}

pub fn start_with(at: &Address, options: Options) -> io::Result<ServerHandle> {
    let listener = Listener::bind(at)?;
    let (dirty_tx, dirty_rx) = crossbeam_channel::unbounded();
    let (done_tx, done_rx) = crossbeam_channel::bounded(1);
    let (git_tx, git_rx) = crossbeam_channel::unbounded::<PathBuf>();
    let shared = Arc::new(Shared {
        sessions: Mutex::new(BTreeMap::new()),
        workspaces: Mutex::new(BTreeMap::new()),
        clients: Mutex::new(BTreeMap::new()),
        attention: Mutex::new(Attention::default()),
        order: Mutex::new(Vec::new()),
        muted_tags: Mutex::new(BTreeSet::new()),
        next: AtomicU64::new(1),
        dirty: dirty_tx,
        done: done_tx,
        ever: false.into(),
        stopping: false.into(),
        address: at.0.clone(),
        state: options.state,
        notices: Mutex::new((1, std::collections::VecDeque::new())),
        started_ms: now_ms(),
        save_due: Mutex::new(None),
        rules: Mutex::new(Rules::read(options.settings.as_deref())),
        settings: options.settings,
        quiet: Duration::from_secs(
            std::env::var("TSUMUGI_QUIET_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10),
        ),
        git: Mutex::new(BTreeMap::new()),
        git_ask: git_tx,
    });
    let reader = shared.clone();
    std::thread::Builder::new().name("mux-git".into()).spawn(move || {
        for dir in git_rx {
            let found = git_info(&dir);
            lock(&reader.git).insert(dir, found);
            let _ = reader.dirty.send(0);
        }
    })?;
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
        Ok(ToServer::Hello { version: crate::proto::STOP }) => {
            // Asked to stop: the tabs on disk first, as they are now.
            *lock(&shared.save_due) = Some(std::time::Instant::now());
            save_if_due(&shared);
            shared.stopping.store(true, Ordering::Relaxed);
            let _ = shared.done.try_send(());
            return;
        }
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
    let _ = tx.send(ToClient::Started { at_ms: shared.started_ms });
    let _ = tx.send(ToClient::Notices(lock(&shared.notices).1.iter().cloned().collect()));
    let _ = tx.send(ToClient::MutedTags(lock(&shared.muted_tags).iter().cloned().collect()));
    lock(&shared.clients).insert(client, tx.clone());
    // Last in the line to tell: a window that never had the keyboard tells
    // only when it is the only one.
    lock(&shared.attention).order.insert(0, client);
    broadcast_attention(shared.as_ref());
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
    {
        let mut a = lock(&shared.attention);
        a.order.retain(|c| *c != client);
        a.focused.remove(&client);
    }
    broadcast_attention(shared.as_ref());
    for s in lock(&shared.sessions).values_mut() {
        s.watchers.remove(&client);
        s.fresh.remove(&client);
    }
    drop(tx);
    let _ = writer.join();
}

/// How many lines a search across sessions gives back from one, and in all.
const SEARCH_PER_SESSION: usize = 30;
const SEARCH_MOST: usize = 200;

fn handle(shared: &Arc<Shared>, client: ClientId, tx: &Sender<ToClient>, msg: ToServer) {
    let mut sessions = lock(&shared.sessions);
    match msg {
        ToServer::Hello { .. } => {}
        ToServer::SetCharset { id, name } => {
            if let Some(s) = sessions.get_mut(&id) {
                if s.term.set_charset(&name) {
                    s.info.charset = s.term.charset().to_owned();
                    broadcast(shared, &sessions);
                    save_soon(shared, SAVE_AFTER_CHANGE);
                }
            }
        }
        ToServer::AllText { id } => {
            if let Some(s) = sessions.get(&id) {
                let _ = tx.send(ToClient::Text { id, text: s.term.all_text() });
            }
        }
        ToServer::SearchAll { query } => {
            // Each session's newest first, the newest sessions' first.
            let mut hits = Vec::new();
            let mut ids: Vec<&SessionId> = sessions.keys().collect();
            ids.sort_by(|a, b| b.cmp(a));
            for id in ids {
                let s = &sessions[id];
                for (line, col, text) in s.term.find_lines(&query, SEARCH_PER_SESSION) {
                    hits.push(crate::proto::Hit { id: *id, line, col, text });
                }
                if hits.len() >= SEARCH_MOST {
                    hits.truncate(SEARCH_MOST);
                    break;
                }
            }
            let _ = tx.send(ToClient::FoundAll { query, hits });
        }
        ToServer::List => {
            let workspaces = lock(&shared.workspaces);
            let _ = tx.send(ToClient::Workspaces(ordered(shared, &workspaces).into_iter().cloned().collect()));
            drop(workspaces);
            let _ = tx.send(ToClient::Listed(sessions.values().map(|s| s.info.clone()).collect()));
        }
        ToServer::SetLayout { id, layout, focus } => {
            // Only panes that are still sessions: the window may have sent
            // this before it heard that one ended. And only panes of no
            // other tab, each once: a window that had not heard a pane
            // moved could otherwise put one session in two tabs (the
            // source review, 2026-10-07).
            let mut workspaces = lock(&shared.workspaces);
            let elsewhere: std::collections::HashSet<SessionId> = workspaces.iter().filter(|(k, _)| **k != id).flat_map(|(_, w)| w.layout.leaves()).collect();
            let leaves = layout.leaves();
            let twice = leaves.iter().enumerate().any(|(k, s)| leaves[..k].contains(s));
            let layout = if twice { None } else { layout.retain(&|s| sessions.contains_key(s) && !elsewhere.contains(s)) };
            if let (Some(w), Some(layout)) = (workspaces.get_mut(&id), layout) {
                w.focus = if layout.contains(&focus) { focus } else { layout.leaves()[0] };
                w.layout = layout;
            }
            broadcast_workspaces(shared, &workspaces);
            save_soon(shared, SAVE_AFTER_CHANGE);
        }
        ToServer::Saved => {
            let saved = shared.state.as_deref().and_then(crate::state::load);
            let _ = tx.send(ToClient::Saved(saved));
        }
        ToServer::Restore { only } => {
            let n = if sessions.is_empty() { restore(shared, &mut sessions, only.as_deref()) } else { 0 };
            let _ = tx.send(ToClient::Restored(n));
        }
        ToServer::ReadNotices { ids } => {
            let mut notices = lock(&shared.notices);
            for n in notices.1.iter_mut().filter(|n| ids.as_ref().is_none_or(|ids| ids.contains(&n.id))) {
                n.read = true;
            }
            broadcast_notices(shared, &notices.1);
        }
        ToServer::Focus { focused } => {
            {
                let mut a = lock(&shared.attention);
                if focused {
                    a.focused.insert(client);
                    a.order.retain(|c| *c != client);
                    a.order.push(client);
                } else {
                    a.focused.remove(&client);
                }
            }
            broadcast_attention(shared);
        }
        ToServer::Mute { ids, on } => {
            for id in ids {
                if let Some(s) = sessions.get_mut(&id) {
                    s.info.muted = on;
                }
            }
            broadcast(shared, &sessions);
            save_soon(shared, SAVE_AFTER_CHANGE);
        }
        ToServer::OwnTab { id } => {
            let mut workspaces = lock(&shared.workspaces);
            let holder = workspaces.values().find(|w| w.layout.contains(&id) && w.layout.leaves().len() > 1).map(|w| w.id);
            if let Some(wid) = holder {
                let w = workspaces.remove(&wid).expect("just found");
                if let Some(layout) = w.layout.remove(&id) {
                    let focus = if w.focus == id { layout.leaves()[0] } else { w.focus };
                    workspaces.insert(wid, Workspace { layout, focus, ..w });
                }
                let ws = shared.next.fetch_add(1, Ordering::Relaxed);
                workspaces.insert(ws, Workspace::new(ws, tsumugi_layout::Node::Leaf(id), id));
                // Right after the tab it came from.
                let mut now: Vec<WorkspaceId> = ordered(shared, &workspaces).iter().map(|w| w.id).filter(|w| *w != ws).collect();
                let at = now.iter().position(|w| *w == wid).map_or(now.len(), |k| k + 1);
                now.insert(at, ws);
                *lock(&shared.order) = now;
                broadcast_workspaces(shared, &workspaces);
                save_soon(shared, SAVE_AFTER_CHANGE);
            }
        }
        ToServer::MoveWorkspace { id, to } => {
            let workspaces = lock(&shared.workspaces);
            let mut now: Vec<WorkspaceId> = ordered(shared, &workspaces).iter().map(|w| w.id).collect();
            if let Some(from) = now.iter().position(|w| *w == id) {
                now.remove(from);
                now.insert(to.min(now.len()), id);
                *lock(&shared.order) = now;
                broadcast_workspaces(shared, &workspaces);
                save_soon(shared, SAVE_AFTER_CHANGE);
            }
        }
        ToServer::RenameWorkspace { id, name } => {
            let mut workspaces = lock(&shared.workspaces);
            if let Some(w) = workspaces.get_mut(&id) {
                w.name = name.trim().chars().take(80).collect();
                broadcast_workspaces(shared, &workspaces);
                save_soon(shared, SAVE_AFTER_CHANGE);
            }
        }
        ToServer::NoteWorkspace { id, note } => {
            let mut workspaces = lock(&shared.workspaces);
            if let Some(w) = workspaces.get_mut(&id) {
                w.note = note.trim().chars().take(200).collect();
                broadcast_workspaces(shared, &workspaces);
                save_soon(shared, SAVE_AFTER_CHANGE);
            }
        }
        ToServer::PinWorkspace { id, on } => {
            let mut workspaces = lock(&shared.workspaces);
            if let Some(w) = workspaces.get_mut(&id) {
                w.pinned = on;
                broadcast_workspaces(shared, &workspaces);
                save_soon(shared, SAVE_AFTER_CHANGE);
            }
        }
        ToServer::Restart { id } => {
            let Some(s) = sessions.get_mut(&id) else { return };
            match start_terminal(shared, id, &s.info.cwd.clone(), s.term.size(), (8, 16), s.shell.clone()) {
                Ok(term) => {
                    // The character set stays the session's.
                    term.set_charset(&s.info.charset);
                    let old = std::mem::replace(&mut s.term, term);
                    // Every watcher gets the whole new screen.
                    s.sent = None;
                    s.fresh.clone_from(&s.watchers);
                    s.notice = None;
                    s.info.state = State::Running;
                    s.info.since_ms = now_ms();
                    s.info.note.clear();
                    s.pending = lock(&shared.rules).take_up(s.claude.as_deref(), &s.info.agent);
                    // Ending a shell can take a moment: not under the lock.
                    std::thread::spawn(move || drop(old));
                    broadcast(shared, &sessions);
                    let _ = shared.dirty.send(id);
                }
                Err(e) => {
                    let _ = tx.send(ToClient::Error(format!("the shell did not start again: {e}")));
                }
            }
        }
        ToServer::Tag { ids, tag, on } => {
            let Some(tag) = crate::proto::tag_name(&tag) else { return };
            for id in ids {
                let Some(s) = sessions.get_mut(&id) else { continue };
                let tags = &mut s.info.tags;
                if !on {
                    tags.retain(|t| *t != tag);
                } else if !tags.contains(&tag) && tags.len() < crate::proto::MAX_TAGS {
                    tags.push(tag.clone());
                }
            }
            broadcast(shared, &sessions);
            save_soon(shared, SAVE_AFTER_CHANGE);
        }
        ToServer::MuteTag { tag, on } => {
            let Some(tag) = crate::proto::tag_name(&tag) else { return };
            let list: Vec<String> = {
                let mut muted = lock(&shared.muted_tags);
                if on {
                    muted.insert(tag);
                } else {
                    muted.remove(&tag);
                }
                muted.iter().cloned().collect()
            };
            for tx in lock(&shared.clients).values() {
                let _ = tx.send(ToClient::MutedTags(list.clone()));
            }
            save_soon(shared, SAVE_AFTER_CHANGE);
        }
        ToServer::Spawn { cwd, shell, size, cell, place, typed } => {
            match spawn_session(shared, &mut sessions, Some(client), cwd, shell, size, cell) {
                Ok(id) => {
                    if let (Some(line), Some(s)) = (typed, sessions.get_mut(&id)) {
                        s.pending = Some(format!("{line}\r").into_bytes());
                    }
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
            // A conversation is typed back into the shell as `claude
            // --resume <id>`: only an id of letters, digits, `-` and `_`
            // is kept (the source review, 2026-10-07).
            let claude = claude.filter(|c| conversation_id(c));
            if let Some(s) = sessions.get_mut(&id) {
                if claude.is_some() && claude != s.claude {
                    s.info.conversation = claude.clone().unwrap_or_default();
                    s.claude = claude;
                    s.info.claude = true;
                    save_soon(shared, SAVE_AFTER_CHANGE);
                }
                s.notice = Some((state, std::time::Instant::now()));
                if !note.is_empty() {
                    s.info.note = note;
                }
                if let Some(before) = settle(s, quiet_of(shared)) {
                    record_notice(shared, s, before);
                    broadcast(shared, &sessions);
                }
            }
        }
        other => {
            let Some(id) = target(&other) else { return };
            let Some(s) = sessions.get_mut(&id) else { return };
            let term = &mut s.term;
            match other {
                ToServer::Input { bytes, .. } => {
                    term.send(bytes);
                    read_notices_of(shared, id);
                }
                ToServer::Paste { text, .. } => term.paste(&text),
                ToServer::Resize { size, cell, .. } => Pane::resize(term, size, cell),
                ToServer::Scroll { by: ScrollBy::Prompt { back }, .. } => {
                    term.jump_prompt(back);
                }
                ToServer::Scroll { by, .. } => term.scroll(scroll(by)),
                ToServer::Select { cell, right_half, start, .. } => term.select(cell, right_half, start),
                ToServer::SelectWord { cell, .. } => term.select_word(cell),
                ToServer::ClearSelection { .. } => term.clear_selection(),
                ToServer::Reveal { line, col, len, .. } => term.reveal(line, col, len),
                ToServer::Find { needle, back, .. } => {
                    if needle.is_empty() {
                        term.end_search();
                    } else {
                        let wrapped = term.search(&tsumugi_pane::plain_pattern(&needle), back);
                        let _ = tx.send(ToClient::Found { id, wrapped });
                    }
                }
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
        | ToServer::Reveal { id, .. }
        | ToServer::Find { id, .. }
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
        ScrollBy::Bottom | ScrollBy::Prompt { .. } => Scroll::Bottom,
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

/// What the server takes from the settings -- the tag rules, the shell,
/// how sessions are watched and resumed -- and when the file was read.
struct Rules {
    stamp: Option<std::time::SystemTime>,
    tags: Vec<crate::settings::TagRule>,
    home: Option<PathBuf>,
    /// `[sessions] quiet`, when set (else `Shared::quiet`).
    quiet: Option<Duration>,
    /// `[notify] long_run`, in milliseconds.
    long_run: u64,
    /// `[sessions] resume` and `claude`: what a restored Claude Code pane types.
    resume: bool,
    claude: String,
    shell: crate::settings::Shell,
    scrollback: usize,
    pane_log: bool,
    /// `[notify] webhook`, `webhook_format` and `webhook_after` (ms).
    webhook: String,
    webhook_format: String,
    webhook_after: u64,
    /// The AI programs told apart from a shell, and the settings' `[agents]`
    /// (the rest left default) to take one up again after a restart.
    agent_names: Vec<String>,
    settings: crate::settings::Settings,
}

impl Rules {
    /// A file that cannot be read gives the defaults; the window says what
    /// is wrong with it.
    fn read(path: Option<&Path>) -> Self {
        let stamp = path.and_then(crate::settings::stamp);
        let s = path.and_then(|p| crate::settings::load(p).ok()).unwrap_or_default();
        let agent_names = s.agent_names();
        let agents = crate::settings::Settings { agents: s.agents.clone(), ..Default::default() };
        Self {
            stamp,
            tags: s.tags.rule,
            home: crate::settings::home(),
            quiet: s.sessions.quiet.map(Duration::from_secs),
            long_run: s.notify.long_run.saturating_mul(1000),
            resume: s.sessions.resume,
            claude: if s.sessions.claude.trim().is_empty() { "claude".into() } else { s.sessions.claude.trim().to_owned() },
            shell: s.shell,
            scrollback: s.advanced.scrollback,
            pane_log: s.advanced.pane_log,
            webhook: s.notify.webhook.trim().to_owned(),
            webhook_format: s.notify.webhook_format.clone(),
            webhook_after: s.notify.webhook_after.saturating_mul(1000),
            agent_names,
            settings: agents,
        }
    }

    /// What a restored or restarted session types: Claude Code's
    /// conversation resumed, else the other AI program's own line.
    fn take_up(&self, claude: Option<&str>, agent: &str) -> Option<Vec<u8>> {
        if let Some(c) = claude {
            return self.resume_line(c);
        }
        self.resume.then(|| self.settings.resume_for(agent)).flatten().map(|l| format!("{l}\r").into_bytes())
    }

    /// The line a restored or restarted Claude Code pane types, if any.
    fn resume_line(&self, conversation: &str) -> Option<Vec<u8>> {
        (self.resume && conversation_id(conversation)).then(|| format!("{} --resume {conversation}\r", self.claude).into_bytes())
    }

    /// The tags the rules give a session in `cwd` on `branch`.
    fn given(&self, cwd: &Path, branch: &str) -> Vec<String> {
        self.tags.iter().filter_map(|r| r.tag_for_session(cwd, branch, self.home.as_deref())).collect()
    }

    /// `info`'s tags after it moved: the ones the rules gave it where it was
    /// (`was`, the tags given there) and do not give it here are taken off,
    /// and this folder's put on. A tag added by hand stays, unless a rule
    /// gave the same one where it was.
    fn follow(&self, info: &mut Info, was: &[String]) -> bool {
        let now = self.given(&info.cwd, &info.branch);
        let before = info.tags.len();
        info.tags.retain(|t| !was.contains(t) || now.contains(t));
        let taken = info.tags.len() != before;
        self.apply(info) | taken
    }

    /// Put on `info` the tags its folder's and branch's rules give it. Only
    /// adds; `follow` takes the old folder's off when it moves. A tag taken
    /// off by hand comes back only when the folder changes.
    fn apply(&self, info: &mut Info) -> bool {
        let mut added = false;
        for t in self.tags.iter().filter_map(|r| r.tag_for_session(&info.cwd, &info.branch, self.home.as_deref())) {
            if !info.tags.contains(&t) && info.tags.len() < crate::proto::MAX_TAGS {
                info.tags.push(t);
                added = true;
            }
        }
        added
    }
}

/// Which windows have the keyboard (`ToServer::Focus`).
#[derive(Default)]
struct Attention {
    focused: BTreeSet<ClientId>,
    /// Every client, the one that had the keyboard last at the end: it is
    /// the one that tells.
    order: Vec<ClientId>,
}

fn broadcast_attention(shared: &Shared) {
    let (looking, teller) = {
        let a = lock(&shared.attention);
        (!a.focused.is_empty(), a.order.last().copied())
    };
    for (id, tx) in lock(&shared.clients).iter() {
        let _ = tx.send(ToClient::Attention { looking, teller: teller == Some(*id) });
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
    // One write at a time (the pump's and a STOP's), each of the latest.
    static ONE_WRITE: Mutex<()> = Mutex::new(());
    let _turn = lock(&ONE_WRITE);
    let sessions = lock(&shared.sessions);
    let workspaces = lock(&shared.workspaces);
    // Not due any more from here: a change made while the file is being
    // written asks for another save, which clearing afterwards would have
    // thrown away (the source review, 2026-10-07).
    *lock(&shared.save_due) = None;
    let saved = crate::state::Saved {
        // In the sidebar's order, which a restore keeps.
        workspaces: ordered(shared, &workspaces)
            .into_iter()
            .map(|w| crate::state::SavedWorkspace {
                name: w.name.clone(),
                pinned: w.pinned,
                note: w.note.clone(),
                layout: w.layout.clone(),
                focus: w.focus,
                panes: w
                    .layout
                    .leaves()
                    .into_iter()
                    .filter_map(|id| sessions.get(&id).map(|s| (id, s)))
                    .map(|(id, s)| crate::state::SavedPane {
                        id,
                        cwd: s.info.cwd.clone(),
                        shell: s.shell.clone(),
                        claude: s.claude.clone(),
                        title: s.info.title.clone(),
                        state: s.info.state,
                        muted: s.info.muted,
                        tags: s.info.tags.clone(),
                        charset: s.info.charset.clone(),
                        agent: s.info.agent.clone(),
                    })
                    .collect(),
            })
            .collect(),
        at_ms: now_ms(),
        muted_tags: lock(&shared.muted_tags).iter().cloned().collect(),
    };
    drop(workspaces);
    drop(sessions);
    if let Err(e) = crate::state::store(path, &saved) {
        eprintln!("tsumugi: the tabs could not be written to {}: {e}", path.display());
        save_soon(shared, Duration::from_secs(5));
    }
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
    // No shell asked for: the settings' `[shell]`, else the system's.
    let shell = shell.or_else(|| lock(&shared.rules).shell.command());
    let command = shell.as_ref().map_or_else(tsumugi_pane::default_program, |(p, _)| p.clone());
    let term = start_terminal(shared, id, &cwd, size, cell, shell.clone())?;
    shared.ever.store(true, Ordering::Relaxed);
    let (branch, project) = git(&cwd);
    let branch = branch.unwrap_or_default();
    let mut info = Info { id, cwd, title: String::new(), command, state: State::Running, note: String::new(), since_ms: now_ms(), branch, project, muted: false, tags: Vec::new(), claude: false, conversation: String::new(), charset: "UTF-8".into(), agent: String::new(), ports: Vec::new() };
    lock(&shared.rules).apply(&mut info);
    let watchers: BTreeSet<ClientId> = client.into_iter().collect();
    sessions.insert(
        id,
        Session { term, info, notice: None, fresh: watchers.clone(), watchers, sent: None, shell, claude: None, pending: None, webhooked: None, screen: None },
    );
    Ok(id)
}

/// What PowerShell 7 runs after its profile so that it says where it is
/// (OSC 7) on each `cd`: on Windows the server cannot read a shell's folder
/// from the system, and without it a folder's tag rules never saw a `cd`.
/// It calls whatever handler was there before (a profile's own tsumugi hook,
/// `mise activate pwsh`), and says the folder it starts in. It also marks
/// each prompt (OSC 133;A) by wrapping the `prompt` the profile left, once
/// only: a profile's own tsumugi hook may have done it already.
#[cfg(any(windows, test))]
const PWSH_CWD_HOOK: &str = r#"$__tsumugi_prev = $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction
function global:__tsumugi_osc7($p) { [Console]::Write("$([char]27)]7;file://$(($p -replace '\\', '/') -replace '^(?!/)', '/')$([char]27)\") }
$ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = {
    param($sender, $e)
    if ($__tsumugi_prev) { $__tsumugi_prev.Invoke($sender, $e) }
    __tsumugi_osc7 $e.NewPath.ProviderPath
}.GetNewClosure()
__tsumugi_osc7 (Get-Location).ProviderPath
if (-not $global:__tsumugi_marked) {
    $global:__tsumugi_marked = $true
    $global:__tsumugi_prompt = $function:prompt
    function global:prompt { "$([char]27)]133;A$([char]27)\" + (& $global:__tsumugi_prompt) }
}
"#;

/// `pwsh`'s arguments with the hook after them, when it starts as an
/// interactive shell; one told to run something (`-Command`, `-File`) or
/// already given `-NoExit` is left as it is.
#[cfg(any(windows, test))]
fn hooked_args(program: &str, args: &[String]) -> Option<Vec<String>> {
    // By hand: a Windows path read on another system is one name.
    let name = program.rsplit(['/', '\\']).next()?.to_ascii_lowercase();
    if name != "pwsh" && name != "pwsh.exe" {
        return None;
    }
    let busy = ["-c", "-command", "-f", "-file", "-e", "-ec", "-encodedcommand", "-noexit", "-cwa", "-commandwithargs"];
    if args.iter().any(|a| busy.contains(&a.to_ascii_lowercase().as_str())) {
        return None;
    }
    let utf16: Vec<u8> = PWSH_CWD_HOOK.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = args.to_vec();
    out.extend(["-NoExit".to_owned(), "-EncodedCommand".to_owned(), base64(&utf16)]);
    Some(out)
}

#[cfg(windows)]
fn with_cwd_hook(shell: Option<(String, Vec<String>)>) -> Option<(String, Vec<String>)> {
    shell.map(|(p, a)| {
        let a = hooked_args(&p, &a).unwrap_or(a);
        (p, a)
    })
}

/// Standard base64, padded (what `-EncodedCommand` reads).
#[cfg(any(windows, test))]
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// A session's shell, told who it is (`TSUMUGI_SESSION`) and where the
/// server is, so that `tsumugi notify` inside it finds them.
fn start_terminal(
    shared: &Shared,
    id: SessionId,
    cwd: &Path,
    size: tsumugi_pane::Size,
    cell: (u16, u16),
    shell: Option<(String, Vec<String>)>,
) -> io::Result<Terminal> {
    let dirty = shared.dirty.clone();
    let (own, scrollback, pane_log) = {
        let r = lock(&shared.rules);
        (r.shell.clone(), r.scrollback, r.pane_log)
    };
    // `[advanced] pane_log`: a file a session beside the state, for bug reports.
    let log = std::env::var_os("TSUMUGI_PTY_LOG").map(PathBuf::from).or_else(|| {
        let dir = shared.state.as_deref()?.parent()?.join("pane-logs");
        (pane_log && std::fs::create_dir_all(&dir).is_ok()).then(|| dir.join(format!("session-{id}.log")))
    });
    // The settings' variables first: tsumugi's own win.
    let mut env: Vec<(String, String)> = own.env.clone().into_iter().collect();
    env.push(("TSUMUGI_SESSION".to_owned(), id.to_string()));
    env.push(("TSUMUGI_ADDRESS".to_owned(), shared.address.to_string_lossy().into_owned()));
    // No shell asked for: the settings' `[shell]`, else the system's.
    let shell = shell.or_else(|| own.command());
    #[cfg(windows)]
    let shell = with_cwd_hook(shell.or_else(|| tsumugi_pane::default_shell().map(|p| (p, Vec::new()))));
    let mut term = Terminal::spawn_with_env(cwd, size, cell, shell, log.as_deref(), env, move || {
        let _ = dirty.send(id);
    })?;
    term.set_scrollback(scrollback);
    // What a program asking for the colours (OSC 10 / 11) is told: the
    // window's default palette, filer's colours.
    term.set_colors([0xc8, 0xcd, 0xd8], [0x1b, 0x1e, 0x24]);
    Ok(term)
}

/// Start again the tabs the state file has, after a restart: each pane's
/// shell in its folder (the home folder when that is gone), and Claude Code
/// resumed where it ran. The number of sessions started.
fn restore(shared: &Arc<Shared>, sessions: &mut BTreeMap<SessionId, Session>, only: Option<&[SessionId]>) -> usize {
    let Some(saved) = shared.state.as_deref().and_then(crate::state::load) else { return 0 };
    let muted: Vec<String> = {
        let mut m = lock(&shared.muted_tags);
        m.extend(saved.muted_tags.iter().cloned());
        m.iter().cloned().collect()
    };
    for tx in lock(&shared.clients).values() {
        let _ = tx.send(ToClient::MutedTags(muted.clone()));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut started = 0;
    let mut workspaces = lock(&shared.workspaces);
    for w in saved.workspaces {
        let mut ids = BTreeMap::new();
        for p in w.panes.iter().filter(|p| only.is_none_or(|o| o.contains(&p.id))) {
            let cwd = if p.cwd.is_dir() { p.cwd.clone() } else { home.clone().unwrap_or_else(|| p.cwd.clone()) };
            let Ok(id) = spawn_session(shared, sessions, None, cwd, p.shell.clone(), tsumugi_pane::Size::new(80, 24), (8, 16)) else {
                continue;
            };
            let s = sessions.get_mut(&id).expect("just started");
            s.info.muted = p.muted;
            s.info.tags.clone_from(&p.tags);
            if s.term.set_charset(&p.charset) {
                s.info.charset = s.term.charset().to_owned();
            }
            s.info.claude = p.claude.is_some();
            s.info.conversation = p.claude.clone().unwrap_or_default();
            if let Some(conversation) = &p.claude {
                s.claude = Some(conversation.clone());
            }
            s.info.agent.clone_from(&p.agent);
            s.pending = lock(&shared.rules).take_up(p.claude.as_deref(), &p.agent);
            ids.insert(p.id, id);
            started += 1;
        }
        let Some(layout) = w.layout.retain(&|old| ids.contains_key(old)) else { continue };
        let layout = layout.map(&mut |old| ids[&old]);
        let focus = ids.get(&w.focus).copied().unwrap_or_else(|| layout.leaves()[0]);
        let ws = shared.next.fetch_add(1, Ordering::Relaxed);
        workspaces.insert(ws, Workspace { name: w.name.clone(), pinned: w.pinned, note: w.note.clone(), ..Workspace::new(ws, layout, focus) });
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
    workspaces.insert(ws, Workspace::new(ws, tsumugi_layout::Node::Leaf(id), id));
}

/// Tell every client the workspaces as they are now.
/// The tabs in the sidebar's order: as dragged, then the rest by id.
fn ordered<'a>(shared: &Shared, workspaces: &'a BTreeMap<WorkspaceId, Workspace>) -> Vec<&'a Workspace> {
    let order = lock(&shared.order);
    let mut list: Vec<&Workspace> = order.iter().filter_map(|id| workspaces.get(id)).collect();
    list.extend(workspaces.values().filter(|w| !order.contains(&w.id)));
    list
}

fn broadcast_workspaces(shared: &Shared, workspaces: &BTreeMap<WorkspaceId, Workspace>) {
    let list: Vec<Workspace> = ordered(shared, workspaces).into_iter().cloned().collect();
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
            workspaces.insert(wid, Workspace { layout, focus, ..w });
        }
    }
    broadcast_workspaces(shared, &workspaces);
    drop(workspaces);
    save_soon(shared, SAVE_AFTER_END);
    let list: Vec<Info> = sessions.values().map(|s| s.info.clone()).collect();
    let empty = sessions.is_empty();
    drop(sessions);
    let last = last_lines(&s.term.screen_text(), ENDED_LINES);
    for (c, tx) in lock(&shared.clients).iter() {
        if s.watchers.contains(c) {
            let _ = tx.send(ToClient::Exited { id });
        }
        let _ = tx.send(ToClient::Ended { info: Box::new(s.info.clone()), last: last.clone() });
        let _ = tx.send(ToClient::Sessions(list.clone()));
    }
    drop(s);
    if empty {
        let _ = shared.done.try_send(());
    }
}

/// How many of an ended session's last lines go with `Ended`.
const ENDED_LINES: usize = 12;

/// The last `n` lines of a screen's text with something on them, the blank
/// ones below the last dropped.
fn last_lines(text: &str, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(|l| l.trim_end().to_owned()).collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let from = lines.len().saturating_sub(n);
    lines.split_off(from)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Work out a session's state again; when it changed, the state it had and
/// since when.
///
/// In order of how sure each sign is (QUESTIONS.md Q2): an agent's own word
/// (`tsumugi notify`, OSC 9 / 99 / 777) since the last key; the shell's
/// prompt back (OSC 133) since the last key with nothing running under it;
/// nothing running and nothing said for two seconds, which is a shell sitting
/// at its prompt; and output stopped for `quiet` while a program runs and
/// has written since the last key -- the guess, marked more faintly.
fn settle(s: &mut Session, quiet: Duration) -> Option<(State, u64)> {
    let input = s.term.last_input();
    let after_input = |t: Option<std::time::Instant>| t.is_some_and(|t| input.is_none_or(|i| t > i));
    let busy = s.term.busy();
    let notice = s.notice;
    let state = match notice {
        Some((state, at)) if after_input(Some(at)) => state,
        _ if !busy && after_input(s.term.last_prompt()) => State::Done,
        _ if !busy && s.term.quiet_for(Duration::from_secs(2)) => State::Done,
        _ if busy && !quiet.is_zero() && after_input(s.term.last_output()) && (s.term.quiet_for(quiet) || screen_still_for(s, quiet)) => State::MaybeWaiting,
        _ => State::Running,
    };
    if state == s.info.state {
        return None;
    }
    let before = (s.info.state, s.info.since_ms);
    if state == State::Running {
        s.info.note.clear();
    }
    s.info.state = state;
    s.info.since_ms = now_ms();
    Some(before)
}

/// Whether the screen's text has stayed the same for `quiet`. Looked at once
/// a second at most (`settle`'s own pace), so the time is to the second.
fn screen_still_for(s: &mut Session, quiet: Duration) -> bool {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.term.screen_text().hash(&mut h);
    let now = h.finish();
    // A key starts the count again, as it does for the bytes: the same
    // screen from before it says nothing about after it (the source
    // review, 2026-10-07).
    let input = s.term.last_input();
    match s.screen {
        Some((was, since)) if was == now && input.is_none_or(|i| since >= i) => since.elapsed() >= quiet,
        _ => {
            s.screen = Some((now, std::time::Instant::now()));
            false
        }
    }
}

/// How long a session has to have run for its finishing to be worth a
/// notification (docs/v1-scope.md 1h: "1 分以上動いたときだけ").
/// How long a quiet program must be silent to read as waiting: the
/// settings' `[sessions] quiet`, else `TSUMUGI_QUIET_SECS`, else 10 s.
fn quiet_of(shared: &Shared) -> Duration {
    lock(&shared.rules).quiet.unwrap_or(shared.quiet)
}

/// Put a session's new state on the bell's list when it is one a person
/// should hear of: it wants them, it failed, or it finished a long run.
fn record_notice(shared: &Shared, s: &Session, (was, since): (State, u64)) {
    let long_run = lock(&shared.rules).long_run;
    let worth = match s.info.state {
        State::Waiting | State::Error => true,
        State::Done => was == State::Running && now_ms().saturating_sub(since) >= long_run,
        _ => false,
    };
    if !worth {
        return;
    }
    let mut notices = lock(&shared.notices);
    let id = notices.0;
    notices.0 += 1;
    let title = if s.info.title.is_empty() { s.info.command.clone() } else { s.info.title.clone() };
    notices.1.push_back(Notice { id, session: s.info.id, state: s.info.state, title, note: s.info.note.clone(), at_ms: now_ms(), read: false });
    while notices.1.len() > 200 {
        notices.1.pop_front();
    }
    broadcast_notices(shared, &notices.1);
}

/// A key typed into a session answers what it asked: its notices are read.
fn read_notices_of(shared: &Shared, id: SessionId) {
    let mut notices = lock(&shared.notices);
    let mut any = false;
    for n in notices.1.iter_mut().filter(|n| n.session == id && !n.read) {
        n.read = true;
        any = true;
    }
    if any {
        broadcast_notices(shared, &notices.1);
    }
}

fn broadcast_notices(shared: &Shared, list: &std::collections::VecDeque<Notice>) {
    let list: Vec<Notice> = list.iter().cloned().collect();
    for tx in lock(&shared.clients).values() {
        let _ = tx.send(ToClient::Notices(list.clone()));
    }
}

/// Have the git thread read `dir`'s branch (again: a checkout changes it).
fn ask_git(shared: &Shared, dir: &std::path::Path) {
    let _ = shared.git_ask.send(dir.to_path_buf());
}

fn git_info(dir: &std::path::Path) -> (String, PathBuf) {
    let (branch, root) = git(dir);
    (branch.unwrap_or_default(), root)
}

/// The git branch the folder is on -- `.git/HEAD` of the nearest repository
/// above it (a worktree's `.git` file points to its own HEAD) -- and that
/// repository's top folder (`dir` itself when it is in none). Read on the
/// server's threads, never the window's.
fn git(dir: &std::path::Path) -> (Option<String>, PathBuf) {
    match repo(dir) {
        Some((branch, root)) => (branch, root),
        None => (None, dir.to_path_buf()),
    }
}

fn repo(dir: &std::path::Path) -> Option<(Option<String>, PathBuf)> {
    let mut at = Some(dir);
    while let Some(d) = at {
        let git = d.join(".git");
        let head = if git.is_dir() {
            Some(git.join("HEAD"))
        } else if git.is_file() {
            let text = std::fs::read_to_string(&git).unwrap_or_default();
            text.trim().strip_prefix("gitdir:").map(|g| d.join(g.trim()).join("HEAD"))
        } else {
            at = d.parent();
            continue;
        };
        let head = head.and_then(|h| std::fs::read_to_string(h).ok());
        let branch = head.map(|h| {
            let h = h.trim();
            match h.strip_prefix("ref: refs/heads/") {
                Some(branch) => branch.to_owned(),
                None => h.chars().take(7).collect(),
            }
        });
        return Some((branch, d.to_path_buf()));
    }
    None
}

/// Turn "session N changed" into messages, at most one screen per session
/// every few milliseconds however fast its shell writes.
/// A session that has waited `[notify] webhook_after` while no one is at a
/// window goes to the webhook, once a wait; muted ones (or their tags) not.
fn send_webhooks(shared: &Shared, sessions: &mut BTreeMap<SessionId, Session>) {
    let rules = lock(&shared.rules);
    if rules.webhook.is_empty() {
        return;
    }
    let (url, format, after) = (rules.webhook.clone(), rules.webhook_format.clone(), rules.webhook_after);
    drop(rules);
    if !lock(&shared.attention).focused.is_empty() {
        return;
    }
    let muted = lock(&shared.muted_tags).clone();
    let now = now_ms();
    for s in sessions.values_mut() {
        let i = &s.info;
        let quiet = i.muted || i.tags.iter().any(|t| muted.contains(t));
        if i.state != State::Waiting || quiet || now.saturating_sub(i.since_ms) < after || s.webhooked == Some(i.since_ms) {
            continue;
        }
        s.webhooked = Some(i.since_ms);
        let folder = i.project.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let said = if i.note.is_empty() { "Waiting for you".to_owned() } else { i.note.clone() };
        let name = if i.title.trim().is_empty() { i.command.clone() } else { i.title.clone() };
        let message = format!("{said}\n{name} · waiting {} s", now.saturating_sub(i.since_ms) / 1000);
        crate::webhook::send(url.clone(), format.clone(), format!("{folder} is waiting for you"), message, i.cwd.display().to_string());
    }
}

/// The AI program among `procs` (a session's processes): the first whose
/// program, or a script it runs, is one of `names`. Empty for none.
pub fn agent_of(procs: &[&tsumugi_pane::Proc], names: &[String]) -> String {
    let is = |word: &str| {
        let s = tsumugi_pane::stem(word);
        names.iter().find(|n| s == **n || s.starts_with(&format!("{n}-"))).cloned()
    };
    procs.iter().find_map(|p| is(&p.name).or_else(|| p.args.iter().take(3).find_map(|a| is(a)))).unwrap_or_default()
}

/// Each shell's AI program and listening ports, from one look at the
/// process table: `shells` is each session's shell and whether Claude
/// Code's hooks know it. Run without `sessions` held -- reading the table
/// and the ports (`lsof` on macOS) takes a while (the source review,
/// 2026-10-07).
fn look_at_processes(shared: &Shared, shells: &[(SessionId, u32, bool)]) -> Vec<(SessionId, String, Vec<u16>)> {
    let table = tsumugi_pane::process_table();
    let names = lock(&shared.rules).agent_names.clone();
    shells
        .iter()
        .map(|&(id, pid, claude)| {
            let under = tsumugi_pane::descendants(&table, pid);
            let mut agent = agent_of(&under, &names);
            // Claude Code's hooks know it even where its process does not say.
            if agent.is_empty() && claude && !under.is_empty() {
                agent = "claude".into();
            }
            let mut pids: Vec<u32> = under.iter().map(|p| p.pid).collect();
            pids.push(pid);
            (id, agent, tsumugi_pane::listening_ports(&pids))
        })
        .collect()
}

/// What `look_at_processes` saw, put on the sessions still there; whether
/// any changed.
fn apply_looks(sessions: &mut BTreeMap<SessionId, Session>, seen: Vec<(SessionId, String, Vec<u16>)>) -> bool {
    let mut changed = false;
    for (id, agent, ports) in seen {
        let Some(s) = sessions.get_mut(&id) else { continue };
        // An AI program seen stays the session's until another is: between
        // its runs the shell is idle, and a restart should still take it up.
        if !agent.is_empty() && agent != s.info.agent {
            s.info.agent = agent;
            changed = true;
        }
        if ports != s.info.ports {
            s.info.ports = ports;
            changed = true;
        }
    }
    changed
}

/// Whether `id` can be typed after `--resume` as it is: Claude Code's ids
/// are UUIDs; nothing that a shell would read as more than a word.
fn conversation_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// How many messages a window may have waiting before its screens stop.
const BACKLOG: usize = 2048;

fn run_pump(shared: Arc<Shared>, dirty: Receiver<SessionId>) {
    let mut last_settle = std::time::Instant::now();
    let mut last_look = std::time::Instant::now();
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
            // The settings changed: their rules apply to every session.
            let mut rules = lock(&shared.rules);
            let fresh = shared.settings.as_deref().is_some_and(|p| crate::settings::stamp(p) != rules.stamp);
            if fresh {
                // A rule changed or taken out takes its tag off with it.
                let was: Vec<Vec<String>> = sessions.values().map(|s| rules.given(&s.info.cwd, &s.info.branch)).collect();
                *rules = Rules::read(shared.settings.as_deref());
                for (s, was) in sessions.values_mut().zip(was) {
                    changed |= rules.follow(&mut s.info, &was);
                }
            }
            drop(rules);
            for s in sessions.values_mut() {
                if let Some(before) = settle(s, quiet_of(&shared)) {
                    record_notice(&shared, s, before);
                    changed = true;
                }
                // Where the shell is now, for the sidebar and for a restore:
                // a bash that never says (no OSC 7) still has a folder.
                if let Some(cwd) = s.term.current_dir().filter(|c| *c != s.info.cwd) {
                    ask_git(&shared, &cwd);
                    let rules = lock(&shared.rules);
                    let was = rules.given(&s.info.cwd, &s.info.branch);
                    s.info.cwd = cwd;
                    rules.follow(&mut s.info, &was);
                    drop(rules);
                    changed = true;
                    save_soon(&shared, SAVE_AFTER_CHANGE);
                }
                // Its folder's branch, once the git thread has read it.
                let read = lock(&shared.git).get(&s.info.cwd).cloned();
                if let Some((branch, project)) = read.filter(|(b, p)| *b != s.info.branch || *p != s.info.project) {
                    let rules = lock(&shared.rules);
                    let was = rules.given(&s.info.cwd, &s.info.branch);
                    (s.info.branch, s.info.project) = (branch, project);
                    rules.follow(&mut s.info, &was);
                    drop(rules);
                    changed = true;
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
            send_webhooks(&shared, &mut sessions);
            // Every other second: what runs under each shell, and what it
            // listens on.
            if last_look.elapsed() >= Duration::from_secs(2) {
                last_look = std::time::Instant::now();
                let shells: Vec<(SessionId, u32, bool)> = sessions.iter().filter_map(|(id, s)| s.term.shell_pid().map(|p| (*id, p, s.info.claude))).collect();
                drop(sessions);
                let seen = look_at_processes(&shared, &shells);
                sessions = lock(&shared.sessions);
                if apply_looks(&mut sessions, seen) {
                    broadcast(&shared, &sessions);
                }
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
            let was = lock(&shared.rules).given(&s.info.cwd, &s.info.branch);
            s.info.title = s.term.title.clone();
            // Where the shell says it is (OSC 7), when it says so.
            if let Some(cwd) = &s.term.shell_cwd {
                s.info.cwd = cwd.clone();
            }
            // A notification settles the state at once; the rest waits for
            // the once-a-second pass, since asking whether anything runs under
            // the shell reads the process table.
            if before.1 != s.info.cwd {
                ask_git(&shared, &s.info.cwd);
                lock(&shared.rules).follow(&mut s.info, &was);
                save_soon(&shared, SAVE_AFTER_CHANGE);
            }
            let settled = if noticed { settle(s, quiet_of(&shared)) } else { None };
            if let Some(b) = settled {
                record_notice(&shared, s, b);
            }
            let changed = (before != (s.info.title.clone(), s.info.cwd.clone())) | settled.is_some();
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
                bracketed_paste: s.term.bracketed_paste(),
                title: s.term.title.clone(),
            };
            let change = crate::diff::diff(s.sent.as_ref().map(|(sc, ex)| (sc, ex)), &screen, &extra);
            let whole = (!s.fresh.is_empty()).then(|| crate::diff::diff(None, &screen, &extra)).flatten();
            let clients = lock(&shared.clients);
            // A window that stopped reading (hung, suspended) gets no more
            // screens once it is this far behind, and a whole one when it
            // catches up: its queue no longer grows until the server runs
            // out of memory (the source review, 2026-10-07).
            let mut behind = Vec::new();
            for c in &s.watchers {
                if let Some(tx) = clients.get(c) {
                    for text in &clipboard {
                        let _ = tx.send(ToClient::Clipboard(text.clone()));
                    }
                    if tx.len() > BACKLOG {
                        behind.push(*c);
                        continue;
                    }
                    let update = if s.fresh.contains(c) { whole.clone() } else { change.clone() };
                    if let Some(update) = update {
                        let _ = tx.send(ToClient::Screen { id, update });
                    }
                }
            }
            drop(clients);
            s.fresh.clear();
            s.fresh.extend(behind);
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

#[cfg(test)]
mod branch {
    #[test]
    fn the_branch_is_read_from_head() {
        let dir = std::env::temp_dir().join(format!("tsumugi-branch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("src/deep")).unwrap();
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/claude/task-09\n").unwrap();
        assert_eq!(super::git(&dir.join("src/deep")), (Some("claude/task-09".to_owned()), dir.clone()), "the branch, and the repository's top");
        std::fs::write(dir.join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(super::git(&dir).0.as_deref(), Some("0123456"), "a detached HEAD by its commit");
        let outside = std::env::temp_dir();
        assert_eq!(super::git(&outside), (None, outside.clone()), "no repository: the folder itself");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod review {
    /// Only an id a shell reads as one word is typed after `--resume`.
    #[test]
    fn a_conversation_id_is_one_safe_word() {
        assert!(super::conversation_id("3f2a9c1e-77b0-4c1e-9d0e-5b2f7a6c8d10"));
        for bad in ["", "x; rm -rf ~", "a b", "a\r", "$(id)", "a`b`"] {
            assert!(!super::conversation_id(bad), "{bad:?}");
        }
    }
}

#[cfg(test)]
mod tags {
    use super::*;
    use crate::settings::TagRule;

    fn rules(tags: Vec<TagRule>) -> Rules {
        Rules { tags, home: None, ..Rules::read(None) }
    }

    fn info(cwd: &str, tags: &[&str]) -> Info {
        Info { id: 1, cwd: PathBuf::from(cwd), title: String::new(), command: String::new(), state: State::Running, note: String::new(), since_ms: 0, branch: String::new(), project: PathBuf::new(), muted: false, tags: tags.iter().map(|t| t.to_string()).collect(), claude: false, conversation: String::new(), charset: String::new(), agent: String::new(), ports: Vec::new() }
    }

    /// Going back and forth between two folders with a rule each swaps the
    /// tags (the bug report, 2026-10-09: c:\dev\filer and c:\dev\tsumugi).
    #[test]
    fn a_folder_rules_tag_follows_the_shell() {
        let r = rules(vec![
            TagRule { folder: "/dev/filer".into(), branch: String::new(), tag: "filer".into() },
            TagRule { folder: "/dev/tsumugi".into(), branch: String::new(), tag: "tsumugi".into() },
        ]);
        let mut i = info("/dev/filer", &["mine"]);
        assert!(r.apply(&mut i));
        assert_eq!(i.tags, ["mine", "filer"]);
        for (to, want) in [("/dev/tsumugi", ["mine", "tsumugi"]), ("/dev/filer", ["mine", "filer"])] {
            let was = r.given(&i.cwd, &i.branch);
            i.cwd = PathBuf::from(to);
            assert!(r.follow(&mut i, &was));
            assert_eq!(i.tags, want);
        }
        let was = r.given(&i.cwd, &i.branch);
        i.cwd = PathBuf::from("/dev/filer/src");
        assert!(!r.follow(&mut i, &was), "a folder under it keeps the tag");
        assert_eq!(i.tags, ["mine", "filer"]);
    }

    #[test]
    fn pwsh_gets_the_hook_and_a_command_does_not() {
        let args = hooked_args(r"C:\Program Files\PowerShell\7\pwsh.exe", &["-NoLogo".into()]).unwrap();
        assert_eq!(&args[..3], ["-NoLogo", "-NoExit", "-EncodedCommand"]);
        assert!(hooked_args("pwsh", &["-Command".into(), "ls".into()]).is_none());
        assert!(hooked_args("powershell", &[]).is_none());
        assert!(hooked_args("bash", &[]).is_none());
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
    }
}
