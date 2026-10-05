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
