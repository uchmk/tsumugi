//! A screen as the rows that changed since the last one sent, so that typing
//! a character sends one row and not the whole screen. A client that has
//! nothing yet, or a grid that changed shape, gets every row.

use serde::{Deserialize, Serialize};
use tsumugi_pane::{Block, CellView, Hyperlink, MouseReport, Placement, Screen};

/// What changed on a session's screen, and the state that goes with it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Update {
    /// Every row is here: start from nothing.
    pub full: bool,
    /// How many rows the screen has now.
    pub lines: u32,
    /// The rows that changed, by number from the top.
    pub rows: Vec<(u32, Vec<CellView>)>,
    pub cursor: (usize, usize),
    pub app_cursor: bool,
    pub alt_screen: bool,
    pub mouse: MouseReport,
    pub scrolled_back: usize,
    pub win32_input: bool,
    pub bracketed_paste: bool,
    pub title: String,
    /// The OSC 8 links on the screen, all of them.
    pub links: Vec<Hyperlink>,
    /// The commands on the screen that ended (OSC 133).
    pub blocks: Vec<Block>,
    /// The pictures on the screen; their pixels are asked for apart.
    pub pictures: Vec<Placement>,
}

/// What goes with the cells, and is compared whole.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extra {
    pub scrolled_back: usize,
    pub win32_input: bool,
    pub bracketed_paste: bool,
    pub title: String,
    pub links: Vec<Hyperlink>,
    pub blocks: Vec<Block>,
    pub pictures: Vec<Placement>,
}

/// The update from `old` (what the client has) to `new`; `None` when nothing
/// at all changed. With no `old`, or a grid of another shape, every row.
pub fn diff(old: Option<(&Screen, &Extra)>, new: &Screen, extra: &Extra) -> Option<Update> {
    let same_shape = |o: &Screen| o.rows.len() == new.rows.len() && o.rows.iter().zip(&new.rows).all(|(a, b)| a.len() == b.len());
    let (full, rows) = match old {
        Some((o, _)) if same_shape(o) => {
            let changed: Vec<(u32, Vec<CellView>)> =
                new.rows.iter().enumerate().filter(|(i, r)| o.rows[*i] != **r).map(|(i, r)| (i as u32, r.clone())).collect();
            (false, changed)
        }
        _ => (true, new.rows.iter().enumerate().map(|(i, r)| (i as u32, r.clone())).collect()),
    };
    if let (false, Some((o, e))) = (full, old) {
        let same = rows.is_empty()
            && (o.cursor, o.app_cursor, o.alt_screen, o.mouse) == (new.cursor, new.app_cursor, new.alt_screen, new.mouse)
            && e == extra;
        if same {
            return None;
        }
    }
    Some(Update {
        full,
        lines: new.rows.len() as u32,
        rows,
        cursor: new.cursor,
        app_cursor: new.app_cursor,
        alt_screen: new.alt_screen,
        mouse: new.mouse,
        scrolled_back: extra.scrolled_back,
        win32_input: extra.win32_input,
        bracketed_paste: extra.bracketed_paste,
        title: extra.title.clone(),
        links: extra.links.clone(),
        blocks: extra.blocks.clone(),
        pictures: extra.pictures.clone(),
    })
}

/// Bring `screen` and `extra` up to date with `u`.
pub fn apply(screen: &mut Screen, extra: &mut Extra, u: Update) {
    if u.full {
        screen.rows.clear();
    }
    screen.rows.resize(u.lines as usize, Vec::new());
    for (i, row) in u.rows {
        if let Some(slot) = screen.rows.get_mut(i as usize) {
            *slot = row;
        }
    }
    screen.cursor = u.cursor;
    screen.app_cursor = u.app_cursor;
    screen.alt_screen = u.alt_screen;
    screen.mouse = u.mouse;
    *extra = Extra { scrolled_back: u.scrolled_back, win32_input: u.win32_input, bracketed_paste: u.bracketed_paste, title: u.title, links: u.links, blocks: u.blocks, pictures: u.pictures };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(text: &[&str]) -> Screen {
        let mut t = tsumugi_pane::testing::term(20, text.len());
        let joined: Vec<String> = text.iter().map(|l| l.to_string()).collect();
        tsumugi_pane::testing::feed(&mut t, &joined.join("\r\n"));
        Screen { rows: tsumugi_pane::snapshot(&t), cursor: tsumugi_pane::cursor_cell(&t), ..Default::default() }
    }

    #[test]
    fn only_the_changed_row_goes() {
        let a = screen(&["one", "two", "three"]);
        let b = screen(&["one", "TWO", "three"]);
        let e = Extra::default();
        let u = diff(Some((&a, &e)), &b, &e).expect("a row changed");
        assert!(!u.full);
        assert_eq!(u.rows.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![1]);
        let (mut have, mut extra) = (a.clone(), Extra::default());
        apply(&mut have, &mut extra, u);
        assert_eq!(have.rows, b.rows, "the client ends up with the new screen");
    }

    #[test]
    fn nothing_changed_sends_nothing() {
        let a = screen(&["one", "two"]);
        let e = Extra::default();
        assert_eq!(diff(Some((&a, &e)), &a.clone(), &e), None);
        let moved = Extra { scrolled_back: 3, ..Default::default() };
        let u = diff(Some((&a, &e)), &a, &moved).expect("the view moved");
        assert!(u.rows.is_empty() && u.scrolled_back == 3);
    }

    #[test]
    fn a_new_client_or_a_new_shape_gets_everything() {
        let a = screen(&["one", "two"]);
        let e = Extra::default();
        let u = diff(None, &a, &e).unwrap();
        assert!(u.full && u.rows.len() == 2);
        let taller = screen(&["one", "two", "three"]);
        let u = diff(Some((&a, &e)), &taller, &e).unwrap();
        assert!(u.full && u.rows.len() == 3);
        let (mut have, mut extra) = (a, Extra::default());
        apply(&mut have, &mut extra, u);
        assert_eq!(have.rows, taller.rows);
    }
}
