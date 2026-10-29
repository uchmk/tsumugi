//! `tsumugi new` and `tsumugi attach`: start a session from a script or an
//! agent, and open the window on one (v1-scope, the multiplexer's CLI).

use std::path::PathBuf;

use ito_pane::Size;
use tsumugi_mux::{Client, Info, Place, SessionId};

use crate::sort;

/// What `tsumugi new` was asked.
#[derive(Debug, Default, PartialEq)]
pub struct New {
    pub folder: Option<PathBuf>,
    pub tags: Vec<String>,
    /// The tab's name, given at once.
    pub name: Option<String>,
    /// Its place among the tabs (1 is the first), when it should not go last.
    pub at: Option<usize>,
    /// The command typed into the new shell (`claude`), from after `--`:
    /// its words, quoted there for the shell (one word is a line).
    pub command: Vec<String>,
    /// Start it in a new git worktree on this branch (made beside the
    /// repository, as the window's "in a worktree" does).
    pub worktree: Option<String>,
    /// What the worktree's new branch is made from (else the current commit).
    pub from: Option<String>,
}

pub const NEW_USAGE: &str =
    "tsumugi new [FOLDER] [--tag TAG]... [--name NAME] [--at N] [--worktree BRANCH [--from REF]] [-- COMMAND ARGS...]";

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
            "--name" | "-n" => match it.next() {
                Some(n) => out.name = Some(n.clone()),
                None => return Err("--name takes a name".into()),
            },
            "--at" => match it.next().and_then(|n| n.parse::<usize>().ok()).filter(|n| *n >= 1) {
                Some(n) => out.at = Some(n),
                None => return Err("--at takes a place, 1 for the first tab".into()),
            },
            "--worktree" | "-w" => match it.next().filter(|b| !b.trim().is_empty() && !b.starts_with('-')) {
                Some(b) => out.worktree = Some(b.trim().to_owned()),
                None => return Err("--worktree takes a branch".into()),
            },
            "--from" => match it.next().filter(|r| !r.trim().is_empty() && !r.starts_with('-')) {
                Some(r) => out.from = Some(r.trim().to_owned()),
                None => return Err("--from takes a commit, branch or tag".into()),
            },
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.folder.is_none() => out.folder = Some(PathBuf::from(a)),
            _ => return Err(format!("one folder only (`{a}` is a second); a command goes after --")),
        }
    }
    if out.from.is_some() && out.worktree.is_none() {
        return Err("--from goes with --worktree".into());
    }
    Ok(out)
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
    // The worktree first; the session starts in it, and the window's list
    // has it, so the window asks whether to remove it when its last session ends.
    let folder = match &n.worktree {
        Some(branch) => crate::worktree::add_from(&folder, branch, n.from.as_deref())?,
        None => folder,
    };
    let shell = None;
    let pane = match client.spawn_typing(folder.clone(), shell, Size::new(80, 24), (8, 16), Place::NewWorkspace, n.command) {
        Ok(pane) => pane,
        Err(e) => {
            // No session will ever end in the worktree just made, so no
            // window would ask to remove it: take it back here.
            if n.worktree.is_some() {
                if let Err(w) = crate::worktree::remove(&folder) {
                    return Err(format!("the session did not start: {e} (the worktree {} stays: {w})", folder.display()));
                }
            }
            return Err(format!("the session did not start: {e}"));
        }
    };
    let id = pane.id();
    for t in n.tags.iter().filter_map(|t| tsumugi_mux::proto::tag_name(t)) {
        client.tag(vec![id], t, true);
    }
    // The tags go on a thread of the client's, and this process ends next:
    // a question asked after them is answered once the server has them all
    // (it took only the first before, or none).
    client.list().map_err(|e| format!("the tags were not sent: {e}"))?;
    if n.name.is_some() || n.at.is_some() {
        let w = tab_of(client, id, 1)?;
        if let Some(name) = n.name {
            client.rename_workspace(w.id, name);
        }
        if let Some(at) = n.at {
            client.move_workspace(w.id, at - 1);
        }
        client.list().map_err(|e| format!("the name was not sent: {e}"))?;
    }
    Ok(id)
}

/// The tab that has session `id` and at least `panes` panes, once the
/// server has said so.
fn tab_of(client: &Client, id: SessionId, panes: usize) -> Result<tsumugi_mux::Workspace, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        client.list().map_err(|e| e.to_string())?;
        if let Some(w) = client.workspaces().into_iter().find(|w| w.layout.contains(&id) && w.layout.leaves().len() >= panes) {
            return Ok(w);
        }
        if std::time::Instant::now() > deadline {
            return Err("the server did not say where the tab is".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// `tsumugi layout NAME [FOLDER]` or `tsumugi layout --list`.
#[derive(Debug, PartialEq)]
pub enum LayoutCmd {
    List,
    Open { name: String, folder: Option<PathBuf> },
}

pub const LAYOUT_USAGE: &str = "tsumugi layout NAME [FOLDER]           open a saved layout (layouts.toml) as a new tab, in FOLDER (here); prints its panes' numbers\ntsumugi layout --list                   the saved layouts and their panes";

pub fn parse_layout(args: &[String]) -> Result<LayoutCmd, String> {
    let mut words = Vec::new();
    let mut list = false;
    for a in args {
        match a.as_str() {
            "--list" | "-l" => list = true,
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ => words.push(a.clone()),
        }
    }
    match (list, words.as_slice()) {
        (true, []) => Ok(LayoutCmd::List),
        (true, _) => Err("--list takes nothing else".into()),
        (false, []) => Err("which layout? (tsumugi layout --list lists them)".into()),
        (false, [name]) => Ok(LayoutCmd::Open { name: name.clone(), folder: None }),
        (false, [name, folder]) => Ok(LayoutCmd::Open { name: name.clone(), folder: Some(PathBuf::from(folder)) }),
        _ => Err("a layout and a folder only".into()),
    }
}

/// The layout called `name` (case aside), or the error that says which are there.
pub fn layout_named<'a>(list: &'a [crate::layouts::Layout], name: &str) -> Result<&'a crate::layouts::Layout, String> {
    let want = name.trim().to_lowercase();
    list.iter().find(|l| l.name == name).or_else(|| list.iter().find(|l| l.name.to_lowercase() == want)).ok_or_else(|| {
        let names: Vec<&str> = list.iter().map(|l| l.name.as_str()).collect();
        if names.is_empty() {
            format!("no layout called `{name}` (none are saved; layouts.toml is read from beside config.toml)")
        } else {
            format!("no layout called `{name}` (there are: {})", names.join(", "))
        }
    })
}

/// Start a layout's panes in a tab of its own, named for the layout, with
/// its shape; the panes' numbers in the layout's order.
pub fn open_layout(client: &Client, l: &crate::layouts::Layout, base: &std::path::Path, claude: &str) -> Result<Vec<SessionId>, String> {
    let panes = l.tree.leaves();
    let mut made: Vec<SessionId> = Vec::new();
    for p in &panes {
        let place = match made.first() {
            None => Place::NewWorkspace,
            Some(first) => Place::Split { beside: *first, dir: tsumugi_mux::Dir::Right },
        };
        let folder = p.folder(base);
        if !folder.is_dir() {
            return Err(format!("{}: not a folder (a pane's dir)", folder.display()));
        }
        let pane = client
            .spawn_typing(folder, None, Size::new(80, 24), (8, 16), place, p.typed(claude))
            .map_err(|e| format!("a pane did not start: {e}"))?;
        made.push(pane.id());
    }
    let first = *made.first().ok_or("the layout has no panes")?;
    let tab = tab_of(client, first, made.len())?;
    if let Some(tree) = crate::layouts::shaped(&l.tree, &made) {
        client.set_layout(tab.id, tree, first);
    }
    client.rename_workspace(tab.id, l.name.clone());
    client.list().map_err(|e| e.to_string())?;
    Ok(made)
}

/// Where the first pane of `tree` is given `share` against its neighbour,
/// as `split -p` asks: the split whose halves are exactly `a` and `b`.
pub fn set_share(tree: &mut tsumugi_mux::Node<SessionId>, a: SessionId, b: SessionId, share: f32) -> bool {
    use tsumugi_mux::Node;
    match tree {
        Node::Leaf(_) => false,
        Node::Split { ratio, first, second, .. } => {
            if matches!((&**first, &**second), (Node::Leaf(x), Node::Leaf(y)) if *x == a && *y == b) {
                *ratio = share.clamp(ito_layout::MIN_RATIO, 1.0 - ito_layout::MIN_RATIO);
                true
            } else {
                set_share(first, a, b, share) || set_share(second, a, b, share)
            }
        }
    }
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
            read_only: false,
            profile: String::new(),
        }
    }

    #[test]
    fn ls_titles_a_session_by_its_name_not_its_programs_path() {
        let pwsh = r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.6.0_x64__8wekyb3d8bbwe\pwsh.exe";
        let mut i = info(3, "C:/dev", pwsh, &[]);
        i.title = pwsh.into();
        assert_eq!(ls_title(&i), "pwsh");
        i.title = "✳ Fix the zoom badge".into();
        assert_eq!(ls_title(&i), "Fix the zoom badge");
        // No title stays empty: the command column names the program.
        i.title = String::new();
        assert_eq!(ls_title(&i), "");
        i.title = "✳".into();
        assert_eq!(ls_title(&i), "");
        // `--json` keeps the title the program set.
        i.title = pwsh.into();
        assert!(ls_json(&[i]).contains("pwsh.exe"));
    }

    #[test]
    fn new_reads_a_folder_tags_and_a_command() {
        let a = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let n = parse_new(&a("~/dev/filer --tag review -- claude --continue")).unwrap();
        assert_eq!(
            n,
            New { folder: Some("~/dev/filer".into()), tags: vec!["review".into()], command: a("claude --continue"), ..New::default() }
        );
        let n = parse_new(&a("--name api --at 2")).unwrap();
        assert_eq!((n.name.as_deref(), n.at), (Some("api"), Some(2)));
        assert!(parse_new(&a("--at 0")).is_err());
        assert!(parse_new(&a("--name")).is_err());
        assert_eq!(parse_new(&[]).unwrap(), New::default());
        assert!(parse_new(&a("a b")).is_err(), "two folders");
        assert!(parse_new(&a("--tag")).is_err());
        let n = parse_new(&a("~/dev/filer --worktree fix/login --from origin/main -- claude")).unwrap();
        assert_eq!((n.worktree.as_deref(), n.from.as_deref()), (Some("fix/login"), Some("origin/main")));
        assert_eq!(parse_new(&a("-w side")).unwrap().worktree.as_deref(), Some("side"));
        assert!(parse_new(&a("--worktree")).is_err());
        assert!(parse_new(&a("--worktree --from main")).is_err(), "an option is not a branch");
        assert!(parse_new(&a("--from main")).is_err(), "--from alone");
        assert!(parse_new(&a("--worktree side --from")).is_err());
    }

    #[test]
    fn a_session_is_found_by_number_folder_program_or_tag() {
        let list = [
            info(1, "/home/u/dev/filer", "claude", &["review"]),
            info(2, "/home/u/dev/tsumugi", "/bin/bash", &[]),
            info(3, "/tmp", "/bin/bash", &[]),
        ];
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
tsumugi split N|NAME [--down] [-p PERCENT | -n COUNT] [-- CMD]\n                                         a new pane beside it, in its folder (PERCENT: its share,\n                                         like tmux -p); prints its number
tsumugi close N|NAME                     end the session
tsumugi log N|NAME [PATH] [--stop]       write its text to PATH (Downloads) as it goes, or finish;
                                         prints the file (`-2` when PATH is there already)
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
            "--host" if !free => {
                host = Some(it.next().filter(|h| !h.is_empty()).ok_or("--host takes a machine (as ssh knows it)")?.clone())
            }
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
    /// The new pane's share of the two, in percent (tmux's `-p`).
    pub percent: Option<u8>,
    /// How many panes the row (or column) ends up with, this one included,
    /// all the same size: `-n 4` adds three.
    pub count: Option<u8>,
    pub command: Vec<String>,
}

pub fn parse_split(args: &[String]) -> Result<Split, String> {
    let mut out = Split { who: String::new(), down: false, percent: None, count: None, command: Vec::new() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--" => out.command = it.by_ref().cloned().collect(),
            "--down" => out.down = true,
            "--right" => out.down = false,
            "--percent" | "-p" => match it.next().and_then(|n| n.trim_end_matches('%').parse::<u8>().ok()).filter(|n| (1..=95).contains(n))
            {
                Some(n) => out.percent = Some(n),
                None => return Err("--percent takes the new pane's share, 1 to 95".into()),
            },
            "--count" | "-n" => match it.next().and_then(|n| n.parse::<u8>().ok()).filter(|n| (2..=12).contains(n)) {
                Some(n) => out.count = Some(n),
                None => return Err("--count takes how many panes in all, 2 to 12".into()),
            },
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.who.is_empty() => out.who = a.clone(),
            _ => return Err(format!("one session only (`{a}` is a second); a command goes after --")),
        }
    }
    if out.who.is_empty() {
        return Err("which session? (tsumugi ls lists them)".into());
    }
    if out.count.is_some() && out.percent.is_some() {
        return Err("--count and --percent do not go together (--count makes them all the same size)".into());
    }
    Ok(out)
}

/// `tsumugi log`'s choices.
#[derive(Debug, PartialEq)]
pub struct Log {
    pub who: String,
    pub path: Option<std::path::PathBuf>,
    pub stop: bool,
}

pub fn parse_log(args: &[String]) -> Result<Log, String> {
    let mut out = Log { who: String::new(), path: None, stop: false };
    for a in args {
        match a.as_str() {
            "--stop" => out.stop = true,
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ if out.who.is_empty() => out.who = a.clone(),
            _ if out.path.is_none() => out.path = Some(a.into()),
            _ => return Err(format!("one file only (`{a}` is a second)")),
        }
    }
    if out.who.is_empty() {
        return Err("which session? (tsumugi ls lists them)".into());
    }
    if out.stop && out.path.is_some() {
        return Err("--stop takes no file".into());
    }
    Ok(out)
}

/// Start or finish a session's work log as the window's key does (a
/// lane's way to make one without the key, 2.70); the file it went to.
/// `remote`: the sessions are another machine's, so the file is one of its
/// paths, given as it is.
pub fn log(client: &Client, list: &[Info], id: SessionId, l: Log, remote: bool) -> Result<String, String> {
    let info = list.iter().find(|i| i.id == id).ok_or_else(|| format!("no session {id}"))?;
    if l.stop {
        if info.logging.is_empty() {
            return Err(format!("session {id} writes no work log"));
        }
        client.log(id, None);
        // Asked after it: the server has finished the file once this answers.
        client.list().map_err(|e| e.to_string())?;
        return Ok(info.logging.clone());
    }
    if !info.logging.is_empty() {
        return Err(format!("session {id} writes a work log to {} already (--stop finishes it)", info.logging));
    }
    let path = match l.path {
        Some(p) if remote => p,
        // The server's folder is not this one.
        Some(p) => std::path::absolute(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        None if remote => return Err("give the file on that machine (--host)".into()),
        None => crate::export::log_path(&crate::sort::display_title(&info.title, &info.command))
            .ok_or("no home folder to write the work log in")?,
    };
    client.log(id, Some(path.clone()));
    let now = client.list().map_err(|e| e.to_string())?;
    match now.iter().find(|i| i.id == id) {
        Some(i) if !i.logging.is_empty() => Ok(i.logging.clone()),
        Some(_) => Err(format!("the work log did not start: {}", path.display())),
        None => Err(format!("session {id} ended")),
    }
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
            "--timeout" => {
                out.timeout = Some(std::time::Duration::from_secs_f64(
                    it.next().and_then(|n| n.parse::<f64>().ok()).filter(|n| *n >= 0.0).ok_or("--timeout takes seconds")?,
                ))
            }
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

/// The title column of `tsumugi ls`: what the window calls the session
/// (pwsh's exe path as `pwsh`, less a spinner), empty when the program set
/// none -- the command column names it. `--json` keeps the title as it is.
pub fn ls_title(i: &Info) -> String {
    if sort::without_spinner(&i.title).is_empty() {
        String::new()
    } else {
        sort::display_title(&i.title, &i.command)
    }
}

/// One row of `tsumugi ls`: id, state, command, folder, title, note, tags (tab-separated).
pub fn ls_row(i: &Info) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}", i.id, i.state.word(), i.command, i.cwd.display(), ls_title(i), i.note, i.tags.join(","))
}

/// What `tsumugi ls` prints, for the clipboard.
pub fn ls_text(list: &[Info]) -> String {
    list.iter().map(ls_row).collect::<Vec<_>>().join("\n")
}

/// A session in words, to tell another session which one is meant: the
/// number `tsumugi send`/`read` takes, the shown name, the folder and more.
pub fn session_info(i: &Info, tab_name: &str) -> String {
    let mut s = format!("session {} ({})\ntab: {}\ncommand: {}\nfolder: {}", i.id, i.state.word(), tab_name, i.command, i.cwd.display());
    for (k, v) in [("title", ls_title(i)), ("branch", i.branch.clone()), ("note", i.note.clone()), ("tags", i.tags.join(","))] {
        if !v.is_empty() {
            s.push_str(&format!("\n{k}: {v}"));
        }
    }
    s
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
                    ("read_only".into(), Json::Bool(i.read_only)),
                    ("profile".into(), s(&i.profile)),
                ])
            })
            .collect(),
    )
    .pretty()
}

/// The share the pane being split keeps when `left` panes (it and the ones
/// still to come) are to end up the same size: one of `left`.
pub fn equal_share(left: u8) -> f32 {
    1.0 / f32::from(left.max(1))
}

/// New panes beside session `id`, in its folder, running `command`; their
/// numbers. With a count, a chain of them, each beside the one before, so
/// that with `id` they are all the same size.
pub fn split(client: &Client, list: &[Info], id: SessionId, s: &Split) -> Result<Vec<SessionId>, String> {
    let info = list.iter().find(|i| i.id == id).ok_or_else(|| format!("no session {id}"))?;
    let dir = if s.down { tsumugi_mux::Dir::Down } else { tsumugi_mux::Dir::Right };
    let mut made = Vec::new();
    let mut from = id;
    for step in 0..s.count.map_or(1, |n| n - 1) {
        let pane = client
            .spawn_typing(info.cwd.clone(), None, Size::new(80, 24), (8, 16), Place::Split { beside: from, dir }, s.command.clone())
            .map_err(|e| format!("the pane did not start: {e}"))?;
        let new = pane.id();
        let share = match (s.count, s.percent) {
            (Some(n), _) => Some(equal_share(n - step)),
            (None, Some(p)) => Some(1.0 - f32::from(p) / 100.0),
            _ => None,
        };
        if let Some(share) = share {
            let tab = tab_of(client, new, 2)?;
            let mut tree = tab.layout.clone();
            if set_share(&mut tree, from, new, share) {
                client.set_layout(tab.id, tree, tab.focus);
                client.list().map_err(|e| e.to_string())?;
            }
        }
        made.push(new);
        from = new;
    }
    Ok(made)
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
        assert_eq!(
            take_host(&words("3 --host box --down -- ssh --host x"), false).unwrap(),
            (Some("box".into()), words("3 --down -- ssh --host x"))
        );
        assert_eq!(take_host(&words("--json"), false).unwrap(), (None, words("--json")));
        // `send`'s text is typed as it is.
        assert_eq!(take_host(&words("--host box 3 echo --host y"), true).unwrap(), (Some("box".into()), words("3 echo --host y")));
        assert!(take_host(&words("3 --host"), false).is_err());
    }

    #[test]
    fn the_remote_controls_words_are_read() {
        assert_eq!(parse_read(&words("3 --lines 10")).unwrap(), Read { who: "3".into(), lines: 10, all: false });
        assert!(parse_read(&words("--all")).is_err(), "which session");
        assert_eq!(
            parse_split(&words("filer --down -- npm run dev")).unwrap(),
            Split { who: "filer".into(), down: true, percent: None, count: None, command: words("npm run dev") }
        );
        assert_eq!(parse_split(&words("3 -p 30%")).unwrap().percent, Some(30));
        assert!(parse_split(&words("3 -p 0")).is_err());
        assert!(parse_split(&words("3 --percent")).is_err());
        assert_eq!(parse_layout(&words("Dev ~/x")).unwrap(), LayoutCmd::Open { name: "Dev".into(), folder: Some("~/x".into()) });
        assert_eq!(parse_layout(&words("--list")).unwrap(), LayoutCmd::List);
        assert!(parse_layout(&words("")).is_err());
        assert!(parse_layout(&words("a b c")).is_err());
        let mut tree = tsumugi_mux::Node::Split {
            dir: tsumugi_mux::Dir::Right,
            ratio: 0.5,
            first: Box::new(tsumugi_mux::Node::Leaf(1)),
            second: Box::new(tsumugi_mux::Node::Leaf(2)),
        };
        assert!(set_share(&mut tree, 1, 2, 0.7));
        assert!(matches!(tree, tsumugi_mux::Node::Split { ratio, .. } if (ratio - 0.7).abs() < 1e-6));
        assert!(!set_share(&mut tree, 2, 1, 0.3), "only the pair in that order");
        assert_eq!(parse_split(&words("3 -n 4")).unwrap().count, Some(4));
        assert!(parse_split(&words("3 -n 1")).is_err());
        assert!(parse_split(&words("3 -n 4 -p 30")).is_err());
        // 4 in a row: 1/4, then 1/3 of the rest, then 1/2: all a quarter.
        let shares: Vec<f32> = (0..3).map(|step| equal_share(4 - step)).collect();
        let sizes = [
            shares[0],
            (1.0 - shares[0]) * shares[1],
            (1.0 - shares[0]) * (1.0 - shares[1]) * shares[2],
            (1.0 - shares[0]) * (1.0 - shares[1]) * (1.0 - shares[2]),
        ];
        assert!(sizes.iter().all(|s| (s - 0.25).abs() < 1e-6), "{sizes:?}");
        assert_eq!(parse_log(&words("3 out.txt")).unwrap(), Log { who: "3".into(), path: Some("out.txt".into()), stop: false });
        assert_eq!(parse_log(&words("3 --stop")).unwrap(), Log { who: "3".into(), path: None, stop: true });
        assert!(parse_log(&words("3 a.txt --stop")).is_err());
        let w = parse_wait(&words("2 --state waiting --timeout 1.5")).unwrap();
        assert_eq!((w.states.len(), w.timeout), (2, Some(std::time::Duration::from_millis(1500))), "waiting takes probably waiting too");
        assert!(parse_wait(&words("2 --state soon")).is_err());
        assert_eq!(last_lines("a\nb\nc\n\n\n", 2), "b\nc\n");
    }
}
