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
    server::start_with(at, server::Options { state: None, settings: None })
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

/// Wait until `ok` holds, or fail saying `what`.
fn eventually(what: &str, ok: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// With two windows open, only one tells of a notice -- the one that had
/// the keyboard last -- and none does while someone is at either.
#[test]
fn one_window_tells() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let a = Client::connect(&at, || {}).expect("a connects");
    eventually("a alone tells", || a.attention() == (false, true));
    let b = Client::connect(&at, || {}).expect("b connects");
    // A window that never had the keyboard does not take over.
    eventually("a still tells", || a.attention() == (false, true) && b.attention() == (false, false));
    b.focus(true);
    eventually("b has the keyboard", || a.attention() == (true, false) && b.attention() == (true, true));
    b.focus(false);
    eventually("nobody looks, b tells", || a.attention() == (false, false) && b.attention() == (false, true));
    a.focus(true);
    a.focus(false);
    eventually("a had it last", || a.attention() == (false, true) && b.attention() == (false, false));
}

/// A tag is kept the one way however it is typed, a session has at most
/// five (`f` came in once `c` went), and taking one off leaves the rest.
#[test]
fn tags_go_on_and_come_off() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    for t in ["#a", "b", " c ", "d", "e", "f", "a", "#"] {
        c.tag(vec![pane.id()], t.into(), true);
    }
    c.tag(vec![pane.id()], "c".into(), false);
    c.tag(vec![pane.id()], "f".into(), true);
    let tags = || c.sessions().into_iter().find(|i| i.id == pane.id()).map(|i| i.tags).unwrap_or_default();
    eventually("the tags settle", || tags() == ["a", "b", "d", "e", "f"].map(String::from));
    pane.kill();
}

/// The settings' folder rules tag a session as it starts, and a rule
/// written while the server runs is read and applied.
#[test]
fn folder_rules_tag_sessions() {
    let dir = std::env::temp_dir().join(format!("tsumugi-rules-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let settings = dir.join("settings.toml");
    let rule = |folder: &std::path::Path, tag: &str| format!("[[tags.rule]]\nfolder = '{}'\ntag = '{tag}'\n", folder.display());
    std::fs::write(&settings, rule(&dir, "here")).unwrap();
    let at = address();
    let _srv = server::start_with(&at, server::Options { state: None, settings: Some(settings.clone()) }).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(dir.join("proj"), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    let tags = || c.sessions().into_iter().find(|i| i.id == pane.id()).map(|i| i.tags).unwrap_or_default();
    eventually("the rule's tag", || tags() == ["here"]);
    // A file's time can be as coarse as a second: written a moment later.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&settings, rule(&dir, "here") + &rule(&dir.join("*"), "{name}")).unwrap();
    eventually("the new rule's tag", || tags() == ["here", "proj"]);
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A tab dragged to another place stays there, for every window.
#[test]
fn tabs_keep_the_order_they_are_dragged_into() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let panes: Vec<_> = (0..3).map(|_| c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts")).collect();
    let tabs = || c.workspaces().iter().map(|w| w.layout.leaves()[0]).collect::<Vec<_>>();
    let ids: Vec<_> = panes.iter().map(|p| p.id()).collect();
    eventually("three tabs", || tabs() == ids);
    let last = c.workspaces()[2].id;
    c.move_workspace(last, 0);
    eventually("the last first", || tabs() == vec![ids[2], ids[0], ids[1]]);
    let first = c.workspaces()[0].id;
    c.move_workspace(first, 9);
    eventually("and back at the end", || tabs() == ids);
    for p in panes {
        p.kill();
    }
}

/// A line asked for at the start is typed once the shell is ready.
#[test]
fn a_new_session_can_start_with_a_line_typed() {
    use crate::Place;
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn_typing(std::env::temp_dir(), None, Size::new(80, 24), (8, 16), Place::NewWorkspace, Some("echo typed-$((40+2))".into())).expect("a shell starts");
    until(&pane, "the line and its answer", |t| t.contains("typed-42"));
    pane.kill();
}

/// A restarted session is a fresh shell in the same place, under the same
/// id, and resumes the conversation its hooks named.
#[test]
fn a_restart_starts_the_shell_again_in_place() {
    use crate::State;
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    pane.send(b"echo before-$((1+1))\r".to_vec());
    until(&pane, "the echo", |t| t.contains("before-2"));
    c.notify(pane.id(), State::Done, String::new(), Some("conv-9".into())).unwrap();
    c.restart(pane.id());
    until(&pane, "a fresh screen resuming the conversation", |t| !t.contains("before-2") && t.contains("claude --resume conv-9"));
    assert_eq!(c.list().unwrap().len(), 1, "still one session");
    pane.kill();
}

/// A tab's name and pin are the server's, for every window.
#[test]
fn tabs_can_be_named_and_pinned() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    eventually("a tab", || c.workspaces().len() == 1);
    let id = c.workspaces()[0].id;
    c.rename_workspace(id, "  my work  ".into());
    c.pin_workspace(id, true);
    eventually("named and pinned", || c.workspaces().first().is_some_and(|w| w.name == "my work" && w.pinned));
    pane.kill();
}

/// A tab's note is the server's; a session that ends tells every window
/// what it was and its last lines.
#[test]
fn notes_stay_and_an_ended_session_leaves_its_last_lines() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let keep = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    eventually("two tabs", || c.workspaces().len() == 2);
    let id = c.workspaces()[0].id;
    c.note_workspace(id, "  the release notes  ".into());
    eventually("noted", || c.workspaces().iter().any(|w| w.id == id && w.note == "the release notes"));
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    pane.send(b"echo last-$((40+2))\r".to_vec());
    until(&pane, "the echo", |t| t.contains("last-42"));
    let gone = pane.id();
    pane.kill();
    eventually("its end told", || {
        let ended = c.take_ended();
        ended.iter().any(|(info, last)| info.id == gone && last.iter().any(|l| l.contains("last-42")))
    });
    keep.kill();
}

/// A session read as Shift_JIS shows Japanese written in it, says so in its
/// info, and its whole buffer comes back as text.
#[cfg(unix)]
#[test]
fn a_session_can_read_shift_jis() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    c.set_charset(pane.id(), "shift_jis".into());
    eventually("said in its info", || c.sessions().iter().any(|i| i.id == pane.id() && i.charset == "Shift_JIS"));
    // 日本 in Shift_JIS, as octal escapes: nothing typed is outside ASCII.
    pane.send(b"printf '\\223\\372\\226\\173-sjis\\n'\r".to_vec());
    // A wide character's spacer reads as a space on the screen sent.
    until(&pane, "the words read", |t| t.replace(' ', "").contains("日本-sjis"));
    c.all_text(pane.id());
    eventually("the whole buffer", || c.take_texts().iter().any(|(id, t)| *id == pane.id() && t.contains("日本-sjis")));
    pane.kill();
}

/// A prompt from the input box arrives whole and is sent with Enter.
#[test]
fn a_prompt_is_pasted_and_sent() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    c.send_prompt(pane.id(), "echo sent-$((20+3))".into());
    until(&pane, "the answer", |t| t.contains("sent-23"));
    pane.kill();
}

/// The settings' `[shell.env]` reaches every session's shell.
#[test]
fn the_settings_variables_reach_the_shell() {
    let at = address();
    let dir = std::env::temp_dir().join(format!("tsumugi-env-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, "[shell.env]\nTSUMUGI_T = \"from-settings\"\n").unwrap();
    let _srv = server::start_with(&at, server::Options { state: None, settings: Some(path) }).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    let line = if cfg!(windows) { "echo \"got-$env:TSUMUGI_T\"" } else { "echo \"got-$TSUMUGI_T\"" };
    c.send_prompt(pane.id(), line.into());
    until(&pane, "the variable", |t| t.contains("got-from-settings"));
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A session waiting while no window is looked at goes to the webhook once,
/// with its folder in the title and what it said (here to a local listener;
/// `curl` does the sending, as on a real machine).
#[cfg(unix)]
#[test]
fn a_wait_nobody_sees_goes_to_the_webhook_once() {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let at = address();
    let dir = std::env::temp_dir().join(format!("tsumugi-webhook-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, format!("[notify]\nwebhook = \"http://127.0.0.1:{port}/topic\"\nwebhook_after = 0\n")).unwrap();
    let _srv = server::start_with(&at, server::Options { state: None, settings: Some(path) }).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    c.focus(false);
    c.notify(pane.id(), crate::proto::State::Waiting, "Claude needs your permission".into(), None).unwrap();
    listener.set_nonblocking(false).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for conn in listener.incoming().take(2).flatten() {
            let mut conn = conn;
            conn.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buf = vec![0u8; 4096];
            let n = conn.read(&mut buf).unwrap_or(0);
            let _ = std::io::Write::write_all(&mut conn, b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            let _ = tx.send(String::from_utf8_lossy(&buf[..n]).into_owned());
        }
    });
    let got = rx.recv_timeout(Duration::from_secs(10)).expect("the webhook was called");
    assert!(got.starts_with("POST /topic") && got.contains("is waiting for you") && got.contains("Claude needs your permission"), "{got}");
    assert!(rx.recv_timeout(Duration::from_secs(3)).is_err(), "once a wait");
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A search across sessions finds a line in the one that printed it, and
/// nothing for what no session printed.
#[test]
fn every_sessions_scrollback_is_searched() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let quiet = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    // The line typed has `$((40+2))`; only the answer has `found-42`.
    c.send_prompt(pane.id(), "echo found-$((40+2))".into());
    until(&pane, "the answer", |t| t.contains("found-42"));
    c.search_all("FOUND-42".into());
    eventually("the lines found", || c.found_all().is_some_and(|(q, _)| q == "FOUND-42"));
    let (_, hits) = c.found_all().expect("an answer");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!((hits[0].id, hits[0].text.trim()), (pane.id(), "found-42"));
    c.search_all("nothing-printed-this".into());
    eventually("nothing found", || c.found_all().is_some_and(|(q, h)| q == "nothing-printed-this" && h.is_empty()));
    pane.kill();
    quiet.kill();
}

/// A server asked to stop goes, its tabs written down for the next one.
#[cfg(unix)]
#[test]
fn a_server_stops_when_asked_and_keeps_its_tabs() {
    let dir = std::env::temp_dir().join(format!("tsumugi-stop-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state");
    let at = address();
    let srv = server::start_with(&at, server::Options { state: Some(state.clone()), settings: None }).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(dir.clone(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    Client::stop(&at).expect("asked");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !srv.stopped() {
        assert!(Instant::now() < deadline, "the server did not stop");
        std::thread::sleep(Duration::from_millis(20));
    }
    srv.wait(Duration::from_secs(1));
    let saved = crate::state::load(&state).expect("the tabs were written");
    assert_eq!(saved.workspaces.len(), 1);
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A window of another version is told so, apart from other failures.
#[test]
fn another_version_is_told_apart() {
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let (mut r, mut w) = crate::transport::connect(&at).unwrap().split().unwrap();
    crate::frame::write(&mut w, &crate::proto::ToServer::Hello { version: crate::proto::VERSION + 1 }).unwrap();
    match crate::frame::read::<_, crate::proto::ToClient>(&mut r).unwrap() {
        crate::proto::ToClient::Error(e) => assert!(e.contains("version"), "{e}"),
        other => panic!("answered {other:?}"),
    }
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

/// A pane taken out of its split becomes a tab of its own, right after the
/// one it left; a pane alone in its tab stays where it is.
#[test]
fn a_pane_becomes_a_tab_of_its_own() {
    use crate::{Dir, Node, Place};
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let size = Size::new(80, 24);
    let a = c.spawn(std::env::temp_dir(), None, size, (8, 16)).expect("a starts");
    let other = c.spawn(std::env::temp_dir(), None, size, (8, 16)).expect("another tab");
    let b = c.spawn_at(std::env::temp_dir(), None, size, (8, 16), Place::Split { beside: a.id(), dir: Dir::Right }).expect("b starts");
    c.own_tab(b.id());
    c.list().unwrap();
    let ws = c.workspaces();
    let leaves: Vec<Vec<crate::SessionId>> = ws.iter().map(|w| w.layout.leaves()).collect();
    assert_eq!(leaves, vec![vec![a.id()], vec![b.id()], vec![other.id()]], "after the tab it left");
    assert_eq!(ws[0].layout, Node::Leaf(a.id()));
    c.own_tab(a.id());
    c.list().unwrap();
    assert_eq!(c.workspaces().len(), 3, "alone already: nothing changes");
    for p in [a, b, other] {
        p.kill();
    }
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
    let options = || server::Options { state: Some(state.clone()), settings: None };

    let at = address();
    let _before = server::start_with(&at, options()).expect("the first server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let size = Size::new(80, 24);
    let a = c.spawn(dir.clone(), None, size, (8, 16)).expect("a starts");
    let b = c.spawn_at(dir.clone(), None, size, (8, 16), Place::Split { beside: a.id(), dir: Dir::Down }).expect("b starts");
    c.mute(vec![a.id()], true);
    c.tag(vec![a.id(), b.id()], "#review".into(), true);
    c.tag(vec![b.id()], "ci run".into(), true);
    c.mute_tag("ci-run".into(), true);
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
    let muted: Vec<_> = infos.iter().filter(|i| i.muted).map(|i| i.id).collect();
    assert_eq!(muted, vec![leaves[0]], "the muted pane stays muted");
    let tags: Vec<_> = leaves.iter().map(|id| infos.iter().find(|i| i.id == *id).unwrap().tags.clone()).collect();
    assert_eq!(tags, vec![vec!["review".to_owned()], vec!["review".to_owned(), "ci-run".to_owned()]], "tags come back");
    eventually("the muted tag comes back", || c2.muted_tags() == vec!["ci-run".to_owned()]);
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
    let _before = server::start_with(&at, server::Options { state: Some(state.clone()), settings: None }).unwrap();
    let c = Client::connect(&at, || {}).unwrap();
    let a = c.spawn(dir.clone(), None, Size::new(80, 24), (8, 16)).unwrap();
    let b = c.spawn_at(dir.clone(), None, Size::new(80, 24), (8, 16), Place::Split { beside: a.id(), dir: Dir::Right }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while crate::state::load(&state).is_none_or(|s| s.workspaces.iter().map(|w| w.panes.len()).sum::<usize>() < 2) {
        assert!(Instant::now() < deadline, "never saved");
        std::thread::sleep(Duration::from_millis(50));
    }
    let at2 = address();
    let _after = server::start_with(&at2, server::Options { state: Some(state.clone()), settings: None }).unwrap();
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

/// The AI program in a session is told by a process under its shell: its
/// program's name, a build of it (`codex-x86_64-…`), or a script node runs.
#[test]
fn an_ai_program_is_told_by_its_process() {
    use tsumugi_pane::Proc;
    let names: Vec<String> = crate::settings::AGENTS.iter().map(|s| s.to_string()).collect();
    let p = |name: &str, args: &[&str]| Proc { pid: 1, ppid: 0, name: name.into(), args: args.iter().map(|a| a.to_string()).collect() };
    let codex = p("codex-x86_64-unknown-linux-musl", &[]);
    let gemini = p("node", &["node", "/usr/local/bin/gemini"]);
    let vim = p("vim", &["vim", "codex.md"]);
    let claude_exe = p("claude", &[]);
    assert_eq!(server::agent_of(&[&codex], &names), "codex");
    assert_eq!(server::agent_of(&[&gemini], &names), "gemini");
    assert_eq!(server::agent_of(&[&vim], &names), "", "a file named after one is not it");
    assert_eq!(server::agent_of(&[&vim, &claude_exe], &names), "claude");
    let s = crate::settings::parse("[agents.mine]\nresume = \"mine --again\"\n").unwrap();
    assert_eq!(server::agent_of(&[&p("mine", &[])], &s.agent_names()), "mine", "one of the settings' own");
    assert_eq!((s.resume_for("mine").as_deref(), s.resume_for("codex").as_deref(), s.resume_for("gemini")), (Some("mine --again"), Some("codex resume --last"), None));
}

/// What runs in a session and what it listens on reach every window: a
/// script called `codex` under the shell is the AI program, and the port its
/// child listens on is the session's.
#[cfg(target_os = "linux")]
#[test]
fn a_sessions_program_and_ports_are_seen() {
    let dir = std::env::temp_dir().join(format!("tsumugi-agent-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let script = dir.join("codex");
    std::fs::write(&script, format!("#!/bin/sh\npython3 -c \"import socket,time; s=socket.socket(); s.bind(('127.0.0.1',{port})); s.listen(); time.sleep(30)\"\n")).unwrap();
    std::process::Command::new("chmod").arg("+x").arg(&script).status().unwrap();
    let at = address();
    let _srv = serve(&at).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(dir.clone(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    c.send_prompt(pane.id(), format!("{} &", script.display()));
    eventually("codex and its port", || c.sessions().iter().any(|i| i.id == pane.id() && i.agent == "codex" && i.ports.contains(&port)));
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A program that keeps writing the same screen -- Claude Code at its
/// prompt redrawing it -- is still quiet: probably waiting, not running.
#[cfg(unix)]
#[test]
fn a_program_redrawing_the_same_screen_is_quiet() {
    use crate::State;
    let at = address();
    let dir = std::env::temp_dir().join(format!("tsumugi-redraw-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.toml");
    std::fs::write(&path, "[sessions]\nquiet = 2\n").unwrap();
    let _srv = server::start_with(&at, server::Options { state: None, settings: Some(path) }).expect("the server starts");
    let c = Client::connect(&at, || {}).expect("a client connects");
    let pane = c.spawn(std::env::temp_dir(), None, Size::new(80, 24), (8, 16)).expect("a shell starts");
    until(&pane, "a prompt", |t| !t.trim().is_empty());
    pane.send(b"clear; while :; do printf '\\rsame'; sleep 0.3; done\r".to_vec());
    until(&pane, "the loop", |t| t.contains("same"));
    let info = |c: &Client| c.list().unwrap().into_iter().find(|i| i.id == pane.id()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while info(&c).state != State::MaybeWaiting {
        assert!(Instant::now() < deadline, "still {:?} with the screen unchanged", info(&c).state);
        std::thread::sleep(Duration::from_millis(200));
    }
    pane.kill();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A machine without tsumugi, or with another version, is told what to do.
#[test]
fn a_failed_ssh_says_what_to_do() {
    use crate::client::{explain, ssh_args};
    use std::io::{Error, ErrorKind};
    let args = ssh_args("box", "/opt/t/tsumugi");
    assert_eq!(args[args.len() - 3..], ["box", "/opt/t/tsumugi", "proxy"]);
    let eof = Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer");
    let missing = explain("box", "tsumugi", &eof, Some(127), "bash: line 1: tsumugi: command not found");
    assert!(missing.contains("tsumugi is not there") && missing.contains(crate::client::RELEASES) && missing.contains("command not found"), "{missing}");
    let windows = explain("win", "tsumugi", &eof, Some(1), "'tsumugi' is not recognized as an internal or external command");
    assert!(windows.contains("is not there"), "{windows}");
    let refused = explain("box", "tsumugi", &eof, Some(255), "ssh: connect to host box port 22: Connection refused");
    assert_eq!(refused, "box: ssh: connect to host box port 22: Connection refused", "ssh's own failure is as it said");
    let other = Error::new(ErrorKind::InvalidData, "the server speaks version 26, the client 27");
    let version = explain("box", "tsumugi", &other, None, "");
    assert!(version.contains(env!("CARGO_PKG_VERSION")) && version.contains("restart its server"), "{version}");
    assert_eq!(explain("box", "tsumugi", &eof, None, ""), "box: failed to fill whole buffer");
}
