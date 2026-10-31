//! `tsumugi skill setup|status|remove` and `tsumugi hook session-start`:
//! teach the AI CLIs on this PC (Claude Code, Codex) about tsumugi.
//!
//! Where the files go, their formats and the manifest are ito's `ito-agent`;
//! what stays here is the words of the skill, the command that starts the MCP
//! server and the hook's command.

use ito_agent::{Agent, App, Home, Launch, Options};

pub const USAGE: &str = "tsumugi skill setup [--dry-run] [--no-hook] [--force] [--agent claude|codex]...   teach Claude Code (and Codex) about tsumugi: a skill, the MCP server and a SessionStart hook\ntsumugi skill status [--agent claude|codex]...                                       what is installed\ntsumugi skill remove [--dry-run]                                                     take back only what setup wrote";

const SKILL: &str = "---
name: tsumugi
description: Use when running inside tsumugi (the terminal for several AI CLI sessions, TSUMUGI_SESSION is set) to look at, drive or wait for the other sessions, or to mark this session for the user
---

# tsumugi

tsumugi runs several AI CLI sessions side by side. Each is a numbered session; this one is `$TSUMUGI_SESSION`.

Prefer the MCP tools (`tsumugi_sessions`, `tsumugi_read`, `tsumugi_wait`, `tsumugi_attention`, `tsumugi_send`); the same things are on the command line:

- `tsumugi ls [--json]` lists the sessions (number, state, command, folder, title, tags).
- `tsumugi read N` shows what session N displays. Reading needs no permission.
- `tsumugi wait N` blocks until session N is waiting or done.
- `tsumugi send N TEXT` types a line into session N. The user must press Allow in a box in their window, and may refuse: say what you send and why, and do not retry a refusal.
- `tsumugi notify --state waiting|done|error [MESSAGE]` marks this session in the sidebar; `tsumugi tag TAG...` tags it.

Sessions belong to the user. Do not send to or close a session you did not start unless asked to.
";

/// What `setup` asks of ito-agent: the exe that is running, as the MCP server and the hook.
fn app() -> std::io::Result<App> {
    let exe = std::env::current_exe()?;
    let manifest = ito_common::env_dir("TSUMUGI_STATE_HOME")
        .or_else(|| ito_common::state_dir("tsumugi"))
        .ok_or_else(|| std::io::Error::other("no state folder"))?
        .join("agent-install.json");
    Ok(App {
        name: "tsumugi".into(),
        skill: SKILL.into(),
        mcp: Some(Launch::new(exe.to_string_lossy(), &["mcp"])),
        hook: Some(format!("\"{}\" hook session-start", exe.display())),
        manifest,
    })
}

fn agent_named(s: &str) -> Option<Agent> {
    Agent::ALL.into_iter().find(|a| a.name() == s)
}

/// `tsumugi skill ...`
pub fn skill(args: &[String]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let usage = || {
        crate::err(&format!("usage:\n{USAGE}\n"));
        ExitCode::from(2)
    };
    let Some(what) = args.first().map(String::as_str) else { return usage() };
    let mut opts = Options::default();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => opts.dry_run = true,
            "--no-hook" => opts.hook = false,
            "--force" => opts.force = true,
            "--agent" => match it.next().and_then(|s| agent_named(s)) {
                Some(g) => opts.agents.push(g),
                None => {
                    crate::err("tsumugi skill: --agent takes claude or codex\n");
                    return ExitCode::from(2);
                }
            },
            _ => return usage(),
        }
    }
    let run = || -> std::io::Result<String> {
        let (app, home) = (app()?, Home::detect()?);
        match what {
            "setup" => ito_agent::setup(&app, &home, &opts).map(|r| lines(&r.lines, r.changed)),
            "remove" => ito_agent::remove(&app, &home, opts.dry_run).map(|r| lines(&r.lines, r.changed)),
            "status" => ito_agent::status(&app, &home, &opts).map(|list| {
                list.iter()
                    .map(|s| {
                        format!(
                            "{}	{}	{}
",
                            s.agent.name(),
                            s.item.name(),
                            s.state
                        )
                    })
                    .collect()
            }),
            _ => Err(std::io::Error::other("usage")),
        }
    };
    if !matches!(what, "setup" | "status" | "remove") {
        return usage();
    }
    match run() {
        Ok(text) => {
            crate::out(&text);
            ExitCode::SUCCESS
        }
        Err(e) => {
            crate::err(&format!("tsumugi skill {what}: {e}\n"));
            ExitCode::FAILURE
        }
    }
}

fn lines(lines: &[String], changed: bool) -> String {
    let mut s: String = lines.iter().map(|l| format!("{l}\n")).collect();
    if !changed {
        s.push_str("nothing to change\n");
    }
    s
}

/// `tsumugi hook session-start`: what an agent's SessionStart hook prints into the new session's
/// context. Outside a tsumugi session it prints nothing and succeeds, because an agent run in another
/// terminal shows a hook's failure on every start.
pub fn hook(args: &[String]) -> std::process::ExitCode {
    if args.first().map(String::as_str) != Some("session-start") {
        crate::err("usage: tsumugi hook session-start\n");
        return std::process::ExitCode::from(2);
    }
    if let Some(text) = session_start(std::env::var("TSUMUGI_SESSION").ok().as_deref(), || {
        tsumugi_mux::Client::connect(&tsumugi_mux::address(), || {}).and_then(|c| c.list()).ok()
    }) {
        crate::out(&text);
    }
    std::process::ExitCode::SUCCESS
}

/// The hook's text for the session number in the environment, with the sessions as the server lists them.
pub fn session_start(session: Option<&str>, list: impl FnOnce() -> Option<Vec<tsumugi_mux::Info>>) -> Option<String> {
    let id: u64 = session?.trim().parse().ok()?;
    let mut text = format!("You are running inside tsumugi, session {id}. Other sessions: `tsumugi ls`; the tsumugi MCP tools and the tsumugi skill say what you may do with them.");
    if let Some(me) = list().and_then(|l| l.into_iter().find(|i| i.id == id)) {
        text = format!("You are running inside tsumugi, session {id}, in {}", me.cwd.display());
        if !me.tags.is_empty() {
            text.push_str(&format!(", tags: {}", me.tags.join(",")));
        }
        text.push_str(". Other sessions: `tsumugi ls`; the tsumugi MCP tools and the tsumugi skill say what you may do with them.");
    }
    text.push('\n');
    Some(text)
}
