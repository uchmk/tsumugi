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
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Waiting, Kind::Running, Kind::Error, Kind::Done];

    pub fn of(state: State) -> Self {
        match state {
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
        }
    }

    /// The state its dot is coloured as.
    pub fn state(self) -> State {
        match self {
            Kind::Waiting => State::Waiting,
            Kind::Running => State::Running,
            Kind::Error => State::Error,
            Kind::Done => State::Done,
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
        self.urgent().map(|i| Kind::of(i.state))
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
        self.focus().map(|i| if i.title.is_empty() { program_name(&i.command) } else { i.title.clone() }).unwrap_or_default()
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

/// A program's name without its folder or `.exe`.
pub fn program_name(command: &str) -> String {
    Path::new(command).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| command.to_owned())
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
                Some(Kind::Done) | None => 3,
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
            claude: false,
        }
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
    fn the_sort_is_kept_by_its_word() {
        for s in Sort::ALL {
            assert_eq!(Sort::from_word(s.word()), Some(s));
        }
        assert_eq!(Sort::from_word("other"), None);
    }
}
