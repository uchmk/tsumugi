//! Quick select (WezTerm's Quick Select, kitty's hints): every link, hash,
//! address and long number on a pane's screen gets a label of a letter or
//! two; typing one copies that thing, with Shift opens it. The pane gets no
//! keys while it is on. Only the labels and the keys live here; the window
//! draws them and carries out what is picked.

use tsumugi_pane::{CellView, Link};

/// The letters labels are made of, the home row first.
const LETTERS: &str = "asdfqwerzxcvjklmiuopghtybn";

/// A thing on the screen and its label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The row on the screen as it shows, and the cells.
    pub row: usize,
    pub cells: std::ops::Range<usize>,
    pub text: String,
    pub link: Option<Link>,
    pub label: String,
}

/// Quick select while it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickSelect {
    pub hits: Vec<Hit>,
    /// The letters typed so far.
    pub typed: String,
    /// Shift was held on one of them: open, not copy.
    open: bool,
}

/// What a key comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing to do yet.
    Wait,
    /// Put the text on the clipboard and leave.
    Copy(String),
    /// Open the link and leave (a hit that is no link is copied instead).
    Open(Link),
    Exit,
}

/// A key quick select reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Letter(char, bool),
    Back,
    Exit,
}

impl QuickSelect {
    /// The screen's things, labelled from the bottom up: the newest output
    /// gets the first letters. The same text twice gets one label.
    pub fn new(rows: &[Vec<CellView>]) -> Self {
        let mut found: Vec<(usize, tsumugi_pane::Hint)> = Vec::new();
        for (y, row) in rows.iter().enumerate().rev() {
            for h in tsumugi_pane::hints(row).into_iter().rev() {
                found.push((y, h));
            }
        }
        let mut texts: Vec<&str> = Vec::new();
        for (_, h) in &found {
            if !texts.contains(&h.text.as_str()) {
                texts.push(&h.text);
            }
        }
        let labels = labels(texts.len());
        let label_of = |t: &str| texts.iter().position(|x| *x == t).and_then(|i| labels.get(i).cloned());
        let hits = found.iter().filter_map(|(y, h)| Some(Hit { row: *y, cells: h.cells.clone(), text: h.text.clone(), link: h.link.clone(), label: label_of(&h.text)? })).collect();
        Self { hits, typed: String::new(), open: false }
    }

    /// The labels moved to where their things are on the screen now, when
    /// output came or the view scrolled. A thing that was not labelled
    /// gets none; one that went loses its label.
    pub fn follow(&mut self, rows: &[Vec<CellView>]) {
        let mut hits = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for h in tsumugi_pane::hints(row) {
                if let Some(label) = self.hits.iter().find(|x| x.text == h.text).map(|x| x.label.clone()) {
                    hits.push(Hit { row: y, cells: h.cells, text: h.text, link: h.link, label });
                }
            }
        }
        self.hits = hits;
    }

    /// The hits whose label starts with what is typed.
    pub fn live(&self) -> impl Iterator<Item = &Hit> {
        self.hits.iter().filter(|h| h.label.starts_with(&self.typed))
    }

    pub fn press(&mut self, key: Key) -> Step {
        match key {
            Key::Exit => Step::Exit,
            Key::Back => {
                self.typed.pop();
                Step::Wait
            }
            Key::Letter(c, shift) => {
                let mut typed = self.typed.clone();
                typed.push(c.to_ascii_lowercase());
                // A letter no label goes on with is not taken.
                let Some(hit) = self.hits.iter().find(|h| h.label.starts_with(&typed)) else { return Step::Wait };
                self.typed = typed;
                self.open |= shift;
                if hit.label != self.typed {
                    return Step::Wait;
                }
                match (&hit.link, self.open) {
                    (Some(link), true) => Step::Open(link.clone()),
                    _ => Step::Copy(hit.text.clone()),
                }
            }
        }
    }
}

/// `n` labels, none the start of another: one letter each while they fit,
/// else two each.
fn labels(n: usize) -> Vec<String> {
    let letters: Vec<char> = LETTERS.chars().collect();
    if n <= letters.len() {
        return letters[..n].iter().map(|c| c.to_string()).collect();
    }
    letters.iter().flat_map(|a| letters.iter().map(move |b| format!("{a}{b}"))).take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(lines: &[&str]) -> Vec<Vec<CellView>> {
        let mut t = tsumugi_pane::testing::term(60, lines.len());
        tsumugi_pane::testing::feed(&mut t, &lines.join("\r\n"));
        tsumugi_pane::snapshot(&t)
    }

    #[test]
    fn the_newest_get_the_first_letters() {
        let q = QuickSelect::new(&screen(&["commit 5e9ef2a", "see https://x.dev/a and 5e9ef2a"]));
        let got: Vec<(usize, &str, &str)> = q.hits.iter().map(|h| (h.row, h.text.as_str(), h.label.as_str())).collect();
        assert_eq!(got, vec![(1, "5e9ef2a", "a"), (1, "https://x.dev/a", "s"), (0, "5e9ef2a", "a")], "the same hash, one label");
    }

    #[test]
    fn typing_a_label_copies_and_shift_opens() {
        let rows = screen(&["https://x.dev/a 12345"]);
        let mut q = QuickSelect::new(&rows);
        assert_eq!(q.press(Key::Letter('a', false)), Step::Copy("12345".into()));
        let mut q = QuickSelect::new(&rows);
        assert_eq!(q.press(Key::Letter('z', false)), Step::Wait, "no such label");
        assert_eq!(q.typed, "");
        assert_eq!(q.press(Key::Letter('S', true)), Step::Open(Link::Url("https://x.dev/a".into())));
        let mut q = QuickSelect::new(&rows);
        assert_eq!(q.press(Key::Letter('A', true)), Step::Copy("12345".into()), "a number has nothing to open");
    }

    #[test]
    fn the_labels_follow_the_output() {
        let mut q = QuickSelect::new(&screen(&["1234", "5678"]));
        q.follow(&screen(&["5678", "abcdef1", "1234"]));
        let got: Vec<(usize, &str, &str)> = q.hits.iter().map(|h| (h.row, h.text.as_str(), h.label.as_str())).collect();
        assert_eq!(got, vec![(0, "5678", "a"), (2, "1234", "s")], "the new hash gets no label");
    }

    #[test]
    fn many_take_two_letters() {
        assert_eq!(labels(3), ["a", "s", "d"]);
        let two = labels(30);
        assert_eq!((two[0].as_str(), two[26].as_str(), two.len()), ("aa", "sa", 30));
        let line = |from: usize| (from..from + 10).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");
        let mut q = QuickSelect::new(&screen(&[&line(1000), &line(1010), &line(1020)]));
        assert_eq!(q.press(Key::Letter('a', false)), Step::Wait);
        assert_eq!(q.live().count(), 26);
        assert_eq!(q.press(Key::Back), Step::Wait);
        assert_eq!(q.press(Key::Letter('s', false)), Step::Wait);
        assert_eq!(q.press(Key::Letter('a', false)), Step::Copy("1003".into()), "the 27th from the right");
        assert_eq!(q.press(Key::Exit), Step::Exit);
    }
}
