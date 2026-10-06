//! The search box at the top of the window (the design's 1c,
//! `Ctrl+Shift+P`): sessions, folders and commands in one list, narrowed by
//! what is typed. Only finds; the window does what was picked.

use std::path::PathBuf;

use tsumugi_mux::SessionId;

use crate::sort::Sort;

/// What an entry does when picked.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// Go to the session.
    Session(SessionId),
    /// Start a session in the folder.
    Folder(PathBuf),
    /// A line found in a session's scrollback: go there and show it.
    Line { id: SessionId, line: i32, col: usize, len: usize },
    Command(Command),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    NewSession,
    SplitRight,
    SplitDown,
    Zoom,
    NextWaiting,
    CloseTab,
    Sort(Sort),
    Settings,
    InputBox,
}

impl Command {
    pub const ALL: [Command; 13] = [
        Command::NewSession,
        Command::SplitRight,
        Command::SplitDown,
        Command::Zoom,
        Command::NextWaiting,
        Command::CloseTab,
        Command::Sort(Sort::Manual),
        Command::Sort(Sort::Needs),
        Command::Sort(Sort::Recent),
        Command::Sort(Sort::Folder),
        Command::Sort(Sort::Name),
        Command::Settings,
        Command::InputBox,
    ];

    pub fn title(self) -> String {
        match self {
            Command::NewSession => "New session…".into(),
            Command::SplitRight => "Split right".into(),
            Command::SplitDown => "Split down".into(),
            Command::Zoom => "Zoom the pane in or out".into(),
            Command::NextWaiting => "Go to the session waiting longest".into(),
            Command::CloseTab => "Close the tab".into(),
            Command::Sort(s) => format!("Sort sessions: {}", s.label()),
            Command::Settings => "Settings".into(),
            Command::InputBox => "Input box: write a prompt".into(),
        }
    }

    /// The key that does the same, shown beside it.
    pub fn key(self, mac: bool) -> &'static str {
        match (self, mac) {
            (Command::NewSession, false) => "Ctrl+Shift+T",
            (Command::NewSession, true) => "Cmd+T",
            (Command::SplitRight, false) => "Alt+Shift+=",
            (Command::SplitRight, true) => "Cmd+D",
            (Command::SplitDown, false) => "Alt+Shift+-",
            (Command::SplitDown, true) => "Cmd+Shift+D",
            (Command::Zoom, false) => "Ctrl+Shift+Z",
            (Command::Zoom, true) => "Cmd+Shift+Z",
            (Command::NextWaiting, false) => "Ctrl+Shift+U",
            (Command::NextWaiting, true) => "Cmd+Shift+U",
            (Command::CloseTab, false) => "Ctrl+Shift+W",
            (Command::CloseTab, true) => "Cmd+W",
            (Command::InputBox, false) => "Ctrl+I",
            (Command::InputBox, true) => "Cmd+I",
            (Command::Settings, false) => "Ctrl+,",
            (Command::Settings, true) => "Cmd+,",
            (Command::Sort(_), _) => "",
        }
    }
}

/// The search box while it is open.
pub struct View {
    pub query: String,
    /// The line picked with the arrows.
    pub selected: usize,
    /// Opened this frame: the click that opened it is not a click outside.
    pub opening: bool,
    /// What the scrollbacks were last asked for, and when the query last
    /// changed (asked once typing pauses).
    pub asked: String,
    pub changed: std::time::Instant,
}

impl View {
    pub fn new() -> Self {
        Self { query: String::new(), selected: 0, opening: true, asked: String::new(), changed: std::time::Instant::now() }
    }
}

/// What the search box was asked to do.
pub enum Answer {
    Pick(Pick),
    Close,
}

/// One line of the list: what it says, the smaller words after it, and
/// what it does.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub title: String,
    pub detail: String,
    pub pick: Pick,
}

/// How well `query` matches `text`: its letters in order, case aside, or
/// `None`. Higher is better: letters next to each other, at the start of a
/// word, and an early start count.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0;
    let mut at = 0;
    let mut last: Option<usize> = None;
    for c in q {
        let found = t[at..].iter().position(|x| *x == c)? + at;
        score += 1;
        if last == Some(found.wrapping_sub(1)) {
            score += 5;
        }
        let word_start = found == 0 || !t[found - 1].is_alphanumeric();
        if word_start {
            score += 3;
        }
        if last.is_none() {
            score -= found.min(20) as i32 / 4;
        }
        last = Some(found);
        at = found + 1;
    }
    Some(score)
}

/// The entries that match, best first; ties keep their order (sessions,
/// then folders, then commands). Each entry is matched on its title and its
/// detail together.
pub fn search(query: &str, entries: &[Entry]) -> Vec<Entry> {
    let mut found: Vec<(i32, usize, &Entry)> =
        entries.iter().enumerate().filter_map(|(k, e)| score(query, &format!("{} {}", e.title, e.detail)).map(|s| (s, k, e))).collect();
    found.sort_by_key(|(s, k, _)| (std::cmp::Reverse(*s), *k));
    found.into_iter().map(|(_, _, e)| e.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, detail: &str) -> Entry {
        Entry { title: title.into(), detail: detail.into(), pick: Pick::Command(Command::Zoom) }
    }

    #[test]
    fn letters_in_order_match() {
        assert!(score("spr", "Split right").is_some());
        assert!(score("SR", "split right").is_some(), "case aside");
        assert_eq!(score("rs", "split"), None, "out of order");
        assert_eq!(score("", "anything"), Some(0));
    }

    #[test]
    fn the_better_match_comes_first() {
        let list = [entry("Split down", ""), entry("Settings", ""), entry("filer", "~/dev/filer · main")];
        let titles = |q: &str| search(q, &list).into_iter().map(|e| e.title).collect::<Vec<_>>();
        assert_eq!(titles("set"), ["Settings"]);
        // Together and at a word's start beats spread out.
        assert_eq!(titles("sd")[0], "Split down");
        // The detail counts too.
        assert_eq!(titles("main"), ["filer"]);
        assert_eq!(titles("").len(), 3, "nothing typed, everything");
    }
}
