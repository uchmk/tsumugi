//! The sidebar's order and filters (the design's 1b): which tabs show, and
//! in what order. Only arranges; the server keeps the order tabs were
//! dragged into, which `Manual` shows as it is.

use std::path::{Path, PathBuf};

use tsumugi_mux::{Info, State, Workspace};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// As dragged (the default): the sidebar does not move under the hand.
    #[default]
    Manual,
    /// Waiting, then errors, then running, then done; the longest waiting
    /// first.
    Needs,
    /// The tab whose state changed last first.
    Recent,
    /// By project, the tabs of one together.
    Folder,
    Name,
}

impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Manual, Sort::Needs, Sort::Recent, Sort::Folder, Sort::Name];

    /// In the menu.
    pub fn label(self) -> &'static str {
        match self {
            Sort::Manual => "Manual (drag)",
            Sort::Needs => "Needs me first",
            Sort::Recent => "Recent activity",
            Sort::Folder => "Folder",
            Sort::Name => "Name",
        }
    }

    /// On the button.
    pub fn short(self) -> &'static str {
        match self {
            Sort::Manual => "Manual",
            Sort::Needs => "Needs me",
            Sort::Recent => "Recent",
            Sort::Folder => "Folder",
            Sort::Name => "Name",
        }
    }

    /// As kept in the file between runs.
    pub fn word(self) -> &'static str {
        match self {
            Sort::Manual => "manual",
            Sort::Needs => "needs",
            Sort::Recent => "recent",
            Sort::Folder => "folder",
            Sort::Name => "name",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        Sort::ALL.into_iter().find(|s| s.word() == w)
    }
}

/// A tab's state for the filter: the most urgent of its panes', the guess
/// counted as waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Waiting,
    Running,
    Error,
    Done,
    /// A plain shell: no agent in it, so running and done say nothing.
    Shell,
}

/// No agent in the session: Claude Code is known by its program, any other
/// by having told tsumugi something (`tsumugi notify`). At its prompt or
/// running a command, such a session is just a shell.
pub fn is_shell(info: &Info) -> bool {
    !info.claude && info.note.is_empty() && matches!(info.state, State::Running | State::Done)
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Waiting, Kind::Running, Kind::Error, Kind::Done, Kind::Shell];

    pub fn of(info: &Info) -> Self {
        if is_shell(info) {
            return Kind::Shell;
        }
        match info.state {
            State::Waiting | State::MaybeWaiting => Kind::Waiting,
            State::Running => Kind::Running,
            State::Error => Kind::Error,
            State::Done => Kind::Done,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Waiting => "Waiting",
            Kind::Running => "Running",
            Kind::Error => "Error",
            Kind::Done => "Done",
            Kind::Shell => "Shell",
        }
    }

    /// The state its dot is coloured as; `None` for a shell, which is grey.
    pub fn state(self) -> Option<State> {
        match self {
            Kind::Waiting => Some(State::Waiting),
            Kind::Running => Some(State::Running),
            Kind::Error => Some(State::Error),
            Kind::Done => Some(State::Done),
            Kind::Shell => None,
        }
    }
}

/// What the sidebar shows only; each `None` lets everything through, and
/// they combine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub kind: Option<Kind>,
    pub project: Option<PathBuf>,
    pub tag: Option<String>,
}

/// One tab as the sidebar sees it.
pub struct Tab<'a> {
    pub workspace: &'a Workspace,
    pub infos: Vec<&'a Info>,
}

impl<'a> Tab<'a> {
    pub fn new(workspace: &'a Workspace, sessions: &'a [Info]) -> Self {
        let infos = workspace.layout.leaves().iter().filter_map(|id| sessions.iter().find(|i| i.id == *id)).collect();
        Self { workspace, infos }
    }

    /// The pane with the keys, whose name and folder the tab shows.
    pub fn focus(&self) -> Option<&'a Info> {
        self.infos.iter().find(|i| i.id == self.workspace.focus).or(self.infos.first()).copied()
    }

    /// The pane that most wants a person, whose state the tab shows.
    pub fn urgent(&self) -> Option<&'a Info> {
        self.infos.iter().max_by_key(|i| (urgency(i.state), std::cmp::Reverse(i.since_ms))).copied()
    }

    pub fn kind(&self) -> Option<Kind> {
        self.urgent().map(Kind::of)
    }

    pub fn project(&self) -> Option<&'a Path> {
        self.focus().map(|i| i.project.as_path())
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.infos.iter().any(|i| i.tags.iter().any(|t| t == tag))
    }

    /// What the row says and `Name` sorts by: the name given by hand, else
    /// the title, else the program.
    pub fn name(&self) -> String {
        if !self.workspace.name.is_empty() {
            return self.workspace.name.clone();
        }
        self.focus().map(|i| display_title(&i.title, &i.command)).unwrap_or_default()
    }

    /// When any of its panes last changed state.
    fn last_change(&self) -> u64 {
        self.infos.iter().map(|i| i.since_ms).max().unwrap_or(0)
    }
}

/// The most urgent state among a tab's panes, which its row shows.
pub fn urgency(state: State) -> u8 {
    match state {
        State::Waiting => 4,
        State::Error => 3,
        State::MaybeWaiting => 2,
        State::Running => 1,
        State::Done => 0,
    }
}

/// What a session is called: its title, but a title that is only the path
/// of the program run (pwsh and cmd set the window's title to their exe,
/// `C:\Program Files\WindowsApps\…\pwsh.exe`) as the program's name, and
/// no title as the program.
pub fn display_title(title: &str, command: &str) -> String {
    let t = title.trim();
    if t.is_empty() {
        return program_name(command);
    }
    let path_like = (t.contains('\\') || t.contains('/')) && !t.contains(' ') || t.to_ascii_lowercase().ends_with(".exe");
    let is_program = t == command || program_name(t).eq_ignore_ascii_case(&program_name(command));
    if path_like && (is_program || t.to_ascii_lowercase().ends_with(".exe")) { program_name(t) } else { t.to_owned() }
}

/// A program's name without its folder or `.exe`.
/// Both separators, whatever the system: a Windows path can come to a
/// window on another machine's server, and the other way round.
pub fn program_name(command: &str) -> String {
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    match name.len().checked_sub(4).filter(|&at| name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".exe")) {
        Some(at) => name[..at].to_owned(),
        None => name.to_owned(),
    }
}

/// The tabs to show, in the order to show them. `tabs` is in the server's
/// (dragged) order.
pub fn arrange<'a, 'b>(tabs: &'b [Tab<'a>], sort: Sort, filter: &Filter) -> Vec<&'b Tab<'a>> {
    let mut shown: Vec<&Tab> = tabs
        .iter()
        .filter(|t| filter.kind.is_none_or(|k| t.kind() == Some(k)))
        .filter(|t| filter.project.as_deref().is_none_or(|p| t.project() == Some(p)))
        .filter(|t| filter.tag.as_deref().is_none_or(|g| t.has_tag(g)))
        .collect();
    // Stable sorts: ties keep the dragged order.
    match sort {
        Sort::Manual => {}
        Sort::Needs => shown.sort_by_key(|t| {
            let rank = match t.kind() {
                Some(Kind::Waiting) => 0,
                Some(Kind::Error) => 1,
                Some(Kind::Running) => 2,
                Some(Kind::Done) => 3,
                Some(Kind::Shell) | None => 4,
            };
            // The one that has waited longest first.
            (rank, t.urgent().map_or(u64::MAX, |i| i.since_ms))
        }),
        Sort::Recent => shown.sort_by_key(|t| std::cmp::Reverse(t.last_change())),
        Sort::Folder => shown.sort_by(|a, b| a.project().cmp(&b.project())),
        Sort::Name => shown.sort_by_key(|t| t.name().to_lowercase()),
    }
    // Pinned ones at the top, in that same order.
    shown.sort_by_key(|t| !t.workspace.pinned);
    shown
}

/// How tall the sidebar's rows are (the design's 1i).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    /// The three-line card (the default).
    #[default]
    Cards,
    /// One line each, 28px: twenty in 600px. The tab shown still opens to
    /// its card.
    Lines,
}

impl Density {
    pub fn word(self) -> &'static str {
        match self {
            Density::Cards => "cards",
            Density::Lines => "lines",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        [Density::Cards, Density::Lines].into_iter().find(|d| d.word() == w)
    }
}

/// One thing in the sidebar's list.
pub enum Item<'b, 'a> {
    /// A project's heading, when sorted by folder (the design's 1i B).
    Group { project: PathBuf, total: usize, kinds: Vec<Kind>, open: bool },
    /// A tab, as a card or as a line.
    Tab { tab: &'b Tab<'a>, card: bool },
    /// The tabs of an open group left out because nothing in them wants a
    /// person, by state.
    More { project: PathBuf, kinds: Vec<Kind> },
}

/// The sidebar's list. Sorted by folder, the tabs come under their
/// project's heading; a closed group shows no tabs, an open one only those
/// that want a person (waiting, error) and the one shown, unless it was
/// opened all the way (`whole`). `Lines` makes every tab but the shown one
/// a line.
pub fn items<'b, 'a>(
    shown: &[&'b Tab<'a>],
    sort: Sort,
    density: Density,
    closed: &[PathBuf],
    whole: &[PathBuf],
    active: Option<tsumugi_mux::WorkspaceId>,
) -> Vec<Item<'b, 'a>> {
    let card = |t: &Tab| density == Density::Cards || Some(t.workspace.id) == active;
    if sort != Sort::Folder {
        return shown.iter().map(|t| Item::Tab { tab: t, card: card(t) }).collect();
    }
    let mut out = Vec::new();
    let mut at = 0;
    while at < shown.len() {
        let project = shown[at].project().map(Path::to_path_buf).unwrap_or_default();
        let end = shown[at..].iter().position(|t| t.project().map(Path::to_path_buf).unwrap_or_default() != project).map_or(shown.len(), |n| at + n);
        let group = &shown[at..end];
        let open = !closed.contains(&project);
        let kinds: Vec<Kind> = group.iter().filter_map(|t| t.kind()).collect();
        out.push(Item::Group { project: project.clone(), total: group.len(), kinds, open });
        if open {
            let all = whole.contains(&project);
            let mut left = Vec::new();
            for t in group {
                let wants = matches!(t.kind(), Some(Kind::Waiting | Kind::Error));
                if all || wants || Some(t.workspace.id) == active {
                    out.push(Item::Tab { tab: t, card: card(t) });
                } else if let Some(k) = t.kind() {
                    left.push(k);
                }
            }
            if !left.is_empty() {
                out.push(Item::More { project: project.clone(), kinds: left });
            }
        }
        at = end;
    }
    out
}

/// "4 running, 2 done": how many of each.
pub fn count_words(kinds: &[Kind]) -> String {
    Kind::ALL
        .iter()
        .filter_map(|k| {
            let n = kinds.iter().filter(|x| *x == k).count();
            (n > 0).then(|| format!("{n} {}", k.label().to_lowercase()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The projects among the tabs, each with how many tabs it has, in the
/// order first seen.
pub fn projects(tabs: &[Tab]) -> Vec<(PathBuf, usize)> {
    let mut out: Vec<(PathBuf, usize)> = Vec::new();
    for p in tabs.iter().filter_map(Tab::project) {
        match out.iter_mut().find(|(q, _)| q == p) {
            Some((_, n)) => *n += 1,
            None => out.push((p.to_path_buf(), 1)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsumugi_mux::Node;

    fn info(id: u64, state: State, since_ms: u64, project: &str, title: &str) -> Info {
        Info {
            id,
            cwd: project.into(),
            title: title.into(),
            command: "/bin/bash".into(),
            state,
            note: String::new(),
            since_ms,
            branch: String::new(),
            project: project.into(),
            muted: false,
            tags: if id == 3 { vec!["ci".into()] } else { vec![] },
            // Agents, so running and done are kinds of their own.
            claude: true,
            conversation: String::new(),
            charset: String::new(),
        }
    }

    #[test]
    fn a_session_without_an_agent_is_a_shell() {
        let mut i = info(1, State::Running, 0, "/p/a", "bash");
        i.claude = false;
        assert_eq!(Kind::of(&i), Kind::Shell, "a shell running a command");
        i.state = State::Done;
        assert_eq!(Kind::of(&i), Kind::Shell, "and at its prompt");
        i.state = State::Waiting;
        assert_eq!(Kind::of(&i), Kind::Waiting, "a wait is a wait");
        i.state = State::Done;
        i.note = "Wrote the scope".into();
        assert_eq!(Kind::of(&i), Kind::Done, "it told tsumugi: an agent");
    }

    fn order(shown: &[&Tab]) -> Vec<u64> {
        shown.iter().map(|t| t.workspace.focus).collect()
    }

    #[test]
    fn each_sort_and_filter() {
        let sessions = vec![
            info(1, State::Done, 50, "/p/tsumugi", "beta"),
            info(2, State::Waiting, 30, "/p/filer", "Alpha"),
            info(3, State::Error, 40, "/p/filer", "gamma"),
            info(4, State::MaybeWaiting, 10, "/p/tsumugi", ""),
            info(5, State::Running, 20, "/p/filer", "delta"),
        ];
        let mut workspaces: Vec<Workspace> = (1..=5).map(|id| Workspace::new(id + 100, Node::Leaf(id), id)).collect();
        let tabs: Vec<Tab> = workspaces.iter().map(|w| Tab::new(w, &sessions)).collect();
        let all = Filter::default();
        assert_eq!(order(&arrange(&tabs, Sort::Manual, &all)), [1, 2, 3, 4, 5]);
        // The guess counts as waiting, and has waited longer.
        assert_eq!(order(&arrange(&tabs, Sort::Needs, &all)), [4, 2, 3, 5, 1]);
        assert_eq!(order(&arrange(&tabs, Sort::Recent, &all)), [1, 3, 2, 5, 4]);
        assert_eq!(order(&arrange(&tabs, Sort::Folder, &all)), [2, 3, 5, 1, 4]);
        // No title: the program's name; case does not count.
        assert_eq!(order(&arrange(&tabs, Sort::Name, &all)), [2, 4, 1, 5, 3]);

        let waiting = Filter { kind: Some(Kind::Waiting), ..Filter::default() };
        assert_eq!(order(&arrange(&tabs, Sort::Manual, &waiting)), [2, 4]);
        let filer = Filter { project: Some("/p/filer".into()), ..Filter::default() };
        assert_eq!(order(&arrange(&tabs, Sort::Name, &filer)), [2, 5, 3]);
        let both = Filter { kind: Some(Kind::Waiting), ..filer.clone() };
        assert_eq!(order(&arrange(&tabs, Sort::Manual, &both)), [2]);
        let ci = Filter { tag: Some("ci".into()), ..Filter::default() };
        assert_eq!(order(&arrange(&tabs, Sort::Manual, &ci)), [3]);

        assert_eq!(projects(&tabs), vec![(PathBuf::from("/p/tsumugi"), 2), (PathBuf::from("/p/filer"), 3)]);

        // A pinned tab is first whatever the order.
        workspaces[4].pinned = true;
        let tabs: Vec<Tab> = workspaces.iter().map(|w| Tab::new(w, &sessions)).collect();
        assert_eq!(order(&arrange(&tabs, Sort::Manual, &all)), [5, 1, 2, 3, 4]);
        assert_eq!(order(&arrange(&tabs, Sort::Needs, &all)), [5, 4, 2, 3, 1]);
        // A name given by hand is the one sorted by.
        workspaces[0].name = "aaa".into();
        let tabs: Vec<Tab> = workspaces.iter().map(|w| Tab::new(w, &sessions)).collect();
        assert_eq!(order(&arrange(&tabs, Sort::Name, &all)), [5, 1, 2, 4, 3]);
    }

    #[test]
    fn grouped_by_folder_only_what_wants_a_person_shows() {
        let sessions = vec![
            info(1, State::Running, 1, "/p/a", "a1"),
            info(2, State::Waiting, 2, "/p/a", "a2"),
            info(3, State::Done, 3, "/p/a", "a3"),
            info(4, State::Running, 4, "/p/b", "b1"),
        ];
        let workspaces: Vec<Workspace> = (1..=4).map(|id| Workspace::new(id + 100, Node::Leaf(id), id)).collect();
        let tabs: Vec<Tab> = workspaces.iter().map(|w| Tab::new(w, &sessions)).collect();
        let shown = arrange(&tabs, Sort::Folder, &Filter::default());
        let words = |items: &[Item]| {
            items
                .iter()
                .map(|i| match i {
                    Item::Group { project, total, open, .. } => format!("{}:{total}{}", project.display(), if *open { "" } else { " closed" }),
                    Item::Tab { tab, card } => format!("{}{}", tab.workspace.focus, if *card { "" } else { "-" }),
                    Item::More { kinds, .. } => format!("+{}", count_words(kinds)),
                })
                .collect::<Vec<_>>()
        };
        // Tab 1 is the one shown, so it stays.
        let listed = items(&shown, Sort::Folder, Density::Cards, &[], &[], Some(101));
        assert_eq!(words(&listed), ["/p/a:3", "1", "2", "+1 done", "/p/b:1", "+1 running"]);
        let closed = [PathBuf::from("/p/a")];
        let whole = [PathBuf::from("/p/b")];
        assert_eq!(words(&items(&shown, Sort::Folder, Density::Lines, &closed, &whole, None)), ["/p/a:3 closed", "/p/b:1", "4-"]);
        // Not by folder: no headings, lines but for the one shown.
        let flat = items(&shown, Sort::Manual, Density::Lines, &[], &[], Some(102));
        assert_eq!(words(&flat), ["1-", "2", "3-", "4-"]);
    }

    #[test]
    fn a_title_that_is_the_programs_path_is_its_name() {
        let pwsh = r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.6.0_x64__8wekyb3d8bbwe\pwsh.exe";
        assert_eq!(display_title(pwsh, pwsh), "pwsh");
        assert_eq!(display_title(r"C:\WINDOWS\system32\cmd.exe", "cmd.exe"), "cmd");
        assert_eq!(display_title("/bin/bash", "/bin/bash"), "bash");
        assert_eq!(display_title("", "/usr/bin/zsh"), "zsh");
        // A title of its own stays, paths and all.
        assert_eq!(display_title("root@vm: /tmp/proj", "/bin/bash"), "root@vm: /tmp/proj");
        assert_eq!(display_title("✳ Fix the zoom badge", "pwsh.exe"), "✳ Fix the zoom badge");
        assert_eq!(display_title("~/dev/filer", "/bin/bash"), "~/dev/filer");
    }

    #[test]
    fn the_sort_is_kept_by_its_word() {
        for s in Sort::ALL {
            assert_eq!(Sort::from_word(s.word()), Some(s));
        }
        assert_eq!(Sort::from_word("other"), None);
    }
}
