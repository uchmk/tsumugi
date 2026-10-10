//! A screen as the rows that changed since the last one sent, so that typing
//! a character sends one row and not the whole screen. A client that has
//! nothing yet, or a grid that changed shape, gets every row.

use serde::{Deserialize, Serialize};
use ito_pane::{Block, CellView, Hyperlink, MouseReport, Placement, Screen};

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
    pub focus_report: bool,
    /// The kitty keyboard enhancements the program asked for.
    pub kitty: u8,
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
    /// Something is selected, on the screen or scrolled off it.
    pub selected: bool,
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
    pub selected: bool,
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
            && (o.cursor, o.app_cursor, o.alt_screen, o.mouse, o.focus_report, o.kitty) == (new.cursor, new.app_cursor, new.alt_screen, new.mouse, new.focus_report, new.kitty)
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
        focus_report: new.focus_report,
        kitty: new.kitty,
        scrolled_back: extra.scrolled_back,
        win32_input: extra.win32_input,
        bracketed_paste: extra.bracketed_paste,
        title: extra.title.clone(),
        links: extra.links.clone(),
        blocks: extra.blocks.clone(),
        pictures: extra.pictures.clone(),
        selected: extra.selected,
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
    screen.focus_report = u.focus_report;
    screen.kitty = u.kitty;
    *extra = Extra { scrolled_back: u.scrolled_back, win32_input: u.win32_input, bracketed_paste: u.bracketed_paste, title: u.title, links: u.links, blocks: u.blocks, pictures: u.pictures, selected: u.selected };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(text: &[&str]) -> Screen {
        let mut t = ito_pane::testing::term(20, text.len());
        let joined: Vec<String> = text.iter().map(|l| l.to_string()).collect();
        ito_pane::testing::feed(&mut t, &joined.join("\r\n"));
        Screen { rows: ito_pane::snapshot(&t), cursor: ito_pane::cursor_cell(&t), ..Default::default() }
    }

    /// The styles go over the wire as they are: the flags and the
    /// underline's colour, and none left over after `SGR 0`.
    #[test]
    fn the_styles_come_back_from_the_wire() {
        let row = "a \x1b[4mu\x1b[0m \x1b[4:2mdd\x1b[0m \x1b[4:3;58;2;255;0;0mcurl\x1b[0m \x1b[4:4mdots\x1b[0m \x1b[4:5mdash\x1b[0m \x1b[9mstrike\x1b[0m [\x1b[8mhid\x1b[0m]";
        let mut t = ito_pane::testing::term(60, 2);
        ito_pane::testing::feed(&mut t, row);
        let sent = Screen { rows: ito_pane::snapshot(&t), ..Default::default() };
        let u = diff(None, &sent, &Extra::default()).unwrap();
        let mut buf = Vec::new();
        crate::frame::write(&mut buf, &u).unwrap();
        let back: Update = crate::frame::read(&mut &buf[..]).unwrap();
        assert_eq!(back, u);
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

    /// A selection scrolled off the screen shows in no cell, so the window
    /// hears of it on its own (Ctrl+Shift+C copies it, not Ctrl+C).
    #[test]
    fn a_selection_off_the_screen_still_goes() {
        let a = screen(&["one", "two"]);
        let e = Extra::default();
        let selected = Extra { selected: true, ..Default::default() };
        let u = diff(Some((&a, &e)), &a, &selected).expect("the selection changed");
        assert!(u.rows.is_empty());
        let (mut have, mut extra) = (a.clone(), Extra::default());
        apply(&mut have, &mut extra, u);
        assert!(extra.selected);
    }

    /// DECSET 1004 alone, with no cell changed, still reaches the client:
    /// the window decides from it whether to tell the program of focus.
    #[test]
    fn a_request_for_focus_reports_goes_on_its_own() {
        let a = screen(&["one"]);
        let b = Screen { focus_report: true, ..a.clone() };
        let e = Extra::default();
        let u = diff(Some((&a, &e)), &b, &e).expect("the mode changed");
        let (mut have, mut extra) = (a.clone(), Extra::default());
        apply(&mut have, &mut extra, u);
        assert!(have.focus_report);
    }

    /// A program pushing kitty's keyboard flags changes no cell, and the
    /// client still has to hear of it to send Shift+Enter its new way.
    #[test]
    fn kitty_keyboard_flags_go_on_their_own() {
        let a = screen(&["one"]);
        let b = Screen { kitty: 0b11, ..a.clone() };
        let e = Extra::default();
        let u = diff(Some((&a, &e)), &b, &e).expect("the flags changed");
        let (mut have, mut extra) = (a.clone(), Extra::default());
        apply(&mut have, &mut extra, u);
        assert_eq!(have.kitty, 0b11);
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
