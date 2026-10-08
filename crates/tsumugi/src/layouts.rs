//! Layouts kept to open again: a tab's panes, how they are split, and what
//! each starts (`claude`, `resume` or `shell`), in `layouts.toml` beside the
//! settings. Opened from the search box in a folder, they make a new tab.
//!
//! A pane is its start word; a split is a table with `right` or `down` (the
//! first pane's share) and `first` and `second`:
//!
//! ```toml
//! [[layout]]
//! name = "Review"
//! tree = { right = 0.6, first = "claude", second = { down = 0.5, first = "shell", second = "claude" } }
//! ```

use std::path::PathBuf;

use tsumugi_layout::{Dir, MIN_RATIO, Node};

use crate::newsession::Start;

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub name: String,
    pub tree: Node<Start>,
}

/// The file beside the settings.
pub fn file() -> Option<PathBuf> {
    tsumugi_mux::settings::default_path().map(|p| p.with_file_name("layouts.toml"))
}

fn node_to_toml(n: &Node<Start>) -> toml::Value {
    match n {
        Node::Leaf(s) => toml::Value::String(s.word().into()),
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

fn node_from_toml(v: &toml::Value) -> Option<Node<Start>> {
    if let Some(w) = v.as_str() {
        return Start::from_word(w.trim()).map(Node::Leaf);
    }
    let t = v.as_table()?;
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
pub fn words(tree: &Node<Start>) -> String {
    tree.leaves().iter().map(|s| s.word()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(dir: Dir, ratio: f32, a: Node<Start>, b: Node<Start>) -> Node<Start> {
        Node::Split { dir, ratio, first: Box::new(a), second: Box::new(b) }
    }

    #[test]
    fn layouts_come_back_as_they_were_written() {
        let tree = split(Dir::Right, 0.6, Node::Leaf(Start::Claude), split(Dir::Down, 0.5, Node::Leaf(Start::Shell), Node::Leaf(Start::Resume)));
        let list = vec![Layout { name: "Review".into(), tree }, Layout { name: "One".into(), tree: Node::Leaf(Start::Shell) }];
        assert_eq!(from_toml(&to_toml(&list)), list);
        assert_eq!(words(&list[0].tree), "claude, shell, resume");
        assert!(from_toml("not = [toml").is_empty());
    }

    #[test]
    fn a_hand_written_layout_reads_and_a_wrong_one_is_left_out() {
        let text = "[[layout]]\nname = \"A\"\ntree = { down = 2, first = \"claude\", second = \"shell\" }\n\n[[layout]]\nname = \"B\"\ntree = { right = 0.5, first = \"vim\", second = \"shell\" }\n";
        let list = from_toml(text);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tree, split(Dir::Down, 1.0 - MIN_RATIO, Node::Leaf(Start::Claude), Node::Leaf(Start::Shell)));
    }

    #[test]
    fn saving_under_a_name_there_is_replaces_it() {
        let mut list = vec![Layout { name: "A".into(), tree: Node::Leaf(Start::Shell) }];
        put(&mut list, Layout { name: "A".into(), tree: Node::Leaf(Start::Claude) });
        put(&mut list, Layout { name: "B".into(), tree: Node::Leaf(Start::Shell) });
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].tree, Node::Leaf(Start::Claude));
    }
}
