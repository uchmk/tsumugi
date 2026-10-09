//! `tsumugi mcp`: tsumugi's sessions as MCP tools. Claude Code (or another
//! LLM client) starts it and talks JSON-RPC on its stdin and stdout; each
//! call asks the running server, the way `tsumugi ls` and `tsumugi read` do.
//! It never starts a server: with none running, the tool says so.
//!
//! Only reading for now (filer's QUESTIONS.md Q95): what the sessions are and
//! what one shows. Typing into a session comes later, behind a confirmation.

use std::time::Duration;

use tsumugi_mcp::serde_json::{json, Value};
use tsumugi_mcp::{Answer, Server, Tool};
use tsumugi_mux::Client;

use crate::cli;

pub fn run() -> std::process::ExitCode {
    let stdin = std::io::stdin();
    match server().serve(stdin.lock(), std::io::stdout()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tsumugi mcp: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn server() -> Server {
    Server::new("tsumugi", env!("CARGO_PKG_VERSION"))
        .instructions("tsumugi is the terminal where the user runs several AI CLI sessions (Claude Code and others) side by side. These tools read the sessions of the tsumugi running on this machine: which there are, what each is doing, and what one shows.")
        .tool(Tool::new(
            "tsumugi_sessions",
            "The sessions in tsumugi: for each, its number (id), state (running, waiting, done, error, idle), command, folder (cwd), project, git branch, title, note and tags. A session waiting is one asking the user something.",
            |_| sessions(),
        ))
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
}

fn client() -> Result<Client, String> {
    Client::connect(&tsumugi_mux::address(), || {}).map_err(|e| format!("tsumugi is not running ({e})"))
}

fn sessions() -> Answer {
    let list = client()?.list().map_err(|e| e.to_string())?;
    Ok(cli::ls_json(&list))
}

fn screen(args: &Value) -> Answer {
    // A number arrives as a string or as a number, depending on the client.
    let who = match args.get("session") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => return Err("`session` is missing: a number or a name from tsumugi_sessions".into()),
    };
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
    fn the_tools_are_listed_and_only_read() {
        let s = server();
        let a: Value = serde_json_from(&s.handle(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap());
        let names: Vec<&str> = a["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["tsumugi_sessions", "tsumugi_screen"]);
        assert!(a["result"]["tools"].as_array().unwrap().iter().all(|t| t["annotations"]["readOnlyHint"] == true));
    }

    #[test]
    fn screen_wants_a_session() {
        let err = screen(&json!({})).unwrap_err();
        assert!(err.contains("session"), "{err}");
    }

    fn serde_json_from(s: &str) -> Value {
        tsumugi_mcp::serde_json::from_str(s).unwrap()
    }
}
