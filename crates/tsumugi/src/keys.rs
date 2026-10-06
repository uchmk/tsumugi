//! The window's own keys (QUESTIONS.md Q1): Windows Terminal's on Windows and
//! Linux, iTerm2's (Cmd) on macOS. Everything else goes to the shell. None of
//! these is a key Claude Code uses (`Ctrl+C`, `Esc`, `Shift+Tab`, `Ctrl+R`).

use eframe::egui::{Key, Modifiers};
use tsumugi_layout::Toward;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    /// The N-th tab, counting from 0.
    Tab(usize),
    /// The next session waiting for a person, the longest-waiting first.
    NextWaiting,
    /// Split the pane with the keys: a new shell to its right, or below it.
    SplitRight,
    SplitDown,
    /// Give the keys to the pane on that side.
    Move(Toward),
    /// Show the pane with the keys alone, or all of them again.
    Zoom,
    /// The search box: sessions, folders and commands (the design's 1c).
    Search,
    /// The sidebar as a narrow rail, or back (the design's 1i A). Not the
    /// design's `Ctrl+B`: Claude Code sends a running command to the
    /// background with it, and it is tmux's prefix.
    Rail,
    /// The settings screen (the design's 1m).
    Settings,
    /// The input box below the panes (the design's 12, 1l). The Tab key
    /// stays the shell's; only the I key with Ctrl is taken.
    Input,
}

/// The action for a key press, if it is one of the window's: a key the
/// settings gave it (`[keys]`), else its own key unless the settings moved
/// that action elsewhere or took it away.
pub fn action(key: Key, m: Modifiers) -> Option<Action> {
    let bound = bindings();
    action_with(key, m, cfg!(target_os = "macos"), &bound)
}

fn action_with(key: Key, m: Modifiers, mac: bool, bound: &[(Action, Option<Chord>)]) -> Option<Action> {
    if let Some((a, _)) = bound.iter().find(|(_, c)| c.as_ref().is_some_and(|c| c.matches(key, m))) {
        return Some(*a);
    }
    let own = action_on(key, m, mac)?;
    (!bound.iter().any(|(a, _)| *a == own)).then_some(own)
}

/// The actions the settings can give another key, by the name `[keys]`
/// uses, with their own keys (elsewhere, then on macOS).
pub const NAMED: [(Action, &str, &str, &str); 12] = [
    (Action::NewTab, "new_tab", "Ctrl+Shift+T", "Cmd+T"),
    (Action::CloseTab, "close_tab", "Ctrl+Shift+W", "Cmd+W"),
    (Action::NextTab, "next_tab", "Ctrl+Tab", "Ctrl+Tab"),
    (Action::PrevTab, "prev_tab", "Ctrl+Shift+Tab", "Ctrl+Shift+Tab"),
    (Action::NextWaiting, "next_waiting", "Ctrl+Shift+U", "Cmd+Shift+U"),
    (Action::SplitRight, "split_right", "Alt+Shift+=", "Cmd+D"),
    (Action::SplitDown, "split_down", "Alt+Shift+-", "Cmd+Shift+D"),
    (Action::Zoom, "zoom", "Ctrl+Shift+Z", "Cmd+Shift+Z"),
    (Action::Search, "search", "Ctrl+Shift+P", "Cmd+Shift+P"),
    (Action::Rail, "rail", "Ctrl+Shift+B", "Cmd+Shift+B"),
    (Action::Settings, "settings", "Ctrl+,", "Cmd+,"),
    (Action::Input, "input", "Ctrl+I", "Cmd+I"),
];

/// What a changeable action is called on the settings screen and in
/// TESTING-KEYS.md.
pub fn title(a: Action) -> &'static str {
    match a {
        Action::NewTab => "New session",
        Action::CloseTab => "Close the session",
        Action::NextTab => "Next tab",
        Action::PrevTab => "Previous tab",
        Action::NextWaiting => "Go to the session waiting longest",
        Action::SplitRight => "Split right",
        Action::SplitDown => "Split down",
        Action::Zoom => "Zoom one pane",
        Action::Search => "Search",
        Action::Rail => "Narrow rail",
        Action::Settings => "Settings",
        Action::Input => "Input box",
        Action::Tab(_) => "The Nth tab",
        Action::Move(_) => "Move between panes",
    }
}

/// A key with the modifiers held, as `[keys]` writes it: `Ctrl+Shift+T`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// macOS's Cmd.
    pub cmd: bool,
}

impl Chord {
    /// `Ctrl+Shift+T`, `Alt+Shift+=`, `Cmd+,`, `F5`: modifiers (Ctrl, Shift,
    /// Alt or Opt, Cmd), then the key by its name or its character.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        // `Ctrl++` is Ctrl and the plus key.
        let (mods, key) = match parts.as_slice() {
            [rest @ .., "", ""] => (rest, "+"),
            [rest @ .., key] => (rest, *key),
            [] => return Err("an empty key".into()),
        };
        let mut c = Chord { key: Key::A, ctrl: false, shift: false, alt: false, cmd: false };
        for m in mods {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => c.ctrl = true,
                "shift" => c.shift = true,
                "alt" | "opt" | "option" => c.alt = true,
                "cmd" | "command" | "super" => c.cmd = true,
                other => return Err(format!("`{other}` is not Ctrl, Shift, Alt or Cmd")),
            }
        }
        c.key = key_named(key).ok_or_else(|| format!("`{key}` is not a key egui knows (A, F5, Tab, Comma or `,`, …)"))?;
        Ok(c)
    }

    fn matches(&self, key: Key, m: Modifiers) -> bool {
        // With Shift held, a US keyboard says `+` for the `=` key.
        let same_key = key == self.key || (self.key == Key::Equals && key == Key::Plus);
        same_key && m.ctrl == self.ctrl && m.shift == self.shift && m.alt == self.alt && m.mac_cmd == self.cmd
    }

    /// As `[keys]` writes it.
    pub fn label(&self) -> String {
        let mut out = String::new();
        for (on, name) in [(self.cmd, "Cmd"), (self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift")] {
            if on {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(&key_label(self.key));
        out
    }

    /// The key pressed, as a chord, when it is not a modifier alone.
    pub fn pressed(key: Key, m: Modifiers) -> Chord {
        Chord { key, ctrl: m.ctrl, shift: m.shift, alt: m.alt, cmd: m.mac_cmd }
    }
}

/// Ctrl, Shift, Alt or the Windows / Cmd key pressed alone.
pub fn is_modifier(key: Key) -> bool {
    matches!(key, Key::ShiftLeft | Key::ShiftRight | Key::ControlLeft | Key::ControlRight | Key::AltLeft | Key::AltRight | Key::SuperLeft | Key::SuperRight)
}

fn key_named(name: &str) -> Option<Key> {
    let symbol = match name {
        "," => Some(Key::Comma),
        "-" => Some(Key::Minus),
        "=" => Some(Key::Equals),
        "+" => Some(Key::Plus),
        "." => Some(Key::Period),
        "/" => Some(Key::Slash),
        ";" => Some(Key::Semicolon),
        "`" => Some(Key::Backtick),
        "[" => Some(Key::OpenBracket),
        "]" => Some(Key::CloseBracket),
        "\\" => Some(Key::Backslash),
        _ => None,
    };
    symbol.or_else(|| Key::from_name(name)).or_else(|| Key::from_name(&name.to_ascii_uppercase())).or_else(|| {
        let mut cs = name.chars();
        let first = cs.next()?.to_ascii_uppercase();
        Key::from_name(&format!("{first}{}", cs.as_str().to_ascii_lowercase()))
    })
}

fn key_label(key: Key) -> String {
    match key {
        Key::Comma => ",".into(),
        Key::Minus => "-".into(),
        Key::Equals => "=".into(),
        Key::Plus => "+".into(),
        Key::Period => ".".into(),
        Key::Slash => "/".into(),
        k => k.name().to_owned(),
    }
}

/// The keys the settings gave, or took away (`None`), by action.
static BINDINGS: std::sync::RwLock<Vec<(Action, Option<Chord>)>> = std::sync::RwLock::new(Vec::new());

fn bindings() -> Vec<(Action, Option<Chord>)> {
    BINDINGS.read().map(|b| b.clone()).unwrap_or_default()
}

/// Read `[keys]` from the settings: action name, then a key or `"none"`.
/// The keys in force change only when all of them read.
pub fn set_bindings(table: &std::collections::BTreeMap<String, String>) -> Result<(), String> {
    let mut out = Vec::new();
    for (name, text) in table {
        let Some((a, ..)) = NAMED.iter().find(|(_, n, ..)| n == name) else {
            let names: Vec<&str> = NAMED.iter().map(|(_, n, ..)| *n).collect();
            return Err(format!("keys.{name}: not one of {}", names.join(", ")));
        };
        let chord = if text.trim().eq_ignore_ascii_case("none") { None } else { Some(Chord::parse(text).map_err(|e| format!("keys.{name}: {e}"))?) };
        out.push((*a, chord));
    }
    if let Ok(mut b) = BINDINGS.write() {
        *b = out;
    }
    Ok(())
}

/// The key an action is on now, as shown beside it: the settings' key,
/// `none`, or its own.
pub fn label(action: Action) -> String {
    let mac = cfg!(target_os = "macos");
    if let Some((_, c)) = bindings().into_iter().find(|(a, _)| *a == action) {
        return c.map_or_else(|| "none".into(), |c| c.label());
    }
    NAMED.iter().find(|(a, ..)| *a == action).map(|(_, _, win, m)| if mac { *m } else { *win }).unwrap_or_default().to_owned()
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
    let arrow = || match key {
        Key::ArrowLeft => Some(Toward::Left),
        Key::ArrowRight => Some(Toward::Right),
        Key::ArrowUp => Some(Toward::Up),
        Key::ArrowDown => Some(Toward::Down),
        _ => None,
    };
    if mac {
        if cmd && m.alt && !m.shift {
            if let Some(t) = arrow() {
                return Some(Action::Move(t));
            }
        }
        return match key {
            Key::D if cmd && !m.shift => Some(Action::SplitRight),
            Key::D if cmd && m.shift => Some(Action::SplitDown),
            Key::Z if cmd && m.shift => Some(Action::Zoom),
            Key::T if cmd && !m.shift => Some(Action::NewTab),
            Key::W if cmd && !m.shift => Some(Action::CloseTab),
            Key::U if cmd && m.shift => Some(Action::NextWaiting),
            Key::P if cmd && m.shift => Some(Action::Search),
            Key::B if cmd && m.shift => Some(Action::Rail),
            Key::Comma if cmd && !m.shift => Some(Action::Settings),
            Key::I if cmd && !m.shift => Some(Action::Input),
            _ if cmd && !m.shift => digit().map(Action::Tab),
            _ => None,
        };
    }
    match key {
        Key::T if ctrl_shift => Some(Action::NewTab),
        Key::W if ctrl_shift => Some(Action::CloseTab),
        Key::U if ctrl_shift => Some(Action::NextWaiting),
        Key::Z if ctrl_shift => Some(Action::Zoom),
        Key::P if ctrl_shift => Some(Action::Search),
        Key::B if ctrl_shift => Some(Action::Rail),
        // Windows Terminal's and VS Code's.
        Key::Comma if m.ctrl && !m.shift && !m.alt => Some(Action::Settings),
        Key::I if m.ctrl && !m.shift && !m.alt => Some(Action::Input),
        // Alt+Shift+= / Alt+Shift+-, Windows Terminal's; with Shift held a US
        // keyboard reports `+` for the first.
        Key::Equals | Key::Plus if m.alt && m.shift && !m.ctrl => Some(Action::SplitRight),
        Key::Minus if m.alt && m.shift && !m.ctrl => Some(Action::SplitDown),
        _ if m.alt && !m.ctrl && !m.shift && arrow().is_some() => arrow().map(Action::Move),
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
        assert_eq!(action_on(Key::U, CTRL_SHIFT, false), Some(Action::NextWaiting));
        let alt_shift = Modifiers { alt: true, ctrl: false, shift: true, mac_cmd: false, command: false };
        let alt = Modifiers { alt: true, ..Default::default() };
        assert_eq!(action_on(Key::Plus, alt_shift, false), Some(Action::SplitRight));
        assert_eq!(action_on(Key::Minus, alt_shift, false), Some(Action::SplitDown));
        assert_eq!(action_on(Key::ArrowLeft, alt, false), Some(Action::Move(Toward::Left)));
        assert_eq!(action_on(Key::Z, CTRL_SHIFT, false), Some(Action::Zoom));
        assert_eq!(action_on(Key::P, CTRL_SHIFT, false), Some(Action::Search));
        assert_eq!(action_on(Key::P, CTRL, false), None, "Ctrl+P is the shell's history");
        assert_eq!(action_on(Key::B, CTRL_SHIFT, false), Some(Action::Rail));
        assert_eq!(action_on(Key::B, CTRL, false), None, "Ctrl+B is Claude Code's and tmux's");
        assert_eq!(action_on(Key::Comma, CTRL, false), Some(Action::Settings));
        assert_eq!(action_on(Key::Comma, CMD, true), Some(Action::Settings));
        assert_eq!(action_on(Key::I, CTRL, false), Some(Action::Input));
        assert_eq!(action_on(Key::Tab, Modifiers::NONE, false), None, "Tab is the shell's");
        assert_eq!(action_on(Key::Minus, alt, false), None, "Alt+- is the shell's");
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
    fn the_settings_move_a_key() {
        let new = Chord::parse("Ctrl+Shift+N").unwrap();
        let bound = vec![(Action::NewTab, Some(new)), (Action::Zoom, None)];
        assert_eq!(action_with(Key::N, CTRL_SHIFT, false, &bound), Some(Action::NewTab), "the key given");
        assert_eq!(action_with(Key::T, CTRL_SHIFT, false, &bound), None, "its own key is the shell's again");
        assert_eq!(action_with(Key::Z, CTRL_SHIFT, false, &bound), None, "taken away");
        assert_eq!(action_with(Key::W, CTRL_SHIFT, false, &bound), Some(Action::CloseTab), "the rest as they were");
        assert_eq!(Chord::parse("Ctrl+,").unwrap().key, Key::Comma);
        assert_eq!(Chord::parse("ctrl++").unwrap().key, Key::Plus);
        assert_eq!(Chord::parse("Alt+Shift+=").unwrap().label(), "Alt+Shift+=");
        assert_eq!(Chord::parse("F5").unwrap(), Chord { key: Key::F5, ctrl: false, shift: false, alt: false, cmd: false });
        assert_eq!(Chord::parse("cmd+shift+p").unwrap().label(), "Cmd+Shift+P");
        assert!(Chord::parse("Hyper+X").is_err());
        assert!(Chord::parse("Ctrl+Nothing").is_err());
        // Alt+Shift+= arrives as Plus from a US keyboard.
        let split = vec![(Action::SplitRight, Some(Chord::parse("Ctrl+Alt+=").unwrap()))];
        let ctrl_alt = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: true };
        assert_eq!(action_with(Key::Plus, ctrl_alt, false, &split), Some(Action::SplitRight));
        // Every named action's own key reads.
        for (_, _, win, mac) in NAMED {
            assert!(Chord::parse(win).is_ok() && Chord::parse(mac).is_ok(), "{win} {mac}");
        }
    }

    #[test]
    fn cmd_keys_on_mac() {
        assert_eq!(action_on(Key::T, CMD, true), Some(Action::NewTab));
        assert_eq!(action_on(Key::W, CMD, true), Some(Action::CloseTab));
        assert_eq!(action_on(Key::Num1, CMD, true), Some(Action::Tab(0)));
        assert_eq!(action_on(Key::D, CMD, true), Some(Action::SplitRight));
        let cmd_shift = Modifiers { shift: true, ..CMD };
        assert_eq!(action_on(Key::P, cmd_shift, true), Some(Action::Search));
        let cmd_opt = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: true, command: true };
        assert_eq!(action_on(Key::ArrowUp, cmd_opt, true), Some(Action::Move(Toward::Up)));
        let ctrl_mac = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: false };
        assert_eq!(action_on(Key::T, ctrl_mac, true), None, "Ctrl+T is the shell's on a Mac too");
    }
}
