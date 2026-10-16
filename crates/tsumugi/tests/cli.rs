//! The command line against a server of its own, with no window: the rows
//! of TESTING.md that are read rather than looked at. Its "Covered by tests"
//! list names these tests; a row moved there is not ticked by hand any more.
//! CI runs them on Windows (ConPTY) and Linux on every push.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tsumugi_mux::{Address, Client};

/// Long enough for a shell's first prompt on a cold CI runner.
const PATIENCE: Duration = Duration::from_secs(60);

/// A folder, a state file, settings and an address no other server uses:
/// the owner's server and the other tests' are never touched. Dropping it
/// stops the server and removes the folder.
struct Server {
    dir: PathBuf,
    address: PathBuf,
}

impl Server {
    fn new(label: &str) -> Self {
        // A plain shell: no rc files, and no PSReadLine drawing a guess
        // from history into what `read` gives back.
        Self::with_shell(label, if cfg!(windows) { "cmd" } else { "sh" }, &[])
    }

    fn with_shell(label: &str, shell: &str, args: &[&str]) -> Self {
        let dir = std::env::temp_dir().join(format!("tsumugi-cli-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let address = if cfg!(windows) {
            PathBuf::from(format!(r"\\.\pipe\tsumugi-test-{label}-{}", std::process::id()))
        } else {
            dir.join("s.sock")
        };
        let args: Vec<String> = args.iter().map(|a| format!("{a:?}")).collect();
        std::fs::write(dir.join("config.toml"), format!("[shell]\nprogram = \"{shell}\"\nargs = [{}]\n", args.join(", "))).unwrap();
        Self { dir, address }
    }

    /// `tsumugi …` against this server, not yet run.
    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_tsumugi"));
        c.args(args)
            .current_dir(&self.dir)
            .env("TSUMUGI_ADDRESS", &self.address)
            .env("TSUMUGI_STATE_HOME", &self.dir)
            .env("TSUMUGI_CONFIG_HOME", &self.dir)
            .env_remove("TSUMUGI_SESSION")
            .stdin(Stdio::null());
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    /// Its stdout, after asserting it succeeded.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(out.status.success(), "tsumugi {args:?}: {:?}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    /// `tsumugi new . …`: the new session's number.
    fn new_session(&self, args: &[&str]) -> String {
        let mut all = vec!["new", "."];
        all.extend_from_slice(args);
        let id = self.ok(&all).trim().to_owned();
        assert!(id.parse::<u64>().is_ok(), "new printed {id:?}");
        id
    }

    /// The whole scrollback of a session once `done` holds for it.
    fn read_until(&self, id: &str, what: &str, done: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let text = self.ok(&["read", id, "--all"]);
            if done(&text) {
                return text;
            }
            assert!(Instant::now() < deadline, "session {id} never showed {what}; it shows:\n{text}");
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn line_of<'a>(&self, ls: &'a str, id: &str) -> Option<Vec<&'a str>> {
        ls.lines().map(|l| l.split('\t').collect::<Vec<_>>()).find(|c| c[0] == id)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let at = Address(self.address.clone());
        let _ = Client::stop(&at);
        // Gone first: it writes its sessions down as it stops, and lets go
        // of its copy of the exe (Windows).
        let deadline = Instant::now() + Duration::from_secs(10);
        while tsumugi_mux::transport::connect(&at).is_ok() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        // It stays 300 ms after the listener goes (main.rs `server`).
        std::thread::sleep(Duration::from_millis(500));
        while (std::fs::remove_dir_all(&self.dir).is_err() || self.dir.exists()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn same_folder(a: &str, b: &Path) -> bool {
    let norm = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_owned());
    norm(Path::new(a)) == norm(b)
}

/// 1.2, 1.10, and the half of 1.4 that needs no window: `new` starts the
/// server itself, types its command into the shell, and puts every tag on;
/// `ls` lists the session in its seven columns; the next two `new` are 2 and 3.
#[test]
fn new_starts_a_server_and_ls_lists_the_session() {
    let s = Server::new("ls");
    let id = s.new_session(&["--tag", "a", "--tag", "b", "--tag", "c", "--", "echo", "hi"]);
    let text = s.read_until(&id, "`hi` run", |t| t.matches("hi").count() >= 2);
    assert!(text.contains("echo hi"), "{text}");
    let ls = s.ok(&["ls"]);
    let cols = s.line_of(&ls, &id).unwrap_or_else(|| panic!("session {id} not in:\n{ls}"));
    assert_eq!(cols.len(), 7, "{ls}");
    assert!(same_folder(cols[3], &s.dir), "folder {:?} is not {}", cols[3], s.dir.display());
    // pwsh and cmd make their exe's path the title: the column names the program.
    assert!(!cols[4].to_ascii_lowercase().ends_with(".exe"), "title {:?}", cols[4]);
    assert_eq!(cols[6], "a,b,c");
    // Sessions count on their own, not with the clients and tabs between.
    let more = [s.new_session(&[]), s.new_session(&[])];
    assert_eq!([id.as_str(), &more[0], &more[1]], ["1", "2", "3"]);
}

/// 1.15 (text): `new --worktree BRANCH` makes a worktree beside the repository, on
/// a new branch (from `--from` when given), starts the session in it and
/// puts it on the list the window asks about when its last session ends;
/// `--from` with a branch there already, or a ref that is not there, makes nothing.
#[test]
fn new_worktree_starts_the_session_in_a_worktree_of_its_own() {
    let s = Server::new("worktree");
    let repo = s.dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(&repo).args(args).stdin(Stdio::null()).output().expect("git runs");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "first"]);
    let first = git(&["rev-parse", "HEAD"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "second"]);

    let id = s.ok(&["new", "repo", "--worktree", "side", "--from", &first]).trim().to_owned();
    let tree = s.dir.join("repo-side");
    assert!(tree.is_dir(), "no worktree at {}", tree.display());
    let ls = s.ok(&["ls"]);
    let cols = s.line_of(&ls, &id).unwrap_or_else(|| {
        panic!(
            "session {id} not in:
{ls}"
        )
    });
    assert!(same_folder(cols[3], &tree), "folder {:?} is not {}", cols[3], tree.display());
    assert_eq!(git(&["rev-parse", "side"]), first, "the branch was not made from --from");
    let list = std::fs::read_to_string(s.dir.join("worktrees")).expect("the worktree list");
    assert!(
        list.lines().any(|l| same_folder(l, &tree)),
        "not on the list:
{list}"
    );

    let refused = s.run(&["new", "repo", "--worktree", "side", "--from", "main"]);
    assert!(!refused.status.success() && String::from_utf8_lossy(&refused.stderr).contains("there already"));
    let bad = s.run(&["new", "repo", "--worktree", "other", "--from", "no-such-ref"]);
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("no such commit"));
    assert!(!s.dir.join("repo-other").exists(), "a worktree for a bad ref");
}

/// 1.16 (half), the half that needs no window on the screen: `focus` asks the
/// window that had the keys last to show the session, by number or name,
/// and says so when there is no such session.
#[test]
fn focus_asks_a_window_to_show_the_session() {
    let s = Server::new("focus");
    let id = s.new_session(&["--tag", "here"]);
    let n: u64 = id.parse().unwrap();
    let window = Client::connect(&Address(s.address.clone()), || {}).expect("a window connects");
    window.judge();
    window.list().expect("the server answers the window");
    s.ok(&["focus", &id]);
    s.ok(&["focus", "here"]);
    let deadline = Instant::now() + PATIENCE;
    let mut shown = Vec::new();
    while shown.len() < 2 {
        shown.extend(window.take_shows());
        assert!(Instant::now() < deadline, "the window was asked {shown:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(shown, [n, n]);
    let none = s.run(&["focus", "999"]);
    assert!(!none.status.success() && String::from_utf8_lossy(&none.stderr).contains("no session 999"));
    assert_eq!(s.run(&["focus"]).status.code(), Some(2), "no name is a usage error");
}

/// `new … -- PROGRAM ARGS` gives each word to the program as it was, quoted
/// for the shell it is typed into: a program in a folder with a space in its
/// name made pwsh read the line as an expression (`ParserError`, the ARM64
/// lane, 2026-10-10).
#[test]
fn new_gives_the_words_to_the_program_as_they_are() {
    let s = if cfg!(windows) { Server::with_shell("argv", "pwsh", &["-NoProfile"]) } else { Server::new("argv") };
    let tools = s.dir.join("my tools");
    std::fs::create_dir_all(&tools).unwrap();
    let program = if cfg!(windows) { tools.join("run.cmd") } else { tools.join("run") };
    if cfg!(windows) {
        std::fs::write(&program, "@echo %~1> ran.txt\r\n@echo %~2>> ran.txt\r\n").unwrap();
    } else {
        std::fs::write(&program, "#!/bin/sh\nprintf '%s\\n' \"$@\" > ran.txt\n").unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    let id = s.new_session(&["--", program.to_str().unwrap(), "a  b", "it's"]);
    let ran = s.dir.join("ran.txt");
    let deadline = Instant::now() + PATIENCE;
    while !std::fs::read_to_string(&ran).is_ok_and(|t| t.lines().count() >= 2) {
        assert!(Instant::now() < deadline, "no ran.txt; the session shows:\n{}", s.ok(&["read", &id, "--all"]));
        std::thread::sleep(Duration::from_millis(200));
    }
    let lines: Vec<String> = std::fs::read_to_string(&ran).unwrap().lines().map(|l| l.trim_end().to_owned()).collect();
    assert_eq!(lines, ["a  b", "it's"]);
}

/// #76: a program in a pane stops on Ctrl+C. The server is started in a
/// process group of its own, which turned Ctrl+C off for every shell it
/// started, so `ping` ran on. The script sends the console Ctrl+C itself
/// (ignoring it) and says whether `ping` went.
#[cfg(windows)]
#[test]
fn ctrl_c_stops_a_program_in_a_session() {
    let s = Server::new("ctrlc");
    std::fs::write(
        s.dir.join("cc.ps1"),
        "Add-Type -Namespace CC -Name N -MemberDefinition '[DllImport(\"kernel32.dll\")] public static extern bool \
         GenerateConsoleCtrlEvent(uint e, uint g); [DllImport(\"kernel32.dll\")] public static extern bool \
         SetConsoleCtrlHandler(System.IntPtr h, bool add);'\r\n\
         $p = Start-Process ping -ArgumentList '-n', '30', '127.0.0.1' -NoNewWindow -PassThru\r\n\
         Start-Sleep 2\r\n\
         [void][CC.N]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)\r\n\
         [void][CC.N]::GenerateConsoleCtrlEvent(0, 0)\r\n\
         $r = if ($p.WaitForExit(5000)) { 'STOPPED' } else { 'RUNNING' }\r\n\
         Set-Content -Path cc.txt -Value $r\r\n\
         if (-not $p.HasExited) { $p.Kill() }\r\n",
    )
    .unwrap();
    let id = s.new_session(&["--", "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "cc.ps1"]);
    let out = s.dir.join("cc.txt");
    let deadline = Instant::now() + PATIENCE;
    while !std::fs::read_to_string(&out).is_ok_and(|t| !t.trim().is_empty()) {
        assert!(Instant::now() < deadline, "no cc.txt; the session shows:\n{}", s.ok(&["read", &id, "--all"]));
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), "STOPPED", "Ctrl+C did not stop ping");
}

/// 18.1: one array, a session an object with the keys a script reads.
#[test]
fn ls_json_is_one_array_of_sessions() {
    let s = Server::new("json");
    let id = s.new_session(&["--tag", "x"]);
    let json = s.ok(&["ls", "--json"]);
    let t = json.trim();
    assert!(t.starts_with('[') && t.ends_with(']'), "{json}");
    for key in [
        "id",
        "state",
        "command",
        "agent",
        "cwd",
        "project",
        "branch",
        "title",
        "note",
        "since_ms",
        "tags",
        "ports",
        "muted",
        "charset",
        "read_only",
        "profile",
    ] {
        assert!(json.contains(&format!("\"{key}\":")), "no {key} in {json}");
    }
    assert!(json.contains(&format!("\"id\": {id}")) || json.contains(&format!("\"id\":{id}")), "{json}");
    assert!(json.contains("\"x\""), "{json}");
}

/// The webhook half of 4.28: a session that fails goes to the webhook at
/// once, what it said over the program's name -- not the exe's path cmd and
/// pwsh make their title (a local listener stands in for ntfy; `curl` sends,
/// as on a real machine).
#[test]
fn a_failure_goes_to_the_webhook_named_by_its_program() {
    use std::io::Read;
    if std::process::Command::new("curl").arg("--version").output().is_err() {
        eprintln!("no curl here; skipped");
        return;
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let s = Server::new("webhook");
    let settings = s.dir.join("config.toml");
    let mut text = std::fs::read_to_string(&settings).unwrap();
    text.push_str(&format!("[notify]\nwebhook = \"http://127.0.0.1:{port}/topic\"\nwebhook_states = [\"error\"]\n"));
    std::fs::write(&settings, text).unwrap();
    let id = s.new_session(&[]);
    if cfg!(windows) {
        // cmd's title, its exe's path, comes with its first prompt.
        let deadline = Instant::now() + PATIENCE;
        while !s.ok(&["ls", "--json"]).to_ascii_lowercase().contains("cmd.exe") {
            assert!(Instant::now() < deadline, "cmd never set its title:\n{}", s.ok(&["ls", "--json"]));
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Some(Ok(mut conn)) = listener.incoming().next() else { return };
        conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        // The headers, then as many bytes as they say.
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n @ 1..) = conn.read(&mut buf) {
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got);
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|n| n.trim().to_owned()));
                if length.and_then(|n| n.parse::<usize>().ok()).is_none_or(|n| body.len() >= n) {
                    break;
                }
            }
        }
        let _ = std::io::Write::write_all(&mut conn, b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let _ = tx.send(String::from_utf8_lossy(&got).into_owned());
    });
    s.ok(&["notify", "--state", "error", "--session", &id, "boom"]);
    let got = rx.recv_timeout(PATIENCE).expect("the webhook was called");
    let (head, body) = got.split_once("\r\n\r\n").unwrap_or((&got, ""));
    assert!(head.starts_with("POST /topic") && head.contains("failed"), "{got}");
    assert_eq!(body, if cfg!(windows) { "boom\ncmd" } else { "boom\nsh" }, "{got}");
}

/// 18.2 and 18.3: `send` types a line and runs it, `read --all` gives the
/// scrollback with Japanese whole, `read --lines 5` its last five lines.
#[test]
fn send_runs_a_line_and_read_gives_it_back() {
    let s = Server::new("send");
    let id = s.new_session(&[]);
    s.ok(&["send", &id, "echo 日本語ok > sent.txt"]);
    let deadline = Instant::now() + PATIENCE;
    while !s.dir.join("sent.txt").exists() {
        assert!(Instant::now() < deadline, "send: no sent.txt; the session shows:\n{}", s.ok(&["read", &id, "--all"]));
        std::thread::sleep(Duration::from_millis(200));
    }
    let all = s.read_until(&id, "the line sent", |t| t.contains("日本語ok"));
    assert!(all.contains("echo 日本語ok > sent.txt"), "{all}");
    for n in 0..8 {
        s.ok(&["send", &id, &format!("echo line{n}")]);
    }
    s.read_until(&id, "line7 twice", |t| t.matches("line7").count() >= 2);
    let five = s.ok(&["read", &id, "--lines", "5"]);
    assert!(five.lines().count() <= 5, "{five}");
    assert!(!five.contains("日本語ok"), "--lines 5 gave more than the last five:\n{five}");
}

/// 2.78's text half: a read-only session refuses `send` with exit 1 and
/// says so, and `ls --json` tells it; let go, `send` works again.
#[test]
fn send_to_a_read_only_session_is_refused() {
    let s = Server::new("ro");
    let id = s.new_session(&[]);
    let c = Client::connect(&Address(s.address.clone()), || {}).expect("a client connects");
    let n: u64 = id.parse().unwrap();
    c.read_only(n, true);
    let deadline = Instant::now() + PATIENCE;
    let read_only = || s.ok(&["ls", "--json"]).replace(' ', "").contains("\"read_only\":true");
    while !read_only() {
        assert!(Instant::now() < deadline, "never read-only: {}", s.ok(&["ls", "--json"]));
        std::thread::sleep(Duration::from_millis(100));
    }
    let out = s.run(&["send", &id, "echo refused > no.txt"]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("read-only"), "{}", String::from_utf8_lossy(&out.stderr));
    c.read_only(n, false);
    while read_only() {
        assert!(Instant::now() < deadline, "never let go");
        std::thread::sleep(Duration::from_millis(100));
    }
    s.ok(&["send", &id, "echo let-in > in.txt"]);
    while !s.dir.join("in.txt").exists() {
        assert!(Instant::now() < deadline, "no in.txt; the session shows:\n{}", s.ok(&["read", &id, "--all"]));
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(!s.dir.join("no.txt").exists(), "the refused line ran");
}

/// kura #315 option 1, tsumugi's half: an MCP client's `ask_send` types
/// nothing until a window allows it. With no window it is refused at once; a
/// window's Refuse and the asker's own timeout type nothing; Allow types the
/// line and runs it.
#[test]
fn an_ask_to_type_waits_for_a_window_and_a_refusal_types_nothing() {
    let s = Server::new("ask");
    let id = s.new_session(&[]);
    let n: u64 = id.parse().unwrap();
    let at = Address(s.address.clone());
    let asker = Client::connect(&at, || {}).expect("an asker connects");
    let err = asker.ask_send(n, "echo none > none.txt".into(), Duration::from_secs(5)).unwrap_err();
    assert!(err.contains("no tsumugi window"), "{err}");

    let window = Client::connect(&at, || {}).expect("a window connects");
    window.judge();
    // Judge goes down the window's own connection while the asker's ask goes
    // down another: a round trip on the window's returns once the server has
    // read the Judge, so the ask below cannot overtake it ("no window").
    window.list().expect("the server answers the window");
    let answer = |allow: bool, text: &str| {
        let asker = asker.clone();
        let sent = text.to_owned();
        let t = std::thread::spawn(move || asker.ask_send(n, sent, Duration::from_secs(20)));
        let deadline = Instant::now() + PATIENCE;
        let ask = loop {
            if let Some(a) = window.asks().into_iter().next() {
                break a;
            }
            assert!(Instant::now() < deadline, "the window was never asked");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert_eq!((ask.1, ask.2.as_str()), (n, text));
        window.decide(ask.0, allow);
        t.join().unwrap()
    };
    let refused = answer(false, "echo refused > no.txt").unwrap_err();
    assert!(refused.contains("refused"), "{refused}");
    answer(true, "echo allowed > yes.txt").expect("allowed");
    let deadline = Instant::now() + PATIENCE;
    while !s.dir.join("yes.txt").exists() {
        assert!(
            Instant::now() < deadline,
            "no yes.txt; the session shows:
{}",
            s.ok(&["read", &id, "--all"])
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(!s.dir.join("no.txt").exists() && !s.dir.join("none.txt").exists(), "a line that was not allowed ran");

    // Nobody answers: the asker gives up, and a late answer does nothing.
    let t = asker.ask_send(n, "echo late > late.txt".into(), Duration::from_millis(500)).unwrap_err();
    assert!(t.contains("did not answer"), "{t}");
    std::thread::sleep(Duration::from_millis(300));
    assert!(window.asks().is_empty(), "the box stayed after the asker gave up");
    window.decide(1000, true);
    std::thread::sleep(Duration::from_millis(500));
    assert!(!s.dir.join("late.txt").exists());
}

/// 2.70's last clause, and starting a work log without the key: `log N
/// PATH` (relative to where it is run) prints the file, a second start is
/// refused while one runs, `--stop` finishes it, and a log started again at
/// the same path goes to `-2` instead of over the first.
#[test]
fn log_starts_and_finishes_a_work_log_and_never_writes_over_one() {
    let s = Server::new("log");
    let id = s.new_session(&[]);
    s.ok(&["send", &id, "echo before-log"]);
    s.read_until(&id, "the first line", |t| t.matches("before-log").count() >= 2);
    let first = s.dir.join("logs").join("work.txt");
    let printed = s.ok(&["log", &id, "logs/work.txt"]);
    assert!(same_folder(printed.trim(), &first), "log printed {printed:?}, not {}", first.display());
    let twice = s.run(&["log", &id, "logs/other.txt"]);
    assert_eq!(twice.status.code(), Some(1), "a second log while one runs: {}", String::from_utf8_lossy(&twice.stderr));
    s.ok(&["send", &id, "echo during-log"]);
    s.read_until(&id, "the second line", |t| t.matches("during-log").count() >= 2);
    assert_eq!(s.ok(&["log", &id, "--stop"]), printed);
    let second = s.ok(&["log", &id, "logs/work.txt"]);
    assert!(same_folder(second.trim(), &s.dir.join("logs").join("work-2.txt")), "the second log went to {second:?}");
    s.ok(&["log", &id, "--stop"]);
    let text = std::fs::read_to_string(&first).unwrap();
    assert!(text.contains("before-log") && text.contains("during-log"), "the first log has:\n{text}");
    assert!(std::fs::metadata(second.trim()).is_ok(), "no {second:?}");
    assert_eq!(s.run(&["log", &id, "--stop"]).status.code(), Some(1), "--stop with no log running");
}

/// 18.5 with `notify` standing in for Claude Code's hook: `wait` returns the
/// state with exit 0, runs out with exit 1, and says 3 for a session gone;
/// and 18.6's half that needs no window: `close` ends the session.
#[test]
fn wait_and_close() {
    let s = Server::new("wait");
    let id = s.new_session(&[]);
    s.ok(&["notify", "--state", "waiting", "--session", &id, "may I?"]);
    assert_eq!(s.ok(&["wait", &id, "--state", "waiting", "--timeout", "30"]).trim(), "waiting");
    let ls = s.ok(&["ls"]);
    assert_eq!(s.line_of(&ls, &id).map(|c| c[5]), Some("may I?"), "{ls}");
    s.ok(&["notify", "--state", "done", "--session", &id]);
    assert_eq!(s.ok(&["wait", &id, "--state", "done", "--timeout", "30"]).trim(), "done");
    let short = s.run(&["wait", &id, "--state", "error", "--timeout", "1"]);
    assert_eq!(short.status.code(), Some(1), "{}", String::from_utf8_lossy(&short.stderr));
    s.ok(&["close", &id]);
    let deadline = Instant::now() + PATIENCE;
    while s.line_of(&s.ok(&["ls"]), &id).is_some() {
        assert!(Instant::now() < deadline, "session {id} still listed after close");
        std::thread::sleep(Duration::from_millis(100));
    }
    let gone = s.run(&["wait", &id, "--state", "done"]);
    assert_eq!(gone.status.code(), Some(3), "{}", String::from_utf8_lossy(&gone.stderr));
}

/// What Restart the server (Settings, Advanced) waits on: a server asked
/// to stop answers no new client once a window's line to it has ended, so
/// the window does not take the old server back (17.26), and the next start
/// gets a new one.
#[test]
fn a_stopped_server_answers_nobody_once_a_line_to_it_ends() {
    let s = Server::new("restart");
    s.new_session(&[]);
    let at = Address(s.address.clone());
    let window = Client::connect(&at, || {}).unwrap();
    Client::stop(&at).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !window.lost() {
        assert!(Instant::now() < deadline, "the window's line outlived the stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match Client::connect(&at, || {}) {
            Ok(_) => panic!("the stopped server took a new client"),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => break,
            Err(_) => {}
        }
        assert!(Instant::now() < deadline, "the stopped server never left the address");
        std::thread::sleep(Duration::from_millis(50));
    }
    // It stays 300 ms after the listener goes (main.rs `server`).
    std::thread::sleep(Duration::from_millis(500));
    let id = s.new_session(&[]);
    assert!(s.line_of(&s.ok(&["ls"]), &id).is_some(), "a new server started");
}

/// `tsumugi new` at once after the last session was closed: the server was
/// still there, waiting to write its tabs before it went, and took the new
/// session away with it (#179). It stays now, once that wait is long over.
#[test]
fn a_session_started_as_the_last_one_ends_stays() {
    let s = Server::new("after-end");
    let first = s.new_session(&[]);
    s.ok(&["close", &first]);
    // `close` does not wait for the end; this does, so the next session
    // comes after it.
    let deadline = Instant::now() + PATIENCE;
    while s.line_of(&s.ok(&["ls"]), &first).is_some() {
        assert!(Instant::now() < deadline, "session {first} still listed after close");
        std::thread::sleep(Duration::from_millis(20));
    }
    let id = s.new_session(&[]);
    // Past the server's wait after an end (3 s, and 2 s more at most).
    std::thread::sleep(Duration::from_secs(7));
    let ls = s.ok(&["ls"]);
    assert!(s.line_of(&ls, &id).is_some(), "session {id} not in:\n{ls}");
}

/// `tsumugi mcp` the way Claude Code runs it: JSON-RPC lines on stdin, one
/// answer per request on stdout. With no server it says tsumugi is not
/// running; with one, `tsumugi_sessions` lists the session and
/// `tsumugi_screen` gives what it shows.
#[test]
fn mcp_lists_sessions_and_reads_a_screen() {
    use std::io::Write;
    let s = Server::new("mcp");
    let mcp = |lines: &[String]| -> Vec<String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tsumugi"))
            .arg("mcp")
            .current_dir(&s.dir)
            .env("TSUMUGI_ADDRESS", &s.address)
            .env("TSUMUGI_STATE_HOME", &s.dir)
            .env("TSUMUGI_CONFIG_HOME", &s.dir)
            .env_remove("TSUMUGI_SESSION")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        for l in lines {
            writeln!(input, "{l}").unwrap();
        }
        drop(input);
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "tsumugi mcp: {:?}", out.status);
        String::from_utf8(out.stdout).unwrap().lines().map(str::to_owned).collect()
    };
    let call = |id: u32, tool: &str, args: &str| {
        format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#)
    };
    let hello = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#.to_owned(),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_owned(),
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#.to_owned(),
    ];

    let mut lines = hello.to_vec();
    lines.push(call(3, "tsumugi_sessions", "{}"));
    let out = mcp(&lines);
    assert_eq!(out.len(), 3, "{out:#?}");
    assert!(out[0].contains(r#""protocolVersion":"2025-06-18""#) && out[0].contains(r#""name":"tsumugi""#), "{}", out[0]);
    assert!(out[1].contains("tsumugi_sessions") && out[1].contains("tsumugi_screen"), "{}", out[1]);
    assert!(out[2].contains(r#""isError":true"#) && out[2].contains("not running"), "no server yet: {}", out[2]);

    let id = s.new_session(&["--tag", "mcp", "--", "echo", "mcp-says-hi"]);
    s.read_until(&id, "`mcp-says-hi` run", |t| t.matches("mcp-says-hi").count() >= 2);
    let mut lines = hello.to_vec();
    lines.push(call(3, "tsumugi_sessions", "{}"));
    lines.push(call(4, "tsumugi_screen", &format!(r#"{{"session":"{id}","all":true}}"#)));
    lines.push(call(5, "tsumugi_screen", r#"{"session":"mcp","lines":3}"#));
    let out = mcp(&lines);
    assert_eq!(out.len(), 5, "{out:#?}");
    assert!(out[2].contains(r#""isError":false"#) && out[2].contains(&format!(r#"\"id\": {id}"#)), "{}", out[2]);
    assert!(out[3].contains(r#""isError":false"#) && out[3].contains("echo mcp-says-hi"), "{}", out[3]);
    assert!(out[4].contains(r#""isError":false"#) && out[4].contains("mcp-says-hi"), "by its tag: {}", out[4]);
}

/// `--help` names the folders the settings and the state can be moved to.
#[test]
fn help_names_the_folders() {
    let s = Server::new("help");
    let out = s.ok(&["--help"]);
    assert!(out.contains("TSUMUGI_CONFIG_HOME") && out.contains("TSUMUGI_STATE_HOME"), "{out}");
}

/// `--keys` refuses a script it cannot play before any window opens: exit 2
/// and what is wrong with it.
#[test]
fn keys_refuses_a_bad_script() {
    let s = Server::new("keys");
    for (args, says) in [
        (&["--keys"][..], "usage"),
        (&["--keys", "echo hi"], "<Space>"),
        (&["--keys", "<Shot:../up>"], "letters, digits"),
        (&["--keys", "<Wait:soon>"], "milliseconds"),
        (&["--keys", "j", "k"], "usage"),
    ] {
        let out = s.run(args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "tsumugi {args:?}: {err}");
        assert!(err.contains(says), "tsumugi {args:?} said: {err}");
    }
    assert!(s.ok(&["--help"]).contains("--keys"), "the help names --keys");
}

/// A run started detached that only waits for `TSUMUGI_KEYS_DONE` learns of
/// a refusal there too, and a key no keyboard can type is one.
#[test]
fn keys_writes_a_refusal_to_the_done_file() {
    let s = Server::new("keys-done");
    let done = s.dir.join("done.txt");
    for (args, says) in
        [(&["--keys", "<Wait:soon>"][..], "milliseconds"), (&["--keys"], "usage"), (&["--keys", "j<C-\u{3042}>"], "cannot be typed")]
    {
        let _ = std::fs::remove_file(&done);
        let out = s.command(args).env("TSUMUGI_KEYS_DONE", &done).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "tsumugi {args:?}: {err}");
        assert!(err.contains(says), "tsumugi {args:?} said: {err}");
        let report = std::fs::read_to_string(&done).unwrap_or_default();
        assert!(report.starts_with("keys: refused\nwhy: ") && report.contains(says), "tsumugi {args:?} wrote: {report:?}");
    }
}

/// `tsumugi serial` refuses words it cannot read with exit 2 and why, and
/// a port that is not there with exit 1.
#[test]
fn serial_refuses_bad_words_and_a_missing_port() {
    let s = Server::new("serial-words");
    for (args, says) in [
        (&["serial"][..], "usage"),
        (&["serial", "COM3@fast"], "not a speed"),
        (&["serial", "COM3@9600,9N1"], "not a frame"),
        (&["serial", "a", "b"], "usage"),
    ] {
        let out = s.run(args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "tsumugi {args:?}: {err}");
        assert!(err.contains(says), "tsumugi {args:?} said: {err}");
    }
    let port = if cfg!(windows) { "COM250" } else { "/dev/tsumugi-no-such-port" };
    let out = s.run(&["serial", port]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    if !cfg!(windows) {
        assert!(String::from_utf8_lossy(&out.stdout).contains(port), "it names the port");
    }
}

/// `tsumugi serial` against a pseudo-terminal standing in for the device:
/// what the device sends reaches the screen, what is typed reaches the
/// device, `Ctrl+]` twice sends one, and `Ctrl+]` then `q` leaves.
#[cfg(unix)]
#[test]
fn serial_carries_both_ways_and_leaves_on_its_key() {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    use std::sync::mpsc;

    let (mut master, mut slave): (libc::c_int, libc::c_int) = (0, 0);
    // SAFETY: two out-parameters; no name, terminal settings or size.
    assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) }, 0);
    // SAFETY: openpty gave both and nothing else owns them.
    let (mut device, side) = unsafe { (std::fs::File::from_raw_fd(master), std::fs::File::from_raw_fd(slave)) };
    // Raw, so the device's side neither echoes nor turns `\r` into `\n`.
    // SAFETY: a termios filled by tcgetattr on the master that is open.
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        assert_eq!(libc::tcgetattr(master, &mut t), 0);
        libc::cfmakeraw(&mut t);
        assert_eq!(libc::tcsetattr(master, libc::TCSANOW, &t), 0);
    }
    // SAFETY: the fd is open; the name is copied before anything else runs.
    let name = unsafe { libc::ttyname(slave) };
    assert!(!name.is_null());
    let path = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy().into_owned();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tsumugi"))
        .args(["serial", &format!("{path}@9600,7E1")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Its screen, read on a thread so a test that fails does not hang.
    let (tx, screen) = mpsc::channel();
    let mut out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0; 256];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let (to_device, from_device) = mpsc::channel();
    let mut reader = device.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0; 256];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || to_device.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let wait_for = |rx: &mpsc::Receiver<Vec<u8>>, want: &[u8]| {
        let mut got = Vec::new();
        let end = Instant::now() + Duration::from_secs(10);
        while !got.windows(want.len()).any(|w| w == want) {
            match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
                Ok(b) => got.extend(b),
                Err(_) => panic!("waited for {:?}, got {:?}", String::from_utf8_lossy(want), String::from_utf8_lossy(&got)),
            }
        }
        got
    };
    wait_for(&screen, b"9600 7E1; Ctrl+] q leaves");
    device.write_all(b"hello from the device\r\n").unwrap();
    wait_for(&screen, b"hello from the device");
    let mut keys = child.stdin.take().unwrap();
    keys.write_all(b"ls\r").unwrap();
    wait_for(&from_device, b"ls\r");
    keys.write_all(b"\x1d\x1d!").unwrap();
    assert_eq!(wait_for(&from_device, b"!"), b"\x1d!", "two Ctrl+] send one");
    keys.write_all(b"\x1dq").unwrap();
    let end = Instant::now() + PATIENCE;
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < end, "Ctrl+] q did not leave");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status:?}");
    drop(side);
}

/// `tsumugi tmux` against a tmux server of its own (skipped where there is
/// no tmux): a pane opens as a session showing what was on it, keys typed
/// there reach tmux, a split opens a second session, and when tmux ends
/// the gateway says so and the sessions close.
#[cfg(unix)]
#[test]
fn tmux_panes_open_as_sessions() {
    if Command::new("tmux").arg("-V").output().map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("no tmux here: skipped");
        return;
    }
    let s = Server::new("tmux");
    let tmux = |args: &[&str]| {
        let out = Command::new("tmux").args(args).env("TMUX_TMPDIR", &s.dir).env_remove("TMUX").current_dir(&s.dir).output().unwrap();
        assert!(out.status.success(), "tmux {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    tmux(&["new-session", "-d", "-s", "t", "-x", "80", "-y", "24", "sh"]);
    tmux(&["send-keys", "-t", "t", "echo before-$((6*7))", "Enter"]);
    /// The gateway, stopped if the test fails before tmux lets it go.
    struct Gateway(std::process::Child);
    impl Drop for Gateway {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut gateway = Gateway(
        s.command(&["tmux", "-t", "t"])
            .env("TMUX_TMPDIR", &s.dir)
            .env_remove("TMUX")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let tmux_sessions = |n: usize| {
        let deadline = Instant::now() + PATIENCE;
        loop {
            // No server yet until the gateway has started one.
            let ls = String::from_utf8_lossy(&s.run(&["ls"]).stdout).into_owned();
            let ids: Vec<String> = ls.lines().filter(|l| l.contains("tmux: ")).map(|l| l.split('\t').next().unwrap().to_owned()).collect();
            if ids.len() == n {
                return ids;
            }
            assert!(Instant::now() < deadline, "never {n} tmux sessions; ls:\n{ls}");
            std::thread::sleep(Duration::from_millis(200));
        }
    };
    let id = tmux_sessions(1).remove(0);
    s.read_until(&id, "the pane as tmux has it", |t| t.contains("before-42"));
    s.ok(&["send", &id, "echo after-$((6*7))"]);
    s.read_until(&id, "tmux's answer", |t| t.contains("after-42"));
    assert!(tmux(&["capture-pane", "-p", "-t", "t"]).contains("after-42"), "the keys went to tmux");
    tmux(&["split-window", "-h", "-t", "t", "sh"]);
    tmux_sessions(2);
    tmux(&["kill-server"]);
    let end = Instant::now() + PATIENCE;
    while gateway.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the gateway outlived tmux");
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut said = String::new();
    std::io::Read::read_to_string(gateway.0.stdout.as_mut().unwrap(), &mut said).unwrap();
    assert!(said.contains("tsumugi tmux: tmux"), "{said}");
    tmux_sessions(0);
}

/// `layout --list` names the saved layouts; `layout NAME` opens one as a tab
/// whose panes run their own command, and `split -p` / `new --name` take the
/// share and the tab's name.
#[test]
fn layout_opens_a_saved_layout_and_split_takes_a_share() {
    let s = Server::new("layout");
    std::fs::write(
        s.dir.join("layouts.toml"),
        "[[layout]]\nname = \"Dev\"\ntree = { right = 0.3, first = { run = \"echo from-a\" }, second = { run = \"echo from-b\" } }\n",
    )
    .unwrap();
    let list = s.ok(&["layout", "--list"]);
    assert!(list.starts_with("Dev\t"), "{list}");
    let out = s.run(&["layout", "nope"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Dev"), "the error lists the names");
    let ids: Vec<String> = s.ok(&["layout", "dev", "."]).lines().map(str::to_owned).collect();
    assert_eq!(ids.len(), 2, "one id per pane: {ids:?}");
    s.read_until(&ids[0], "from-a run", |t| t.matches("from-a").count() >= 2);
    s.read_until(&ids[1], "from-b run", |t| t.matches("from-b").count() >= 2);
    let id = s.ok(&["split", &ids[0], "-p", "25"]);
    assert!(id.trim().parse::<u64>().is_ok(), "split printed {id:?}");
    assert!(!s.run(&["split", &ids[0], "-p", "99"]).status.success(), "a share of 99% is refused");
    let row = s.ok(&["split", &ids[1], "--down", "-n", "4"]);
    assert_eq!(row.lines().count(), 3, "-n 4 adds three panes: {row:?}");
    assert!(!s.run(&["split", &ids[1], "-n", "4", "-p", "30"]).status.success(), "-n and -p do not go together");
    let named = s.ok(&["new", ".", "--name", "work"]);
    assert!(named.trim().parse::<u64>().is_ok(), "{named:?}");
}

#[test]
fn env_reports_the_places_and_the_server() {
    let s = Server::new("env");
    let before = s.ok(&["env"]);
    let line = |text: &str, name: &str| {
        text.lines().find(|l| l.starts_with(name)).unwrap_or_else(|| panic!("no {name} line in {text}")).to_owned()
    };
    assert!(line(&before, "tsumugi").contains(env!("CARGO_PKG_VERSION")), "{before}");
    assert!(line(&before, "os").contains(std::env::consts::OS), "{before}");
    assert!(line(&before, "settings").contains(&s.dir.display().to_string()), "{before}");
    assert!(line(&before, "server").contains("not answering"), "asking does not start a server: {before}");
    // Asking started nothing: still no server.
    assert!(s.ok(&["env"]).contains("not answering"));

    s.new_session(&[]);
    let after = s.ok(&["env"]);
    assert!(line(&after, "server").contains("running, 1 session)"), "{after}");

    let hidden = s.command(&["env"]).env("TSUMUGI_API_TOKEN", "hunter2").output().unwrap();
    let hidden = String::from_utf8(hidden.stdout).unwrap();
    assert!(hidden.contains("TSUMUGI_API_TOKEN=(hidden)") && !hidden.contains("hunter2"), "{hidden}");
}

/// What a command prints is let go when nobody reads it any more: PowerShell
/// hands `tsumugi.exe` (a windowed program, run without `tsumugi.com`) a pipe
/// for `$id = & tsumugi.exe new` and closes it without waiting, and
/// `println!` panicked there (os error 232; EPIPE on Linux and macOS).
#[test]
fn answers_into_a_closed_pipe_are_let_go() {
    let s = Server::new("closed");
    std::fs::write(
        s.dir.join("layouts.toml"),
        "[[layout]]\nname = \"Dev\"\ntree = { right = 0.3, first = { run = \"echo from-a\" }, second = { run = \"echo from-b\" } }\n",
    )
    .unwrap();
    assert!(s.ok(&["layout", "--list"]).starts_with("Dev\t"), "a layout to list");
    s.new_session(&[]);
    for args in [&["--version"][..], &["--help"], &["env"], &["ls"], &["ls", "--json"], &["layout", "--list"], &["new", "."]] {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let out = s.command(args).stdout(writer).stderr(Stdio::piped()).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success() && !err.contains("panicked"), "tsumugi {args:?} into a closed pipe: {:?}\n{err}", out.status);
    }
}
