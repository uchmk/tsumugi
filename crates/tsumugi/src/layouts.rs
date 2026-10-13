//! Layouts kept to open again: a tab's panes, how they are split, and what
//! each starts (`claude`, `resume` or `shell`), in `layouts.toml` beside the
//! settings. Opened from the search box in a folder, they make a new tab.
//!
//! A pane is its start word, or a table that says more (`run`: a command to
//! type into the shell, `dir`: a folder under the tab's, `start`: the word
//! when there is no `run`); a split is a table with `right` or `down` (the
//! first pane's share) and `first` and `second`:
//!
//! ```toml
//! [[layout]]
//! name = "Review"
//! tree = { right = 0.6, first = "claude", second = { down = 0.5, first = { run = "npm run dev", dir = "web" }, second = "claude" } }
//! ```

use std::path::PathBuf;

use ito_layout::{Dir, Node, MIN_RATIO};

use crate::newsession::Start;
use tsumugi_mux::SessionId;

/// What one pane of a layout starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Pane {
    pub start: Start,
    /// A command typed into the shell instead of what `start` types; empty: none.
    pub run: String,
    /// A folder under the tab's (or a whole one); empty: the tab's own.
    pub dir: String,
}

impl Pane {
    pub fn of(start: Start) -> Self {
        Self { start, run: String::new(), dir: String::new() }
    }

    /// The line typed into the new shell, if any.
    pub fn typed(&self, claude: &str) -> Vec<String> {
        match self.run.trim() {
            "" => self.start.typed(claude).into_iter().collect(),
            run => vec![run.to_owned()],
        }
    }

    /// Where it starts, given the layout's folder.
    pub fn folder(&self, base: &std::path::Path) -> PathBuf {
        match self.dir.trim() {
            "" => base.to_owned(),
            d => base.join(crate::newsession::expand(d)),
        }
    }

    /// As the search box shows it: the word, or the command.
    pub fn word(&self) -> String {
        match self.run.trim() {
            "" => self.start.word().to_owned(),
            run => run.to_owned(),
        }
    }

    fn to_toml(&self) -> toml::Value {
        if self.run.is_empty() && self.dir.is_empty() {
            return toml::Value::String(self.start.word().into());
        }
        let mut t = toml::Table::new();
        if self.run.is_empty() {
            t.insert("start".into(), toml::Value::String(self.start.word().into()));
        } else {
            t.insert("run".into(), toml::Value::String(self.run.clone()));
        }
        if !self.dir.is_empty() {
            t.insert("dir".into(), toml::Value::String(self.dir.clone()));
        }
        toml::Value::Table(t)
    }

    fn from_toml(v: &toml::Value) -> Option<Self> {
        if let Some(w) = v.as_str() {
            return Start::from_word(w.trim()).map(Self::of);
        }
        let t = v.as_table()?;
        let text = |k: &str| t.get(k).and_then(toml::Value::as_str).map(|s| s.trim().to_owned());
        let start = match text("start") {
            Some(w) => Start::from_word(&w)?,
            None => Start::Shell,
        };
        let (run, dir) = (text("run").unwrap_or_default(), text("dir").unwrap_or_default());
        (!run.is_empty() || !dir.is_empty() || t.contains_key("start")).then_some(Self { start, run, dir })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub name: String,
    pub tree: Node<Pane>,
}

/// The file beside the settings.
pub fn file() -> Option<PathBuf> {
    tsumugi_mux::settings::default_path().map(|p| p.with_file_name("layouts.toml"))
}

fn node_to_toml(n: &Node<Pane>) -> toml::Value {
    match n {
        Node::Leaf(p) => p.to_toml(),
        Node::Split { dir, ratio, first, second } => {
            let mut t = toml::Table::new();
            let key = if *dir == Dir::Right { "right" } else { "down" };
            // Two places are enough, and read better than 0.6000000238.
            t.insert(key.into(), toml::Value::Float((f64::from(*ratio) * 100.0).round() / 100.0));
            t.insert("first".into(), node_to_toml(first));
            t.insert("second".into(), node_to_toml(second));
            toml::Value::Table(t)
        }
    }
}

fn node_from_toml(v: &toml::Value) -> Option<Node<Pane>> {
    let t = v.as_table().filter(|t| t.contains_key("right") || t.contains_key("down"));
    let Some(t) = t else { return Pane::from_toml(v).map(Node::Leaf) };
    let number = |k: &str| t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)));
    let (dir, ratio) = match (number("right"), number("down")) {
        (Some(r), None) => (Dir::Right, r),
        (None, Some(r)) => (Dir::Down, r),
        _ => return None,
    };
    let ratio = (ratio as f32).clamp(MIN_RATIO, 1.0 - MIN_RATIO);
    Some(Node::Split { dir, ratio, first: Box::new(node_from_toml(t.get("first")?)?), second: Box::new(node_from_toml(t.get("second")?)?) })
}

/// The layouts written as TOML, `[[layout]]` each.
pub fn to_toml(list: &[Layout]) -> String {
    let items: Vec<toml::Value> = list
        .iter()
        .map(|l| {
            let mut t = toml::Table::new();
            t.insert("name".into(), toml::Value::String(l.name.clone()));
            t.insert("tree".into(), node_to_toml(&l.tree));
            toml::Value::Table(t)
        })
        .collect();
    let mut top = toml::Table::new();
    top.insert("layout".into(), toml::Value::Array(items));
    format!("# Saved layouts. A pane is \"claude\", \"resume\" or \"shell\"; a split is {{ right = share, first, second }} or {{ down = … }}.\n{}", toml::to_string(&top).unwrap_or_default())
}

/// The layouts read back; what does not read is left out.
pub fn from_toml(text: &str) -> Vec<Layout> {
    let Ok(top) = text.parse::<toml::Table>() else { return Vec::new() };
    let Some(items) = top.get("layout").and_then(toml::Value::as_array) else { return Vec::new() };
    items
        .iter()
        .filter_map(|v| {
            let t = v.as_table()?;
            Some(Layout { name: t.get("name")?.as_str()?.to_owned(), tree: node_from_toml(t.get("tree")?)? })
        })
        .collect()
}

pub fn load() -> Vec<Layout> {
    file().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| from_toml(&t)).unwrap_or_default()
}

/// Write them on a thread.
pub fn save(list: Vec<Layout>) {
    let _ = std::thread::Builder::new().name("layouts".into()).spawn(move || {
        let Some(p) = file() else { return };
        // As with the prompts: a file that does not read as TOML is kept.
        match crate::files::read_or_empty(&p) {
            Ok(old) if old.trim().is_empty() || old.parse::<toml::Table>().is_ok() => {
                if let Err(e) = crate::files::write_atomic(&p, to_toml(&list)) {
                    eprintln!("tsumugi: the layouts: {e}");
                }
            }
            Ok(_) => eprintln!("tsumugi: {} does not read as TOML, so it is left as it is", p.display()),
            Err(e) => eprintln!("tsumugi: the layouts: {e}"),
        }
    });
}

/// `list` with `layout` in it: one of the same name is replaced.
pub fn put(list: &mut Vec<Layout>, layout: Layout) {
    match list.iter_mut().find(|l| l.name == layout.name) {
        Some(old) => *old = layout,
        None => list.push(layout),
    }
}

/// The panes' words, as the search box shows them: "claude, shell, claude".
pub fn words(tree: &Node<Pane>) -> String {
    tree.leaves().iter().map(Pane::word).collect::<Vec<_>>().join(", ")
}

/// The sessions started for a layout, as its panes' shape: `ids` in the
/// order of the tree's panes.
pub fn shaped(tree: &Node<Pane>, ids: &[SessionId]) -> Option<Node<SessionId>> {
    let first = *ids.first()?;
    let mut it = ids.iter();
    Some(tree.clone().map(&mut |_| it.next().copied().unwrap_or(first)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(s: Start) -> Node<Pane> {
        Node::Leaf(Pane::of(s))
    }

    fn split(dir: Dir, ratio: f32, a: Node<Pane>, b: Node<Pane>) -> Node<Pane> {
        Node::Split { dir, ratio, first: Box::new(a), second: Box::new(b) }
    }

    #[test]
    fn layouts_come_back_as_they_were_written() {
        let tree = split(Dir::Right, 0.6, leaf(Start::Claude), split(Dir::Down, 0.5, leaf(Start::Shell), leaf(Start::Resume)));
        let list = vec![Layout { name: "Review".into(), tree }, Layout { name: "One".into(), tree: leaf(Start::Shell) }];
        assert_eq!(from_toml(&to_toml(&list)), list);
        assert_eq!(words(&list[0].tree), "claude, shell, resume");
        assert!(from_toml("not = [toml").is_empty());
    }

    #[test]
    fn a_hand_written_layout_reads_and_a_wrong_one_is_left_out() {
        let text = "[[layout]]\nname = \"A\"\ntree = { down = 2, first = \"claude\", second = \"shell\" }\n\n[[layout]]\nname = \"B\"\ntree = { right = 0.5, first = \"vim\", second = \"shell\" }\n";
        let list = from_toml(text);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tree, split(Dir::Down, 1.0 - MIN_RATIO, leaf(Start::Claude), leaf(Start::Shell)));
    }

    #[test]
    fn a_pane_may_run_a_command_in_a_folder() {
        let text = "[[layout]]\nname = \"Dev\"\ntree = { right = 0.5, first = { run = \"npm run dev\", dir = \"web\" }, second = { start = \"claude\", dir = \"~/x\" } }\n";
        let list = from_toml(text);
        let Node::Split { first, second, .. } = &list[0].tree else { panic!("a split") };
        assert_eq!(**first, Node::Leaf(Pane { start: Start::Shell, run: "npm run dev".into(), dir: "web".into() }));
        assert_eq!(**second, Node::Leaf(Pane { start: Start::Claude, run: String::new(), dir: "~/x".into() }));
        assert_eq!(from_toml(&to_toml(&list)), list);
        assert_eq!(words(&list[0].tree), "npm run dev, claude");
        let p = Pane { start: Start::Claude, run: String::new(), dir: "web".into() };
        assert_eq!(p.typed("claude"), vec!["claude".to_owned()]);
        assert_eq!(p.folder(std::path::Path::new("/a")), std::path::Path::new("/a/web"));
        assert_eq!(from_toml(&to_toml(&list))[0].tree.leaves().len(), 2);
        assert!(from_toml("[[layout]]\nname = \"X\"\ntree = {}\n").is_empty(), "an empty table is no pane");
    }

    #[test]
    fn saving_under_a_name_there_is_replaces_it() {
        let mut list = vec![Layout { name: "A".into(), tree: leaf(Start::Shell) }];
        put(&mut list, Layout { name: "A".into(), tree: leaf(Start::Claude) });
        put(&mut list, Layout { name: "B".into(), tree: leaf(Start::Shell) });
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].tree, leaf(Start::Claude));
    }
}
