//! A server and clients in one process, over the real transport.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tsumugi_pane::{Pane, Size};

use crate::transport::Address;
use crate::{server, Client, RemotePane};

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
    let srv = server::start(&at).expect("the server starts");
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
    let _srv = server::start(&at).expect("the server starts");
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
    let _srv = server::start(&at).expect("the server starts");
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
    let _srv = server::start(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    c.notify(pane.id(), State::Waiting, "Claude needs your permission".into()).expect("the server takes it");
    let info = |c: &Client| c.list().unwrap().into_iter().find(|i| i.id == pane.id()).unwrap();
    let i = info(&c);
    assert_eq!((i.state, i.note.as_str()), (State::Waiting, "Claude needs your permission"));
    pane.send(b"\r".to_vec());
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
    let _srv = server::start(&at).expect("the server starts");
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

/// A second server at the same address refuses to start rather than
/// taking the first one's clients.
#[test]
fn one_server_per_address() {
    let at = address();
    let _srv = server::start(&at).expect("the first server starts");
    let err = server::start(&at).err().expect("a second one is refused");
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse, "{err}");
}

/// A client with nobody to talk to says so instead of waiting.
#[test]
fn no_server_no_connection() {
    assert!(Client::connect(&address(), || {}).is_err());
}
