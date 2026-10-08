//! Copy mode: a cursor of its own over a pane's output, moved by the keys,
//! to select and copy without the mouse (tmux's and WezTerm's copy mode).
//! The pane gets no keys while it is on. Only the steps live here; the
//! window carries them out on the pane (`Pane::scroll`, `Pane::select`).

/// A key copy mode reads, from the arrows, `hjkl` and the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    /// The oldest line in the scrollback (`g`).
    Top,
    /// The newest line (`G`).
    Bottom,
    /// The start of the line (`0`, Home).
    LineStart,
    /// The end of the line (`$`, End).
    LineEnd,
    /// Start a selection at the cursor, or drop the one there is (`v`, Space).
    Mark,
    /// Copy the selection, or the cursor's line when there is none (`y`, Enter).
    Copy,
    /// Leave without copying (Esc, `q`).
    Exit,
}

/// What the window does to the pane for a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Lines through the scrollback: positive is older.
    Lines(i32),
    PageUp,
    PageDown,
    Top,
    Bottom,
    /// Begin a selection at the cell (`start`), or carry it on to it.
    Select { cell: (usize, usize), start: bool },
    Unselect,
    /// Put the selection on the clipboard.
    Copy,
    Exit,
}

/// Copy mode while it is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyMode {
    /// The cursor's column and line on the screen as it shows.
    pub cursor: (usize, usize),
    /// A selection runs from where `Mark` was pressed to the cursor.
    pub marking: bool,
}

impl CopyMode {
    /// Starting at the pane's own cursor.
    pub fn new(cursor: (usize, usize)) -> Self {
        Self { cursor, marking: false }
    }

    /// The steps for `key`, on a screen of `cols` × `rows` cells.
    pub fn press(&mut self, key: Key, cols: usize, rows: usize) -> Vec<Step> {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let (x, y) = (self.cursor.0.min(cols - 1), self.cursor.1.min(rows - 1));
        let mut out = Vec::new();
        let moved = match key {
            Key::Left => (x.saturating_sub(1), y),
            Key::Right => ((x + 1).min(cols - 1), y),
            Key::Up if y == 0 => {
                out.push(Step::Lines(1));
                (x, 0)
            }
            Key::Up => (x, y - 1),
            Key::Down if y + 1 >= rows => {
                out.push(Step::Lines(-1));
                (x, rows - 1)
            }
            Key::Down => (x, y + 1),
            Key::PageUp => {
                out.push(Step::PageUp);
                (x, y)
            }
            Key::PageDown => {
                out.push(Step::PageDown);
                (x, y)
            }
            Key::Top => {
                out.push(Step::Top);
                (0, 0)
            }
            Key::Bottom => {
                out.push(Step::Bottom);
                (0, rows - 1)
            }
            Key::LineStart => (0, y),
            Key::LineEnd => (cols - 1, y),
            Key::Mark => {
                self.marking = !self.marking;
                out.push(if self.marking { Step::Select { cell: (x, y), start: true } } else { Step::Unselect });
                return out;
            }
            Key::Copy => {
                // No selection: the cursor's whole line.
                if !self.marking {
                    out.push(Step::Select { cell: (0, y), start: true });
                    out.push(Step::Select { cell: (cols - 1, y), start: false });
                }
                out.extend([Step::Copy, Step::Exit]);
                return out;
            }
            Key::Exit => {
                out.extend([Step::Unselect, Step::Exit]);
                return out;
            }
        };
        self.cursor = moved;
        if self.marking {
            out.push(Step::Select { cell: moved, start: false });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_stays_on_the_screen_and_the_output_moves_under_it() {
        let mut m = CopyMode::new((2, 0));
        assert_eq!(m.press(Key::Up, 10, 5), vec![Step::Lines(1)]);
        assert_eq!(m.cursor, (2, 0));
        m.cursor = (9, 4);
        assert_eq!(m.press(Key::Right, 10, 5), vec![]);
        assert_eq!(m.cursor, (9, 4));
        assert_eq!(m.press(Key::Down, 10, 5), vec![Step::Lines(-1)]);
        assert_eq!(m.press(Key::LineStart, 10, 5), vec![]);
        assert_eq!(m.cursor, (0, 4));
        assert_eq!(m.press(Key::Top, 10, 5), vec![Step::Top]);
        assert_eq!(m.cursor, (0, 0));
    }

    #[test]
    fn a_mark_carries_the_selection_to_the_cursor_and_copy_ends_it() {
        let mut m = CopyMode::new((1, 1));
        assert_eq!(m.press(Key::Mark, 10, 5), vec![Step::Select { cell: (1, 1), start: true }]);
        assert_eq!(m.press(Key::Down, 10, 5), vec![Step::Select { cell: (1, 2), start: false }]);
        assert_eq!(m.press(Key::PageUp, 10, 5), vec![Step::PageUp, Step::Select { cell: (1, 2), start: false }]);
        assert_eq!(m.press(Key::Copy, 10, 5), vec![Step::Copy, Step::Exit]);
    }

    #[test]
    fn copy_with_no_mark_takes_the_line_and_a_second_mark_drops_it() {
        let mut m = CopyMode::new((3, 2));
        assert_eq!(m.press(Key::Copy, 10, 5), vec![Step::Select { cell: (0, 2), start: true }, Step::Select { cell: (9, 2), start: false }, Step::Copy, Step::Exit]);
        let mut m = CopyMode::new((3, 2));
        m.press(Key::Mark, 10, 5);
        assert_eq!(m.press(Key::Mark, 10, 5), vec![Step::Unselect]);
        assert_eq!(m.press(Key::Left, 10, 5), vec![]);
        assert_eq!(m.press(Key::Exit, 10, 5), vec![Step::Unselect, Step::Exit]);
    }
}
