//! The window's own keys (QUESTIONS.md Q1): Windows Terminal's on Windows and
//! Linux, iTerm2's (Cmd) on macOS. Everything else goes to the shell. None of
//! these is a key Claude Code uses (`Ctrl+C`, `Esc`, `Shift+Tab`, `Ctrl+R`).

use eframe::egui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    /// The N-th tab, counting from 0.
    Tab(usize),
}

/// The action for a key press, if it is one of the window's.
pub fn action(key: Key, m: Modifiers) -> Option<Action> {
    action_on(key, m, cfg!(target_os = "macos"))
}

fn action_on(key: Key, m: Modifiers, mac: bool) -> Option<Action> {
    // egui's `command` is Cmd on macOS and Ctrl elsewhere; `mac_cmd` only
    // ever holds on macOS.
    let cmd = if mac { m.mac_cmd } else { false };
    let ctrl_shift = m.ctrl && m.shift && !m.alt;
    if key == Key::Tab && m.ctrl && !m.alt {
        return Some(if m.shift { Action::PrevTab } else { Action::NextTab });
    }
    let digit = || match key {
        Key::Num1 => Some(0),
        Key::Num2 => Some(1),
        Key::Num3 => Some(2),
        Key::Num4 => Some(3),
        Key::Num5 => Some(4),
        Key::Num6 => Some(5),
        Key::Num7 => Some(6),
        Key::Num8 => Some(7),
        Key::Num9 => Some(8),
        _ => None,
    };
    if mac {
        return match key {
            Key::T if cmd && !m.shift => Some(Action::NewTab),
            Key::W if cmd && !m.shift => Some(Action::CloseTab),
            _ if cmd && !m.shift => digit().map(Action::Tab),
            _ => None,
        };
    }
    match key {
        Key::T if ctrl_shift => Some(Action::NewTab),
        Key::W if ctrl_shift => Some(Action::CloseTab),
        _ if m.ctrl && m.alt && !m.shift => digit().map(Action::Tab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL_SHIFT: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };
    const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
    const CTRL_ALT: Modifiers = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: true };
    const CMD: Modifiers = Modifiers { alt: false, ctrl: false, shift: false, mac_cmd: true, command: true };

    #[test]
    fn windows_terminal_keys_off_mac() {
        assert_eq!(action_on(Key::T, CTRL_SHIFT, false), Some(Action::NewTab));
        assert_eq!(action_on(Key::W, CTRL_SHIFT, false), Some(Action::CloseTab));
        assert_eq!(action_on(Key::Tab, CTRL, false), Some(Action::NextTab));
        assert_eq!(action_on(Key::Tab, CTRL_SHIFT, false), Some(Action::PrevTab));
        assert_eq!(action_on(Key::Num3, CTRL_ALT, false), Some(Action::Tab(2)));
    }

    /// The keys a shell or Claude Code needs stay theirs.
    #[test]
    fn the_shells_keys_are_left_alone() {
        assert_eq!(action_on(Key::T, CTRL, false), None, "Ctrl+T is the shell's");
        assert_eq!(action_on(Key::C, CTRL, false), None);
        assert_eq!(action_on(Key::W, CTRL, false), None, "readline's word rubout");
        assert_eq!(action_on(Key::Num3, CTRL, false), None);
        assert_eq!(action_on(Key::R, CTRL, false), None);
    }

    #[test]
    fn cmd_keys_on_mac() {
        assert_eq!(action_on(Key::T, CMD, true), Some(Action::NewTab));
        assert_eq!(action_on(Key::W, CMD, true), Some(Action::CloseTab));
        assert_eq!(action_on(Key::Num1, CMD, true), Some(Action::Tab(0)));
        let ctrl_mac = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: false };
        assert_eq!(action_on(Key::T, ctrl_mac, true), None, "Ctrl+T is the shell's on a Mac too");
    }
}
