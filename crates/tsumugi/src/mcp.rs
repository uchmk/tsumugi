//! `tsumugi mcp`: tsumugi's sessions as MCP tools. Claude Code (or another
//! LLM client) starts it and talks JSON-RPC on its stdin and stdout; each
//! call asks the running server, the way `tsumugi ls` and `tsumugi read` do.
//! It never starts a server: with none running, the tool says so.
//!
//! Reading is free (filer's QUESTIONS.md Q95): what the sessions are and what
//! one shows. `tsumugi_send` types a line into a session, but only after the
//! user has pressed Allow in a box in a tsumugi window (kura #315, option 1);
//! the box's default is Refuse, and there is no "do not ask again".

use std::time::Duration;

/// How long `tsumugi_send` waits for the user to answer the box.
const DECIDE_WAIT: Duration = Duration::from_secs(120);

/// The most text one `tsumugi_send` may carry.
const MAX_TEXT: usize = 8000;

use ito_mcp::serde_json::{json, Value};
use ito_mcp::{Answer, Server, Tool};
use tsumugi_mux::Client;

use crate::cli;

pub fn run() -> std::process::ExitCode {
    let stdin = std::io::stdin();
    match server().serve(stdin.lock(), std::io::stdout()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            crate::err(&format!("tsumugi mcp: {e}\n"));
            std::process::ExitCode::FAILURE
        }
    }
}

fn server() -> Server {
    // What this agent opened, so `tsumugi_close` closes only that.
    let opened = ito_mcp::Opened::new();
    let (for_split, for_close) = (opened.clone(), opened);
    Server::new("tsumugi", env!("CARGO_PKG_VERSION"))
        .instructions("tsumugi is the terminal where the user runs several AI CLI sessions (Claude Code and others) side by side. These tools read the sessions of the tsumugi running on this machine: which there are, what each is doing, and what one shows. tsumugi_send types a line into a session only after the user presses Allow in a box in their window; they can say no, so tell them what you are sending and why. tsumugi_split opens a shell pane and tsumugi_close closes only panes opened that way.")
        .tool(Tool::new(
            "tsumugi_sessions",
            "The sessions in tsumugi: for each, its number (id), state (running, waiting, done, error, idle), command, folder (cwd), project, git branch, title, note and tags. A session waiting is one asking the user something.",
            |_| sessions(),
        ))
        .tool(Tool::new(
            "tsumugi_attention",
            "A summary for supervising the sessions: how many are running, waiting, maybe waiting, done or in error, and the ones that are not running, longest in their state first, each with for_secs (how long it has been so), title, note, folder, branch and tags. Start here to decide which session to look at (tsumugi_screen) or answer.",
            |_| attention(),
        ))
        .tool(
            Tool::new(
                "tsumugi_wait",
                "Wait until a tsumugi session stops running (it is waiting for an answer, done or in error), for up to `timeout_secs` (50 at most; ask again when it says timed_out). Answers with the session's state, so a supervising agent can sleep on one session instead of polling tsumugi_sessions.",
                wait,
            )
            .input(json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "The session's number, or its folder, title, program or tag." },
                    "timeout_secs": { "type": "integer", "minimum": 1, "description": "How long to wait, at most 50 (30 when left out)." },
                },
                "required": ["session"],
            })),
        )
        .tool(
            Tool::new(
                "tsumugi_screen",
                "The text a tsumugi session shows: its last lines, or its whole scrollback. Name the session by its number, or by a name tsumugi_sessions shows (its folder, title, program or a tag).",
                screen,
            )
            .input(json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "The session's number, or its folder, title, program or tag." },
                    "lines": { "type": "integer", "minimum": 1, "description": "How many of the last lines (40 when left out)." },
                    "all": { "type": "boolean", "description": "The whole scrollback instead of the last lines." },
                },
                "required": ["session"],
            })),
        )
        .tool(
            Tool::new(
                "tsumugi_send",
                "Ask to type a prompt or a command into a tsumugi session and press Enter. The user is asked in their tsumugi window, with the session and the exact text, and nothing is typed unless they press Allow. A session they have made read-only takes nothing.",
                send,
            )
            .input(json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "The session's number, or its folder, title, program or tag." },
                    "text": { "type": "string", "description": "What to type; Enter is pressed after it." },
                },
                "required": ["session", "text"],
            }))
            .writes()
            .needs_confirm(),
        )
        .tool(
            Tool::new(
                "tsumugi_split",
                "Open a new shell pane beside or below a tsumugi session, in its folder, and answer its number. It starts no program: type into it with tsumugi_send (which asks the user). tsumugi_close can close only panes opened this way.",
                move |args| split(args, &for_split),
            )
            .input(json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "The session to open the pane beside: its number, or its folder, title, program or tag." },
                    "down": { "type": "boolean", "description": "Below it instead of to its right." },
                },
                "required": ["session"],
            }))
            .writes(),
        )
        .tool(
            Tool::new(
                "tsumugi_close",
                "Close a pane this agent opened with tsumugi_split. Any other session is refused: the user's own sessions are never closed from here.",
                move |args| close(args, &for_close),
            )
            .input(json!({
                "type": "object",
                "properties": { "session": { "type": "string", "description": "The number tsumugi_split answered." } },
                "required": ["session"],
            }))
            .destructive(),
        )
}

fn client() -> Result<Client, String> {
    Client::connect(&tsumugi_mux::address(), || {}).map_err(|e| format!("tsumugi is not running ({e})"))
}

fn sessions() -> Answer {
    let list = client()?.list().map_err(|e| e.to_string())?;
    Ok(cli::ls_json(&list))
}

fn attention() -> Answer {
    let list = client()?.list().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    Ok(cli::attention_json(&list, now))
}

fn wait(args: &Value) -> Answer {
    let who = who(args)?;
    let client = client()?;
    let id = cli::find(&client.list().map_err(|e| e.to_string())?, &who)?;
    let timeout = ito_mcp::arg_secs(args, "timeout_secs", 30);
    let phase = |list: &[tsumugi_mux::Info]| -> Option<Option<(ito_mcp::Phase, &'static str)>> {
        let i = list.iter().find(|i| i.id == id)?;
        Some(match i.state {
            tsumugi_mux::State::Running => None,
            tsumugi_mux::State::Waiting | tsumugi_mux::State::MaybeWaiting => Some((ito_mcp::Phase::Waiting, i.state.word())),
            tsumugi_mux::State::Done => Some((ito_mcp::Phase::Done, i.state.word())),
            tsumugi_mux::State::Error => Some((ito_mcp::Phase::Error, i.state.word())),
        })
    };
    let mut ended = false;
    let found = ito_mcp::wait(timeout, Duration::from_secs(1), || match client.list().ok().as_deref().and_then(phase) {
        Some(Some(p)) => Some(p),
        Some(None) => None,
        None => {
            ended = true;
            Some((ito_mcp::Phase::Error, "gone"))
        }
    });
    Ok(match found {
        _ if ended => format!("session {id} has ended"),
        Some((_, word)) => format!("session {id} is {word}"),
        None => format!("timed_out: session {id} is still running"),
    })
}

/// A session named by number or by name; a number arrives as a string or as
/// a number, depending on the client.
fn who(args: &Value) -> Result<String, String> {
    match args.get("session") {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        _ => Err("`session` is missing: a number or a name from tsumugi_sessions".into()),
    }
}

fn send(args: &Value) -> Answer {
    let who = who(args)?;
    let text = args.get("text").and_then(Value::as_str).ok_or("`text` is missing")?;
    if text.trim().is_empty() {
        return Err("`text` is empty".into());
    }
    if text.chars().count() > MAX_TEXT {
        return Err(format!("`text` is longer than {MAX_TEXT} characters"));
    }
    let client = client()?;
    let list = client.list().map_err(|e| e.to_string())?;
    let id = cli::find(&list, &who)?;
    client.ask_send(id, text.to_owned(), DECIDE_WAIT)?;
    Ok(format!("typed into session {id} and sent"))
}

fn split(args: &Value, opened: &ito_mcp::Opened) -> Answer {
    let who = who(args)?;
    let down = args.get("down").and_then(Value::as_bool).unwrap_or(false);
    let client = client()?;
    let list = client.list().map_err(|e| e.to_string())?;
    let id = cli::find(&list, &who)?;
    let split = cli::Split { who, down, percent: None, count: None, command: Vec::new() };
    let made = cli::split(&client, &list, id, &split)?;
    let new = made.last().ok_or("no pane was opened")?;
    opened.insert(&new.to_string());
    Ok(format!("opened session {new}"))
}

fn close(args: &Value, opened: &ito_mcp::Opened) -> Answer {
    let id = opened.check(&who(args)?)?;
    let client = client()?;
    let list = client.list().map_err(|e| e.to_string())?;
    let sid = cli::find(&list, &id)?;
    client.attach(sid).kill();
    client.list().map_err(|e| e.to_string())?;
    opened.remove(&id);
    Ok(format!("closed session {id}"))
}

fn screen(args: &Value) -> Answer {
    let who = who(args)?;
    let lines = args.get("lines").and_then(Value::as_u64).map_or(40, |n| n.max(1) as usize);
    let all = args.get("all").and_then(Value::as_bool).unwrap_or(false);
    let client = client()?;
    let list = client.list().map_err(|e| e.to_string())?;
    let id = cli::find(&list, &who)?;
    client.all_text(id);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((_, text)) = client.take_texts().into_iter().find(|(i, _)| *i == id) {
            return Ok(if all { text } else { cli::last_lines(&text, lines) });
        }
        if std::time::Instant::now() > deadline {
            return Err("the tsumugi server did not answer".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_are_listed_and_only_send_split_and_close_write() {
        let s = server();
        let a: Value = serde_json_from(&s.handle(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap());
        let names: Vec<&str> = a["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["tsumugi_sessions", "tsumugi_attention", "tsumugi_wait", "tsumugi_screen", "tsumugi_send", "tsumugi_split", "tsumugi_close"]
        );
        for t in a["result"]["tools"].as_array().unwrap() {
            assert_eq!(
                t["annotations"]["readOnlyHint"] == true,
                !["tsumugi_send", "tsumugi_split", "tsumugi_close"].contains(&t["name"].as_str().unwrap()),
                "{t}"
            );
        }
        for t in a["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["annotations"]["destructiveHint"] == true, t["name"] == "tsumugi_close", "{t}");
        }
    }

    #[test]
    fn close_refuses_what_the_agent_did_not_open() {
        let opened = ito_mcp::Opened::new();
        opened.insert("7");
        assert!(close(&json!({"session": "3"}), &opened).unwrap_err().contains("not opened by the agent"));
        assert!(close(&json!({"session": 3}), &opened).unwrap_err().contains("not opened by the agent"));
        assert!(close(&json!({}), &opened).unwrap_err().contains("session"));
    }

    #[test]
    fn screen_wants_a_session() {
        let err = screen(&json!({})).unwrap_err();
        assert!(err.contains("session"), "{err}");
    }

    #[test]
    fn wait_wants_a_session() {
        assert!(wait(&json!({"timeout_secs": 1})).unwrap_err().contains("session"));
    }

    #[test]
    fn send_wants_a_session_and_some_text() {
        assert!(send(&json!({"text": "hi"})).unwrap_err().contains("session"));
        assert!(send(&json!({"session": "1"})).unwrap_err().contains("text"));
        assert!(send(&json!({"session": "1", "text": "  "})).unwrap_err().contains("empty"));
    }

    fn serde_json_from(s: &str) -> Value {
        ito_mcp::serde_json::from_str(s).unwrap()
    }
}
