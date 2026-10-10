//! `tsumugi new` and `tsumugi attach`: start a session from a script or an
//! agent, and open the window on one (v1-scope, the multiplexer's CLI).

use std::path::PathBuf;

use tsumugi_mux::{Client, Info, Place, SessionId};
use tsumugi_pane::Size;

use crate::sort;

/// What `tsumugi new` was asked.
#[derive(Debug, Default, PartialEq)]
pub struct New {
    pub folder: Option<PathBuf>,
    pub tags: Vec<String>,
    /// The command typed into the new shell (`claude`), from after `--`.
    pub command: Vec<String>,
}

pub const NEW_USAGE: &str = "tsumugi new [FOLDER] [--tag TAG]... [-- COMMAND ARGS...]";

pub fn parse_new(args: &[String]) -> Result<New, String> {
    let mut out = New::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => {
                out.command = it.by_ref().cloned().collect();
            }
            "--tag" | "-t" => match it.next() {
                Some(t) => out.tags.push(t.clone()),
                None => return Err("--tag takes a name".into()),
            },
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.folder.is_none() => out.folder = Some(PathBuf::from(a)),
            _ => return Err(format!("one folder only (`{a}` is a second); a command goes after --")),
        }
    }
    Ok(out)
}

/// The command as one line for the shell: words with a space or a quote in
/// them in double quotes, which bash, zsh, PowerShell and cmd all read. One
/// word alone is a line already (`-- "claude --continue"`), as typed.
pub fn command_line(words: &[String]) -> Option<String> {
    if let [line] = words {
        return Some(line.clone());
    }
    let quoted: Vec<String> = words
        .iter()
        .map(|w| if w.is_empty() || w.contains([' ', '\t', '"', '\'']) { format!("\"{}\"", w.replace('"', "\\\"")) } else { w.clone() })
        .collect();
    (!quoted.is_empty()).then(|| quoted.join(" "))
}

/// Start the session in a tab of its own; its id.
pub fn new(client: &Client, n: New) -> Result<SessionId, String> {
    let here = std::env::current_dir().map_err(|e| format!("no current folder: {e}"))?;
    let folder = match n.folder {
        Some(f) => here.join(crate::newsession::expand(&f.to_string_lossy())),
        None => here,
    };
    // Made whole without resolving links (canonicalize would give Windows'
    // `\\?\` form, which shells do not take as a folder).
    let folder = std::path::absolute(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    if !folder.is_dir() {
        return Err(format!("{}: not a folder", folder.display()));
    }
    let shell = None;
    let pane = client
        .spawn_typing(folder, shell, Size::new(80, 24), (8, 16), Place::NewWorkspace, command_line(&n.command))
        .map_err(|e| format!("the session did not start: {e}"))?;
    let id = pane.id();
    for t in n.tags.iter().filter_map(|t| tsumugi_mux::proto::tag_name(t)) {
        client.tag(vec![id], t, true);
    }
    // The tags go on a thread of the client's, and this process ends next:
    // a question asked after them is answered once the server has them all
    // (it took only the first before, or none).
    client.list().map_err(|e| format!("the tags were not sent: {e}"))?;
    Ok(id)
}

/// The session `name` means: its number, or the one whose folder, title,
/// program or tag it is (case aside). An error when none or several are.
pub fn find(list: &[Info], name: &str) -> Result<SessionId, String> {
    if let Ok(id) = name.parse::<SessionId>() {
        return list.iter().find(|i| i.id == id).map(|i| i.id).ok_or_else(|| format!("no session {id}"));
    }
    let want = name.trim_start_matches('#').to_lowercase();
    let file = |p: &std::path::Path| p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
    let hits: Vec<&Info> = list
        .iter()
        .filter(|i| {
            file(&i.project) == want
                || file(&i.cwd) == want
                || sort::display_title(&i.title, &i.command).to_lowercase() == want
                || crate::program_name(&i.command).to_lowercase() == want
                || i.tags.iter().any(|t| t.to_lowercase() == want)
        })
        .collect();
    match hits.as_slice() {
        [one] => Ok(one.id),
        [] => Err(format!("no session called `{name}` (tsumugi ls lists them)")),
        many => {
            let ids: Vec<String> = many.iter().map(|i| i.id.to_string()).collect();
            Err(format!("`{name}` is more than one session ({}); give its number", ids.join(", ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsumugi_mux::State;

    fn info(id: SessionId, cwd: &str, command: &str, tags: &[&str]) -> Info {
        Info {
            id,
            cwd: cwd.into(),
            title: String::new(),
            command: command.into(),
            state: State::Running,
            note: String::new(),
            since_ms: 0,
            branch: String::new(),
            project: cwd.into(),
            muted: false,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            claude: false,
            conversation: String::new(),
            charset: String::new(),
            agent: String::new(),
            ports: Vec::new(),
            recording: String::new(),
            logging: String::new(),
        }
    }

    #[test]
    fn new_reads_a_folder_tags_and_a_command() {
        let a = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let n = parse_new(&a("~/dev/filer --tag review -- claude --continue")).unwrap();
        assert_eq!(n, New { folder: Some("~/dev/filer".into()), tags: vec!["review".into()], command: a("claude --continue") });
        assert_eq!(parse_new(&[]).unwrap(), New::default());
        assert!(parse_new(&a("a b")).is_err(), "two folders");
        assert!(parse_new(&a("--tag")).is_err());
        assert_eq!(command_line(&a("claude --continue")).as_deref(), Some("claude --continue"));
        assert_eq!(command_line(&["echo".into(), "a b".into()]).as_deref(), Some("echo \"a b\""));
        assert_eq!(command_line(&["claude --continue".into()]).as_deref(), Some("claude --continue"), "one word is the line");
        assert_eq!(command_line(&[]), None);
    }

    #[test]
    fn a_session_is_found_by_number_folder_program_or_tag() {
        let list = [info(1, "/home/u/dev/filer", "claude", &["review"]), info(2, "/home/u/dev/tsumugi", "/bin/bash", &[]), info(3, "/tmp", "/bin/bash", &[])];
        assert_eq!(find(&list, "2"), Ok(2));
        assert!(find(&list, "9").is_err());
        assert_eq!(find(&list, "Filer"), Ok(1), "case aside");
        assert_eq!(find(&list, "#review"), Ok(1));
        assert_eq!(find(&list, "claude"), Ok(1));
        assert!(find(&list, "bash").unwrap_err().contains("2, 3"), "two of them");
        assert!(find(&list, "nothing").is_err());
    }
}

/// The remote control's usage: what another program (or an AI) runs to
/// work the sessions without the window.
pub const REMOTE_USAGE: &str = "\
tsumugi ls [--json]                      the sessions (--json: one array, every field)
tsumugi send N|NAME TEXT...              type TEXT into the session, then Enter
tsumugi read N|NAME [--lines K] [--all]  its last K lines (40), or its whole scrollback
tsumugi split N|NAME [--down] [-- CMD]   a new pane beside it, in its folder; prints its number
tsumugi close N|NAME                     end the session
tsumugi wait N|NAME [--state S] [--timeout SECS]
                                         until it is waiting, done or failed (or S: waiting,
                                         done, error, running); exit 1 on timeout, 3 if it ended
  each takes --host H: the sessions on another machine, over ssh (`tsumugi proxy` there)";

/// `--host H` out of a command's words, wherever it stands before `--` (and,
/// for `send`, before the session: what follows it is the text to type).
pub fn take_host(args: &[String], text_after_who: bool) -> Result<(Option<String>, Vec<String>), String> {
    let (mut host, mut rest) = (None, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let free = rest.iter().any(|r: &String| r == "--") || (text_after_who && rest.iter().any(|r: &String| !r.starts_with('-')));
        match a.as_str() {
            "--host" if !free => host = Some(it.next().filter(|h| !h.is_empty()).ok_or("--host takes a machine (as ssh knows it)")?.clone()),
            _ => rest.push(a.clone()),
        }
    }
    Ok((host, rest))
}

/// `tsumugi read`'s choices.
#[derive(Debug, PartialEq)]
pub struct Read {
    pub who: String,
    pub lines: usize,
    pub all: bool,
}

pub fn parse_read(args: &[String]) -> Result<Read, String> {
    let mut out = Read { who: String::new(), lines: 40, all: false };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--all" => out.all = true,
            "--lines" | "-n" => out.lines = it.next().and_then(|n| n.parse().ok()).ok_or("--lines takes a number")?,
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.who.is_empty() => out.who = a.clone(),
            _ => return Err(format!("one session only (`{a}` is a second)")),
        }
    }
    if out.who.is_empty() {
        return Err("which session? (tsumugi ls lists them)".into());
    }
    Ok(out)
}

/// The last `n` lines of `text` with something on them below.
pub fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let from = lines.len().saturating_sub(n);
    let mut out = lines[from..].join("\n");
    out.push('\n');
    out
}

/// `tsumugi split`'s choices.
#[derive(Debug, PartialEq)]
pub struct Split {
    pub who: String,
    pub down: bool,
    pub command: Vec<String>,
}

pub fn parse_split(args: &[String]) -> Result<Split, String> {
    let mut out = Split { who: String::new(), down: false, command: Vec::new() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => out.command = it.by_ref().cloned().collect(),
            "--down" => out.down = true,
            "--right" => out.down = false,
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.who.is_empty() => out.who = a.clone(),
            _ => return Err(format!("one session only (`{a}` is a second); a command goes after --")),
        }
    }
    if out.who.is_empty() {
        return Err("which session? (tsumugi ls lists them)".into());
    }
    Ok(out)
}

/// `tsumugi wait`'s choices: the states that end the wait.
#[derive(Debug, PartialEq)]
pub struct Wait {
    pub who: String,
    pub states: Vec<tsumugi_mux::State>,
    pub timeout: Option<std::time::Duration>,
}

pub fn parse_wait(args: &[String]) -> Result<Wait, String> {
    use tsumugi_mux::State;
    let mut out = Wait { who: String::new(), states: vec![State::Waiting, State::MaybeWaiting, State::Done, State::Error], timeout: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--state" => {
                let s = it.next().and_then(|s| State::from_word(s)).ok_or("--state takes waiting, done, error or running")?;
                out.states = if s == State::Waiting { vec![State::Waiting, State::MaybeWaiting] } else { vec![s] };
            }
            "--timeout" => out.timeout = Some(std::time::Duration::from_secs_f64(it.next().and_then(|n| n.parse::<f64>().ok()).filter(|n| *n >= 0.0).ok_or("--timeout takes seconds")?)),
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.who.is_empty() => out.who = a.clone(),
            _ => return Err(format!("one session only (`{a}` is a second)")),
        }
    }
    if out.who.is_empty() {
        return Err("which session? (tsumugi ls lists them)".into());
    }
    Ok(out)
}

/// The sessions as one JSON array, every field `tsumugi ls` knows.
pub fn ls_json(list: &[Info]) -> String {
    use crate::json::Json;
    let s = |v: &str| Json::String(v.to_owned());
    Json::Array(
        list.iter()
            .map(|i| {
                Json::Object(vec![
                    ("id".into(), Json::Number(i.id as f64)),
                    ("state".into(), s(i.state.word())),
                    ("command".into(), s(&i.command)),
                    ("agent".into(), s(&i.agent)),
                    ("cwd".into(), s(&i.cwd.to_string_lossy())),
                    ("project".into(), s(&i.project.to_string_lossy())),
                    ("branch".into(), s(&i.branch)),
                    ("title".into(), s(&i.title)),
                    ("note".into(), s(&i.note)),
                    ("since_ms".into(), Json::Number(i.since_ms as f64)),
                    ("tags".into(), Json::Array(i.tags.iter().map(|t| s(t)).collect())),
                    ("ports".into(), Json::Array(i.ports.iter().map(|p| Json::Number(f64::from(*p))).collect())),
                    ("muted".into(), Json::Bool(i.muted)),
                    ("charset".into(), s(&i.charset)),
                ])
            })
            .collect(),
    )
    .pretty()
}

/// A new pane beside session `id`, in its folder, running `command`; its
/// number.
pub fn split(client: &Client, list: &[Info], id: SessionId, s: &Split) -> Result<SessionId, String> {
    let info = list.iter().find(|i| i.id == id).ok_or_else(|| format!("no session {id}"))?;
    let dir = if s.down { tsumugi_mux::Dir::Down } else { tsumugi_mux::Dir::Right };
    let pane = client
        .spawn_typing(info.cwd.clone(), None, Size::new(80, 24), (8, 16), Place::Split { beside: id, dir }, command_line(&s.command))
        .map_err(|e| format!("the pane did not start: {e}"))?;
    Ok(pane.id())
}

#[cfg(test)]
mod remote_tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_host_is_taken_before_the_free_words() {
        assert_eq!(take_host(&words("--host box 3 --lines 5"), false).unwrap(), (Some("box".into()), words("3 --lines 5")));
        assert_eq!(take_host(&words("3 --host box --down -- ssh --host x"), false).unwrap(), (Some("box".into()), words("3 --down -- ssh --host x")));
        assert_eq!(take_host(&words("--json"), false).unwrap(), (None, words("--json")));
        // `send`'s text is typed as it is.
        assert_eq!(take_host(&words("--host box 3 echo --host y"), true).unwrap(), (Some("box".into()), words("3 echo --host y")));
        assert!(take_host(&words("3 --host"), false).is_err());
    }

    #[test]
    fn the_remote_controls_words_are_read() {
        assert_eq!(parse_read(&words("3 --lines 10")).unwrap(), Read { who: "3".into(), lines: 10, all: false });
        assert!(parse_read(&words("--all")).is_err(), "which session");
        assert_eq!(parse_split(&words("filer --down -- npm run dev")).unwrap(), Split { who: "filer".into(), down: true, command: words("npm run dev") });
        let w = parse_wait(&words("2 --state waiting --timeout 1.5")).unwrap();
        assert_eq!((w.states.len(), w.timeout), (2, Some(std::time::Duration::from_millis(1500))), "waiting takes probably waiting too");
        assert!(parse_wait(&words("2 --state soon")).is_err());
        assert_eq!(last_lines("a\nb\nc\n\n\n", 2), "b\nc\n");
    }
}
