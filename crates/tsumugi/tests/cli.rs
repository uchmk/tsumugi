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
        let address = if cfg!(windows) { PathBuf::from(format!(r"\\.\pipe\tsumugi-test-{label}-{}", std::process::id())) } else { dir.join("s.sock") };
        let args: Vec<String> = args.iter().map(|a| format!("{a:?}")).collect();
        std::fs::write(dir.join("settings.toml"), format!("[shell]\nprogram = \"{shell}\"\nargs = [{}]\n", args.join(", "))).unwrap();
        Self { dir, address }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tsumugi"))
            .args(args)
            .current_dir(&self.dir)
            .env("TSUMUGI_ADDRESS", &self.address)
            .env("TSUMUGI_STATE", self.dir.join("state"))
            .env("TSUMUGI_SETTINGS", self.dir.join("settings.toml"))
            .env_remove("TSUMUGI_SESSION")
            .stdin(Stdio::null())
            .output()
            .unwrap()
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
/// `ls` lists the session in its seven columns.
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
    assert_eq!(cols[6], "a,b,c");
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

/// 18.1: one array, a session an object with the keys a script reads.
#[test]
fn ls_json_is_one_array_of_sessions() {
    let s = Server::new("json");
    let id = s.new_session(&["--tag", "x"]);
    let json = s.ok(&["ls", "--json"]);
    let t = json.trim();
    assert!(t.starts_with('[') && t.ends_with(']'), "{json}");
    for key in ["id", "state", "command", "agent", "cwd", "project", "branch", "title", "note", "since_ms", "tags", "ports", "muted", "charset"] {
        assert!(json.contains(&format!("\"{key}\":")), "no {key} in {json}");
    }
    assert!(json.contains(&format!("\"id\": {id}")) || json.contains(&format!("\"id\":{id}")), "{json}");
    assert!(json.contains("\"x\""), "{json}");
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
    let call = |id: u32, tool: &str, args: &str| format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#);
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
