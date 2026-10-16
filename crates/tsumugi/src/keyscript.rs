//! `tsumugi --keys "<C-S-t><Wait:500>echo<Space>hi<Enter><Shot:tab>"`: keys
//! pressed by tsumugi itself once the window is up, so a test can drive the
//! window without reaching into it from outside.
//!
//! The script's words, the events they become and the player are ito's
//! `ito-keyscript`, shared with kura and yagura (ito's docs/common-spec.md,
//! "キーの台本"). What is tsumugi's own stays here: how long the window gets
//! before the first key, and the one-line form of a state file's value.
//! `TSUMUGI_KEYS_DONE` (a path) gets the state a check reads and how the run
//! ended (the `keys:` line: `done`, or `quit`, `closed`, `stalled`,
//! `panicked`, `refused`, with the lines after it saying more);
//! `<Shot:name>` and `<State:name>` write beside it.

use std::path::PathBuf;
use std::time::Duration;

pub use ito_keyscript::*;

/// On top of the player's own wait: the window's first frames lay it out
/// and reach the server, and a key before them lands before there is a pane
/// to take it.
const SERVER: Duration = Duration::from_millis(1000);

/// The player for `steps`, after the wait for the server.
pub fn runner(mut steps: Vec<Step>, done: Option<PathBuf>) -> Runner {
    steps.insert(0, Step::Wait(SERVER));
    Runner::new(steps, done)
}

/// The state file's value: one line, its line breaks written `\n`.
pub fn one_line(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\r', "").replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The common crate's tests cover the script itself; this is what needs
    /// tsumugi: the window's own key, as keys.rs reads it.
    #[test]
    fn keys_become_the_events_the_window_reads() {
        let ev = events(&ito_keys::Key::parse("<C-S-t>").unwrap()).unwrap();
        let egui::Event::Key { key, modifiers, .. } = ev[0] else { panic!("{ev:?}") };
        assert_eq!(crate::keys::action(key, modifiers), Some(crate::keys::Action::NewTab));
    }

    #[test]
    fn state_values_stay_on_one_line() {
        assert_eq!(one_line("a\nb\r\n"), "a\\nb\\n");
    }
}
