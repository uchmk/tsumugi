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
