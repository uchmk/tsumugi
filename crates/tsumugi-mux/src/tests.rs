//! A server and clients in one process, over the real transport.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tsumugi_pane::{Pane, Size};

use crate::transport::Address;
use crate::{server, Client, RemotePane};

/// A server for a test: no state file, so nothing of the machine's own is
/// read or written.
fn serve(at: &Address) -> std::io::Result<server::ServerHandle> {
    server::start_with(at, server::Options { state: None })
}

/// An address no other test, and no real server, is using.
fn address() -> Address {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    if cfg!(windows) {
        Address(PathBuf::from(format!(r"\\.\pipe\tsumugi-test-{pid}-{n}")))
    } else {
        let dir = std::env::temp_dir().join(format!("tsumugi-mux-test-{pid}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        Address(dir.join("sock"))
    }
}

fn text(pane: &RemotePane) -> String {
    let rows = pane.screen().rows;
    rows.iter().map(|r| r.iter().map(|c| c.c).collect::<String>().trim_end().to_owned()).collect::<Vec<_>>().join("\n")
}

/// Wait until `pane`'s screen satisfies `ok`, or fail saying what it showed.
fn until(pane: &RemotePane, what: &str, ok: impl Fn(&str) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let t = text(pane);
        if ok(&t) {
            return;
        }
        assert!(Instant::now() < deadline, "waited for {what}; the screen was:\n{t}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The whole point of the server: a session outlives the client that
/// started it, and the next client finds it with what it had on screen.
#[test]
fn a_session_outlives_its_client() {
    let at = address();
    let srv = serve(&at).expect("the server starts");
    let cwd = std::env::temp_dir();

    let first = Client::connect(&at, || {}).expect("a client connects");
    let mut pane = first.spawn(cwd, None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    pane.resize(Size::new(80, 24), (8, 16));
    // Any prompt at all, then a word the shell echoes back: on screen twice,
    // once typed and once printed.
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    pane.send(b"echo tsumugi-mux\r".to_vec());
    until(&pane, "the echo", |t| t.matches("tsumugi-mux").count() >= 2);
    let id = pane.id();
    drop(pane);
    drop(first);

    let second = Client::connect(&at, || {}).expect("a second client connects");
    let list = second.list().expect("the sessions are listed");
    assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), vec![id], "the session is still there");
    let pane = second.attach(id);
    until(&pane, "the same screen", |t| t.matches("tsumugi-mux").count() >= 2);

    // Ending the last session stops the server.
    pane.kill();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !srv.stopped() {
        assert!(Instant::now() < deadline, "the server did not stop after its last session");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every client hears that a session started, not only the one that asked:
/// that is what a second window's sidebar is drawn from.
#[test]
fn every_client_hears_of_a_new_session() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let a = Client::connect(&at, || {}).expect("a connects");
    let b = Client::connect(&at, || {}).expect("b connects");
    let pane = a.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    while b.sessions().iter().all(|i| i.id != pane.id()) {
        assert!(Instant::now() < deadline, "b never heard of session {}", pane.id());
        std::thread::sleep(Duration::from_millis(20));
    }
    pane.kill();
}

/// Killing one of two sessions leaves the other, and the server answering.
#[test]
fn killing_one_session_leaves_the_rest() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let one = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("one starts");
    let two = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("two starts");
    until(&one, "a prompt", |t| !t.trim().is_empty());
    one.kill();
    let start = Instant::now();
    let list = c.list().expect("the server still answers");
    assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), vec![two.id()], "after {:?}", start.elapsed());
    assert!(start.elapsed() < Duration::from_secs(3), "the answer took {:?}", start.elapsed());
}

/// `tsumugi notify` marks a session waiting with its note, and a key typed
/// into it takes the mark away again.
#[test]
fn a_notification_marks_a_session_until_the_next_key() {
    use crate::State;
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    c.notify(pane.id(), State::Waiting, "Claude needs your permission".into(), None).expect("the server takes it");
    let info = |c: &Client| c.list().unwrap().into_iter().find(|i| i.id == pane.id()).unwrap();
    let i = info(&c);
    assert_eq!((i.state, i.note.as_str()), (State::Waiting, "Claude needs your permission"));
    let bell = c.notices();
    assert_eq!(bell.len(), 1, "the bell has it: {bell:?}");
    assert_eq!((bell[0].session, bell[0].read, bell[0].note.as_str()), (pane.id(), false, "Claude needs your permission"));
    pane.send(b"\r".to_vec());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !c.notices().iter().all(|n| n.read) {
        assert!(Instant::now() < deadline, "a key did not mark it read");
        std::thread::sleep(Duration::from_millis(50));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while info(&c).state == State::Waiting {
        assert!(Instant::now() < deadline, "the mark stayed after a key");
        std::thread::sleep(Duration::from_millis(100));
    }
    pane.kill();
}

/// A program's own notification (OSC 9) marks its session waiting too.
#[cfg(unix)]
#[test]
fn an_osc_9_marks_a_session_waiting() {
    use crate::State;
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let shell = Some(("sh".to_owned(), vec!["-c".to_owned(), "printf '\\033]9;ready for you\\007'; sleep 30".to_owned()]));
    let pane = c.spawn(std::env::temp_dir(), shell, Size::new(80, 24), (8, 16)).expect("sh starts");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let i = c.list().unwrap().into_iter().find(|i| i.id == pane.id()).unwrap();
        if i.state == State::Waiting {
            assert_eq!(i.note, "ready for you");
            break;
        }
        assert!(Instant::now() < deadline, "never marked: {i:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
    pane.kill();
}

/// A split puts the new session beside the old in one tab; ending it gives
/// the room back, and a shape naming a session that is gone loses it.
#[test]
fn splits_live_in_the_server() {
    use crate::{Dir, Node, Place};
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let size = Size::new(80, 24);
    let a = c.spawn(std::env::temp_dir(), None, size, (8, 16)).expect("a starts");
    let b = c.spawn_at(std::env::temp_dir(), None, size, (8, 16), Place::Split { beside: a.id(), dir: Dir::Right }).expect("b starts");
    c.list().unwrap();
    let ws = c.workspaces();
    assert_eq!(ws.len(), 1, "one tab: {ws:?}");
    assert_eq!((ws[0].layout.leaves(), ws[0].focus), (vec![a.id(), b.id()], b.id()), "b beside a, with the keys");

    // A shape that still names b after b has gone keeps a alone.
    let stale = ws[0].layout.clone();
    b.kill();
    c.list().unwrap();
    c.set_layout(ws[0].id, stale, b.id());
    c.list().unwrap();
    let ws = c.workspaces();
    assert_eq!((ws[0].layout.clone(), ws[0].focus), (Node::Leaf(a.id()), a.id()));
    a.kill();
}

/// After a restart: a new server with the old one's state file starts the
/// same tab again, split the same way, each shell in its folder, and types
/// `claude --resume` where Claude Code ran.
#[test]
fn a_restart_brings_the_tabs_back() {
    use crate::{Dir, Place, State};
    let dir = std::env::temp_dir().join(format!("tsumugi-restore-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state");
    let options = || server::Options { state: Some(state.clone()) };

    let at = address();
    let _before = server::start_with(&at, options()).expect("the first server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let size = Size::new(80, 24);
    let a = c.spawn(dir.clone(), None, size, (8, 16)).expect("a starts");
    let b = c.spawn_at(dir.clone(), None, size, (8, 16), Place::Split { beside: a.id(), dir: Dir::Down }).expect("b starts");
    c.notify(b.id(), State::Done, String::new(), Some("conv-1234".into())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while crate::state::load(&state).is_none_or(|s| s.workspaces.first().is_none_or(|w| w.panes.iter().all(|p| p.claude.is_none()))) {
        assert!(Instant::now() < deadline, "the state was never written");
        std::thread::sleep(Duration::from_millis(50));
    }

    // "Restarted": another server, the same state file.
    let at2 = address();
    let _after = server::start_with(&at2, options()).expect("the second server starts");
    let c2 = Client::connect(&at2, || {}).expect("a client connects to it");
    let saved = c2.saved().unwrap().expect("something was saved");
    assert_eq!(saved.workspaces[0].panes.len(), 2);
    assert_eq!(c2.restore().unwrap(), 2, "both panes came back");
    let ws = c2.workspaces();
    assert_eq!(ws.len(), 1);
    let leaves = ws[0].layout.leaves();
    assert!(matches!(ws[0].layout, crate::Node::Split { dir: Dir::Down, .. }), "split as before: {:?}", ws[0].layout);
    let infos = c2.list().unwrap();
    assert!(infos.iter().all(|i| i.cwd == dir), "each in its folder: {infos:?}");
    let resumed = c2.attach(leaves[1]);
    until(&resumed, "claude --resume typed", |t| t.contains("claude --resume conv-1234"));
    assert_eq!(c2.restore().unwrap(), 0, "a server with sessions restores nothing");
    for id in leaves {
        c2.attach(id).kill();
    }
    a.kill();
    b.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// The restore screen can leave panes out: only those picked come back, and
/// the tab keeps its shape around them.
#[test]
fn a_restore_can_leave_panes_out() {
    use crate::{Dir, Place};
    let dir = std::env::temp_dir().join(format!("tsumugi-restore-some-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state");
    let at = address();
    let _before = server::start_with(&at, server::Options { state: Some(state.clone()) }).unwrap();
    let c = Client::connect(&at, || {}).unwrap();
    let a = c.spawn(dir.clone(), None, Size::new(80, 24), (8, 16)).unwrap();
    let b = c.spawn_at(dir.clone(), None, Size::new(80, 24), (8, 16), Place::Split { beside: a.id(), dir: Dir::Right }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while crate::state::load(&state).is_none_or(|s| s.workspaces.iter().map(|w| w.panes.len()).sum::<usize>() < 2) {
        assert!(Instant::now() < deadline, "never saved");
        std::thread::sleep(Duration::from_millis(50));
    }
    let at2 = address();
    let _after = server::start_with(&at2, server::Options { state: Some(state.clone()) }).unwrap();
    let c2 = Client::connect(&at2, || {}).unwrap();
    assert_eq!(c2.restore_only(Some(vec![b.id()])).unwrap(), 1);
    let ws = c2.workspaces();
    assert_eq!(ws[0].layout.leaves().len(), 1, "a alone was left out");
    for id in ws[0].layout.leaves() {
        c2.attach(id).kill();
    }
    a.kill();
    b.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A second server at the same address refuses to start rather than
/// taking the first one's clients.
#[test]
fn one_server_per_address() {
    let at = address();
    let _srv = serve(&at).expect("the first server starts");
    let err = serve(&at).err().expect("a second one is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse, "{err}");
}

/// A client with nobody to talk to says so instead of waiting.
#[test]
fn no_server_no_connection() {
    assert!(Client::connect(&address(), || {}).is_err());
}
