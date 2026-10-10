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
    /// Move the divider beside the pane with the keys that way.
    Resize(Toward),
    /// Show the pane with the keys alone, or all of them again.
    Zoom,
    /// The command palette: commands, prompts, layouts and folders (the
    /// design's 1c). Sessions are on All sessions (`Overview`).
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
    /// Name the tab with the keys (the tab menu's Rename, the design's F2).
    Rename,
    /// Another session in the same folder, as the tab menu's Duplicate.
    Duplicate,
    /// The list of sessions waiting, to answer them together.
    Waiting,
    /// What is typed goes to every pane of the tab, or to the one with the
    /// keys again (iTerm2's Broadcast Input).
    TypeAll,
    /// The bell's list of notifications, open or closed.
    Notices,
    /// The panes' letters a point bigger or smaller, or as the settings
    /// have them again (Windows Terminal's keys). For the window, not saved.
    FontBigger,
    FontSmaller,
    FontReset,
    /// Every session on one screen: its state, tokens, time and last words.
    Overview,
    /// Move over the pane's output and its scrollback with the keys, select
    /// and copy (Windows Terminal's mark mode).
    CopyMode,
    /// The pane's view to the prompt above, or below (the shell's OSC 133
    /// marks; Windows Terminal's and iTerm2's keys).
    PrevPrompt,
    NextPrompt,
    /// Find in the pane's output and scrollback: a bar over the pane.
    Find,
    /// Every key and what it does, by kind, on one screen (filer's help).
    Help,
    /// The pane with the keys trades places with the next one in the tab
    /// (tmux's `swap-pane`), the keys going with it.
    SwapPane,
    /// Every divider of the tab where each pane gets the same room.
    Equalize,
    /// The pane with the keys out of its split into a tab of its own (the
    /// drop on the sidebar, with the keys).
    PaneToTab,
    /// Record the pane with the keys to an asciinema `.cast` file, or stop.
    Record,
    /// Write the pane with the keys to a text file in Downloads as it goes
    /// (a work log), or finish it.
    WorkLog,
    /// The last command's output on the clipboard (the shell's OSC 133
    /// marks; Warp's and iTerm2's).
    CopyOutput,
    /// Letters on the screen's links, hashes and numbers: one typed copies
    /// it, with Shift opens it (WezTerm's Quick Select, kitty's hints).
    QuickSelect,
}

/// The action for a key press, if it is one of the window's: a key the
/// settings gave it (`[keys]`), else its own key unless the settings moved
/// that action elsewhere or took it away.
pub fn action(key: Key, m: Modifiers) -> Option<Action> {
    let bound = bindings();
    action_with(key, m, mac(), &bound)
}

/// `[general] cmd_on_mac`: off, a Mac takes the keys of Windows and Linux.
static CMD_ON_MAC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_cmd_on_mac(on: bool) {
    CMD_ON_MAC.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The window's keys are the Mac's (Cmd) ones.
pub fn mac() -> bool {
    cfg!(target_os = "macos") && CMD_ON_MAC.load(std::sync::atomic::Ordering::Relaxed)
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
pub const NAMED: [(Action, &str, &str, &str); 33] = [
    (Action::NewTab, "new_tab", "Ctrl+Shift+T", "Cmd+T"),
    (Action::CloseTab, "close_tab", "Ctrl+Shift+W", "Cmd+W"),
    (Action::NextTab, "next_tab", "Ctrl+Tab", "Ctrl+Tab"),
    (Action::PrevTab, "prev_tab", "Ctrl+Shift+Tab", "Ctrl+Shift+Tab"),
    (Action::NextWaiting, "next_waiting", "Ctrl+Shift+U", "Cmd+Shift+U"),
    (Action::SplitRight, "split_right", "Alt+Shift++", "Cmd+D"),
    (Action::SplitDown, "split_down", "Alt+Shift+-", "Cmd+Shift+D"),
    (Action::Zoom, "zoom", "Ctrl+Shift+Z", "Cmd+Shift+Z"),
    (Action::Search, "search", "Ctrl+Shift+P", "Cmd+Shift+P"),
    (Action::Rail, "rail", "Ctrl+Shift+B", "Cmd+Shift+B"),
    (Action::Settings, "settings", "Ctrl+,", "Cmd+,"),
    (Action::Input, "input", "Ctrl+I", "Cmd+I"),
    // The design's menu keys. On a Mac Cmd+Shift+D splits down already.
    (Action::Rename, "rename", "F2", "F2"),
    (Action::Duplicate, "duplicate", "Ctrl+Shift+D", "Cmd+Option+D"),
    // Y for yes: no shell's or Claude Code's key.
    (Action::Waiting, "waiting_list", "Ctrl+Shift+Y", "Cmd+Shift+Y"),
    // iTerm2's own key for it on a Mac.
    (Action::TypeAll, "type_into_all", "Ctrl+Shift+I", "Cmd+Shift+I"),
    // N for notifications (Q13).
    (Action::Notices, "notifications", "Ctrl+Shift+N", "Cmd+Shift+N"),
    // Windows Terminal's and every browser's. Ctrl+- is readline's undo
    // (Ctrl+_), which Ctrl+Shift+- still sends.
    (Action::FontBigger, "font_bigger", "Ctrl+=", "Cmd+="),
    (Action::FontSmaller, "font_smaller", "Ctrl+-", "Cmd+-"),
    (Action::FontReset, "font_reset", "Ctrl+0", "Cmd+0"),
    // O for overview.
    (Action::Overview, "overview", "Ctrl+Shift+O", "Cmd+Shift+O"),
    // Windows Terminal's mark mode.
    (Action::CopyMode, "copy_mode", "Ctrl+Shift+M", "Cmd+Shift+M"),
    // Windows Terminal's scrollToMark and iTerm2's.
    (Action::PrevPrompt, "prev_prompt", "Ctrl+Shift+Up", "Cmd+Shift+Up"),
    (Action::NextPrompt, "next_prompt", "Ctrl+Shift+Down", "Cmd+Shift+Down"),
    // Windows Terminal's find; every Mac program's.
    (Action::Find, "find", "Ctrl+Shift+F", "Cmd+F"),
    // Every program's help key, and filer's.
    (Action::Help, "help", "F1", "F1"),
    // X for exchange, E for even, J for (a tab of its) own: no shell's or
    // Claude Code's keys, and not Ctrl+Shift+C / V.
    (Action::SwapPane, "swap_pane", "Ctrl+Shift+X", "Cmd+Shift+X"),
    (Action::Equalize, "equalize", "Ctrl+Shift+E", "Cmd+Shift+E"),
    (Action::PaneToTab, "pane_to_tab", "Ctrl+Shift+J", "Cmd+Shift+J"),
    // R for record. Ctrl+R alone stays the shell's history search.
    (Action::Record, "record", "Ctrl+Shift+R", "Cmd+Shift+R"),
    // S for save, as the menu's "Save the output to a file".
    (Action::WorkLog, "work_log", "Ctrl+Shift+S", "Cmd+Shift+S"),
    // L for last output.
    (Action::CopyOutput, "copy_output", "Ctrl+Shift+L", "Cmd+Shift+L"),
    // WezTerm's. Ctrl+Space alone stays the shell's (and an IME's).
    (Action::QuickSelect, "quick_select", "Ctrl+Shift+Space", "Cmd+Shift+Space"),
];

/// The keys the settings cannot move, the window's and its boxes': (section,
/// key on Windows and Linux, on macOS, what it does). The help (F1) lists
/// them, as TESTING-KEYS.md does.
pub const FIXED: &[(&str, &str, &str, &str)] = &[
    ("The window, fixed", "Ctrl+Alt+1 … 9", "Cmd+1 … 9", "The Nth tab"),
    ("The window, fixed", "Alt+Left", "Cmd+Option+Left", "The keys to the pane on the left"),
    ("The window, fixed", "Alt+Right", "Cmd+Option+Right", "The keys to the pane on the right"),
    ("The window, fixed", "Alt+Up", "Cmd+Option+Up", "The keys to the pane above"),
    ("The window, fixed", "Alt+Down", "Cmd+Option+Down", "The keys to the pane below"),
    ("The window, fixed", "Alt+Shift+Left", "Cmd+Ctrl+Left", "Move the divider nearest the pane with the keys to the left"),
    ("The window, fixed", "Alt+Shift+Right", "Cmd+Ctrl+Right", "Move the divider nearest the pane with the keys to the right"),
    ("The window, fixed", "Alt+Shift+Up", "Cmd+Ctrl+Up", "Move the divider nearest the pane with the keys up"),
    ("The window, fixed", "Alt+Shift+Down", "Cmd+Ctrl+Down", "Move the divider nearest the pane with the keys down"),
    ("Panes", "Ctrl+Shift+C", "Cmd+C", "Copy the selection (none: Ctrl+C goes to the program)"),
    ("The new-session dialog", "Enter", "Enter", "Create in a new tab (on a button: press it)"),
    ("The new-session dialog", "Alt+Enter", "Option+Enter", "Create split to the right of the pane with the keys"),
    ("The new-session dialog", "Tab", "Tab", "In the folder: complete it from the list; complete, or elsewhere: the next field"),
    ("The new-session dialog", "Shift+Tab", "Shift+Tab", "The field before"),
    ("The new-session dialog", "Left / Right", "Left / Right", "On a way to start: the one beside it"),
    ("The new-session dialog", "Space", "Space", "Press the button that has the keys"),
    ("The new-session dialog", "Up / Down", "Up / Down", "Walk the folder list"),
    ("The new-session dialog", "Esc", "Esc", "Cancel"),
    ("The input box", "Ctrl+Enter", "Cmd+Enter", "Send the prompt"),
    ("The input box", "Ctrl+Shift+Enter", "Cmd+Shift+Enter", "Queue the prompt for when the session is done"),
    ("The input box", "Enter", "Enter", "A new line, not sent"),
    ("The input box", "Up / Down", "Up / Down", "The prompts sent before (in an empty box, or one showing a sent one)"),
    ("The input box", "Esc", "Esc", "Close the box, the keys back to the pane, the draft kept"),
    ("The command palette", "Up / Down", "Up / Down", "Walk the entries"),
    ("The command palette", "Enter", "Enter", "Do the entry picked"),
    ("The command palette", "Esc", "Esc", "Close it"),
    ("All sessions", "Letters", "Letters", "Narrow the sessions; 3 letters or more also list the lines of every scrollback that hold them"),
    ("All sessions", "Up / Down", "Up / Down", "Walk the sessions, then the scrollback lines"),
    ("All sessions", "Enter", "Enter", "Go to the session (a line: go there and show it)"),
    ("All sessions", "Left / Right", "Left / Right", "The page beside it, while nothing is typed"),
    ("All sessions", "Esc", "Esc", "Close it"),
    ("Copy mode", "Arrows / h j k l", "Arrows / h j k l", "Move the cursor; past the top or bottom, the output moves"),
    ("Copy mode", "PageUp / PageDown", "PageUp / PageDown", "A page through the scrollback"),
    ("Copy mode", "g / G", "g / G", "The oldest line / the newest"),
    ("Copy mode", "0 / $ (Home / End)", "0 / $ (Home / End)", "The start / the end of the line"),
    ("Copy mode", "v / Space", "v / Space", "Start a selection at the cursor, or drop it"),
    ("Copy mode", "y / Enter", "y / Enter", "Copy the selection (none: the cursor's line) and leave"),
    ("Copy mode", "Esc / q", "Esc / q", "Leave without copying"),
    ("Quick select", "a … z", "a … z", "Copy the thing with that label and leave"),
    ("Quick select", "Shift+a … z", "Shift+a … z", "Open the link with that label (a file in the editor, an address in the browser)"),
    ("Quick select", "Backspace", "Backspace", "Take back the letter typed"),
    ("Quick select", "Esc", "Esc", "Leave without copying"),
    ("The settings screen", "Esc", "Esc", "Leave the control that has the keys (a field as it was); with none, close the screen"),
    ("The settings screen", "Ctrl+Tab / Ctrl+PageDown", "Ctrl+Tab / Cmd+PageDown", "The next page"),
    ("The settings screen", "Ctrl+Shift+Tab / Ctrl+PageUp", "Ctrl+Shift+Tab / Cmd+PageUp", "The page before"),
    ("The settings screen", "Ctrl+F", "Cmd+F", "To the search"),
    ("The settings screen", "Tab / Shift+Tab", "Tab / Shift+Tab", "The search, the page's controls one by one, then Open settings.toml"),
    ("The settings screen", "Space / Enter", "Space / Enter", "Flip the switch or press the button that has the keys"),
    ("The settings screen", "Ctrl+,", "Cmd+,", "Open it (the changeable key above, while not moved)"),
    ("The help", "Esc / F1", "Esc / F1", "Close it"),
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
        Action::Search => "Command palette: commands, prompts, layouts, folders",
        Action::Rail => "Narrow rail",
        Action::Settings => "Settings",
        Action::Input => "Input box",
        Action::Rename => "Rename the tab",
        Action::Duplicate => "Duplicate in the same folder",
        Action::Waiting => "The waiting sessions, answered together",
        Action::TypeAll => "Type into every pane of the tab",
        Action::Notices => "Notifications (the bell)",
        Action::FontBigger => "Bigger letters",
        Action::FontSmaller => "Smaller letters",
        Action::FontReset => "Letters as the settings have them",
        Action::Overview => "All sessions: find a session or a line in any scrollback",
        Action::CopyMode => "Copy mode: select the output with the keys",
        Action::PrevPrompt => "To the prompt above",
        Action::NextPrompt => "To the prompt below",
        Action::Find => "Find in the pane",
        Action::Help => "Every key (this help)",
        Action::SwapPane => "Swap the pane with the next one",
        Action::Equalize => "Give every pane the same room",
        Action::PaneToTab => "The pane to a tab of its own",
        Action::Record => "Record the pane (asciinema .cast)",
        Action::WorkLog => "Write a work log of the pane (text in Downloads)",
        Action::CopyOutput => "Copy the last command's output",
        Action::QuickSelect => "Quick select: copy a link, hash or number by its letters",
        Action::Tab(_) => "The Nth tab",
        Action::Move(_) => "Move between panes",
        Action::Resize(_) => "Resize the pane",
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
        // With Shift held a US keyboard says `+` for the `=` key, so `=` and
        // `+` are one key to a chord (a JIS `=`, Shift and `-`, is made
        // `-` before it gets here: see `physical_minus`).
        let plus = |k: Key| matches!(k, Key::Equals | Key::Plus);
        let same_key = key == self.key || (plus(self.key) && plus(key));
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

/// Keys Claude Code and the shells use, which a window key would take from
/// them: the chord as `[keys]` writes it, and whose it is.
const OTHERS: [(&str, &str); 14] = [
    ("Ctrl+C", "Claude Code's interrupt"),
    ("Ctrl+D", "Claude Code's exit"),
    ("Ctrl+R", "Claude Code's history search"),
    ("Ctrl+O", "Claude Code's transcript"),
    ("Ctrl+T", "Claude Code's task list"),
    ("Ctrl+B", "Claude Code's background and tmux's prefix"),
    ("Ctrl+L", "the shell's clear"),
    ("Ctrl+Z", "the shell's suspend"),
    ("Ctrl+V", "Claude Code's paste"),
    ("Shift+Tab", "Claude Code's mode switch"),
    ("Escape", "Claude Code's stop"),
    ("Ctrl+W", "the shell's word rubout"),
    ("Ctrl+U", "the shell's line rubout"),
    ("Ctrl+A", "the shell's start of line"),
];

/// What `chord` would take, given to `action`: another of the window's
/// actions on it now, or a key of Claude Code's or the shell's.
pub fn clash(chord: &Chord, action: Action) -> Option<String> {
    let label = chord.label();
    for (a, ..) in NAMED {
        if a != action && self::label(a) == label {
            return Some(format!("{label} is {} already", title(a)));
        }
    }
    OTHERS.iter().find(|(k, _)| Chord::parse(k).is_ok_and(|c| c == *chord)).map(|(_, whose)| format!("{label} is {whose}"))
}

/// The key an action is on now, as shown beside it: the settings' key,
/// `none`, or its own.
pub fn label(action: Action) -> String {
    let mac = mac();
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
        // iTerm2's: Cmd+Ctrl+arrows.
        if cmd && m.ctrl && !m.alt && !m.shift {
            if let Some(t) = arrow() {
                return Some(Action::Resize(t));
            }
        }
        return match key {
            // Cmd+Option+D is Duplicate, below: not a split.
            Key::D if cmd && !m.shift && !m.alt => Some(Action::SplitRight),
            Key::D if cmd && m.shift => Some(Action::SplitDown),
            Key::Z if cmd && m.shift => Some(Action::Zoom),
            Key::T if cmd && !m.shift => Some(Action::NewTab),
            Key::W if cmd && !m.shift => Some(Action::CloseTab),
            Key::U if cmd && m.shift => Some(Action::NextWaiting),
            Key::P if cmd && m.shift => Some(Action::Search),
            Key::B if cmd && m.shift => Some(Action::Rail),
            Key::Comma if cmd && !m.shift => Some(Action::Settings),
            Key::I if cmd && !m.shift => Some(Action::Input),
            Key::D if cmd && m.alt && !m.shift => Some(Action::Duplicate),
            Key::Y if cmd && m.shift => Some(Action::Waiting),
            Key::I if cmd && m.shift => Some(Action::TypeAll),
            Key::N if cmd && m.shift => Some(Action::Notices),
            Key::O if cmd && m.shift => Some(Action::Overview),
            Key::M if cmd && m.shift => Some(Action::CopyMode),
            Key::ArrowUp if cmd && m.shift && !m.alt && !m.ctrl => Some(Action::PrevPrompt),
            Key::ArrowDown if cmd && m.shift && !m.alt && !m.ctrl => Some(Action::NextPrompt),
            Key::F if cmd && !m.shift && !m.alt && !m.ctrl => Some(Action::Find),
            Key::X if cmd && m.shift => Some(Action::SwapPane),
            Key::E if cmd && m.shift => Some(Action::Equalize),
            Key::J if cmd && m.shift => Some(Action::PaneToTab),
            Key::R if cmd && m.shift => Some(Action::Record),
            Key::S if cmd && m.shift => Some(Action::WorkLog),
            Key::L if cmd && m.shift => Some(Action::CopyOutput),
            Key::Space if cmd && m.shift && !m.alt && !m.ctrl => Some(Action::QuickSelect),
            Key::Equals | Key::Plus if cmd && !m.alt && !m.ctrl => Some(Action::FontBigger),
            Key::Minus if cmd && !m.shift && !m.alt && !m.ctrl => Some(Action::FontSmaller),
            Key::Num0 if cmd && !m.shift && !m.alt && !m.ctrl => Some(Action::FontReset),
            Key::F2 if !m.any() => Some(Action::Rename),
            Key::F1 if !m.any() => Some(Action::Help),
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
        Key::D if ctrl_shift => Some(Action::Duplicate),
        Key::Y if ctrl_shift => Some(Action::Waiting),
        Key::I if ctrl_shift => Some(Action::TypeAll),
        Key::N if ctrl_shift => Some(Action::Notices),
        Key::O if ctrl_shift => Some(Action::Overview),
        Key::M if ctrl_shift => Some(Action::CopyMode),
        Key::ArrowUp if ctrl_shift => Some(Action::PrevPrompt),
        Key::ArrowDown if ctrl_shift => Some(Action::NextPrompt),
        Key::F if ctrl_shift => Some(Action::Find),
        Key::X if ctrl_shift => Some(Action::SwapPane),
        Key::E if ctrl_shift => Some(Action::Equalize),
        Key::J if ctrl_shift => Some(Action::PaneToTab),
        Key::R if ctrl_shift => Some(Action::Record),
        Key::S if ctrl_shift => Some(Action::WorkLog),
        Key::L if ctrl_shift => Some(Action::CopyOutput),
        Key::Space if ctrl_shift => Some(Action::QuickSelect),
        // `+` is Shift and `=` on a US keyboard, Shift and `;` on a JIS one:
        // Shift may be held.
        Key::Equals | Key::Plus if m.ctrl && !m.alt => Some(Action::FontBigger),
        Key::Minus if m.ctrl && !m.shift && !m.alt => Some(Action::FontSmaller),
        Key::Num0 if m.ctrl && !m.shift && !m.alt => Some(Action::FontReset),
        Key::F2 if !m.any() => Some(Action::Rename),
        Key::F1 if !m.any() => Some(Action::Help),
        // Alt+Shift++ / Alt+Shift+-, Windows Terminal's: `+` is Shift and
        // `=` on a US keyboard, Shift and `;` on a JIS one.
        Key::Equals | Key::Plus if m.alt && m.shift && !m.ctrl => Some(Action::SplitRight),
        Key::Minus if m.alt && m.shift && !m.ctrl => Some(Action::SplitDown),
        _ if m.alt && !m.ctrl && !m.shift && arrow().is_some() => arrow().map(Action::Move),
        _ if m.alt && m.shift && !m.ctrl && arrow().is_some() => arrow().map(Action::Resize),
        _ if m.ctrl && m.alt && !m.shift => digit().map(Action::Tab),
        _ => None,
    }
}

/// On a JIS keyboard `=` is Shift and the `-` key, so Alt+Shift+- arrived
/// as Alt+Shift+= and split right; nothing could split down. A key event
/// from the `-` key with Shift held is `-` here, whatever the layout calls
/// it, before anything reads the keys.
pub fn physical_minus(events: &mut [egui::Event]) {
    for e in events {
        if let egui::Event::Key { key, physical_key: Some(Key::Minus), modifiers, .. } = e {
            if modifiers.shift && *key == Key::Equals {
                *key = Key::Minus;
            }
        }
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
        assert_eq!(action_on(Key::ArrowRight, alt_shift, false), Some(Action::Resize(Toward::Right)));
        assert_eq!(action_on(Key::Z, CTRL_SHIFT, false), Some(Action::Zoom));
        assert_eq!(action_on(Key::P, CTRL_SHIFT, false), Some(Action::Search));
        assert_eq!(action_on(Key::P, CTRL, false), None, "Ctrl+P is the shell's history");
        assert_eq!(action_on(Key::B, CTRL_SHIFT, false), Some(Action::Rail));
        assert_eq!(action_on(Key::B, CTRL, false), None, "Ctrl+B is Claude Code's and tmux's");
        assert_eq!(action_on(Key::Comma, CTRL, false), Some(Action::Settings));
        assert_eq!(action_on(Key::Comma, CMD, true), Some(Action::Settings));
        assert_eq!(action_on(Key::I, CTRL, false), Some(Action::Input));
        assert_eq!(action_on(Key::Y, CTRL_SHIFT, false), Some(Action::Waiting));
        assert_eq!(action_on(Key::I, CTRL_SHIFT, false), Some(Action::TypeAll));
        assert_eq!(action_on(Key::N, CTRL_SHIFT, false), Some(Action::Notices));
        assert_eq!(action_on(Key::Y, CTRL, false), None, "Ctrl+Y is the shell's yank");
        assert_eq!(action_on(Key::Tab, Modifiers::NONE, false), None, "Tab is the shell's");
        assert_eq!(action_on(Key::Minus, alt, false), None, "Alt+- is the shell's");
        assert_eq!(action_on(Key::Equals, CTRL, false), Some(Action::FontBigger));
        assert_eq!(action_on(Key::Plus, CTRL_SHIFT, false), Some(Action::FontBigger), "Ctrl and + on a US or JIS keyboard");
        assert_eq!(action_on(Key::Minus, CTRL, false), Some(Action::FontSmaller));
        assert_eq!(action_on(Key::Minus, CTRL_SHIFT, false), None, "Ctrl+_ is readline's undo");
        assert_eq!(action_on(Key::Num0, CTRL, false), Some(Action::FontReset));
        assert_eq!(action_on(Key::O, CTRL_SHIFT, false), Some(Action::Overview));
        assert_eq!(action_on(Key::O, CTRL, false), None, "Ctrl+O is Claude Code's transcript");
        assert_eq!(action_on(Key::M, CTRL_SHIFT, false), Some(Action::CopyMode));
        assert_eq!(action_on(Key::ArrowUp, CTRL_SHIFT, false), Some(Action::PrevPrompt));
        assert_eq!(action_on(Key::ArrowDown, CTRL_SHIFT, false), Some(Action::NextPrompt));
        assert_eq!(action_on(Key::F, CTRL_SHIFT, false), Some(Action::Find));
        assert_eq!(action_on(Key::X, CTRL_SHIFT, false), Some(Action::SwapPane));
        assert_eq!(action_on(Key::E, CTRL_SHIFT, false), Some(Action::Equalize));
        assert_eq!(action_on(Key::J, CTRL_SHIFT, false), Some(Action::PaneToTab));
    }

    /// The keys a shell or Claude Code needs stay theirs.
    #[test]
    fn the_shells_keys_are_left_alone() {
        assert_eq!(action_on(Key::T, CTRL, false), None, "Ctrl+T is the shell's");
        assert_eq!(action_on(Key::C, CTRL, false), None);
        assert_eq!(action_on(Key::W, CTRL, false), None, "readline's word rubout");
        assert_eq!(action_on(Key::Num3, CTRL, false), None);
        assert_eq!(action_on(Key::R, CTRL, false), None, "readline's history search");
        assert_eq!(action_on(Key::R, CTRL_SHIFT, false), Some(Action::Record));
        assert_eq!(action_on(Key::S, CTRL, false), None, "Ctrl+S is the terminal's stop (XOFF) and some editors' save");
        assert_eq!(action_on(Key::S, CTRL_SHIFT, false), Some(Action::WorkLog));
        assert_eq!(action_on(Key::S, Modifiers { shift: true, command: true, mac_cmd: true, ..Default::default() }, true), Some(Action::WorkLog));
        assert_eq!(action_on(Key::L, CTRL, false), None, "the shell's clear screen");
        assert_eq!(action_on(Key::L, CTRL_SHIFT, false), Some(Action::CopyOutput));
        assert_eq!(action_on(Key::Space, CTRL, false), None, "Ctrl+Space is the shell's (and an IME's)");
        assert_eq!(action_on(Key::Space, CTRL_SHIFT, false), Some(Action::QuickSelect));
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
        assert_eq!(Chord::parse("Alt+Shift++").unwrap().label(), "Alt+Shift++");
        assert_eq!(Chord::parse("F5").unwrap(), Chord { key: Key::F5, ctrl: false, shift: false, alt: false, cmd: false });
        assert_eq!(Chord::parse("cmd+shift+p").unwrap().label(), "Cmd+Shift+P");
        assert!(Chord::parse("Hyper+X").is_err());
        assert!(Chord::parse("Ctrl+Nothing").is_err());
        // Alt+Shift+= arrives as Plus from a US keyboard.
        let split = vec![(Action::SplitRight, Some(Chord::parse("Ctrl+Alt+=").unwrap()))];
        let ctrl_alt = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: true };
        assert_eq!(action_with(Key::Plus, ctrl_alt, false, &split), Some(Action::SplitRight));
        // A clash is named: with another action, or with Claude Code.
        assert_eq!(clash(&Chord::parse("Ctrl+Shift+W").unwrap(), Action::NewTab).as_deref(), Some("Ctrl+Shift+W is Close the session already"));
        assert!(clash(&Chord::parse("Ctrl+C").unwrap(), Action::NewTab).unwrap().contains("Claude Code"));
        assert_eq!(clash(&Chord::parse("Ctrl+Shift+T").unwrap(), Action::NewTab), None, "its own key");
        assert_eq!(clash(&Chord::parse("Ctrl+Shift+K").unwrap(), Action::NewTab), None);
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
        // Cmd+Option+D duplicates; it was taken for Cmd+D's split before
        // (the source review, 2026-10-07).
        assert_eq!(action_on(Key::D, Modifiers { alt: true, ..CMD }, true), Some(Action::Duplicate));
        let cmd_shift = Modifiers { shift: true, ..CMD };
        assert_eq!(action_on(Key::P, cmd_shift, true), Some(Action::Search));
        let cmd_opt = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: true, command: true };
        assert_eq!(action_on(Key::ArrowUp, cmd_opt, true), Some(Action::Move(Toward::Up)));
        let ctrl_mac = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: false };
        assert_eq!(action_on(Key::T, ctrl_mac, true), None, "Ctrl+T is the shell's on a Mac too");
        assert_eq!(action_on(Key::Equals, CMD, true), Some(Action::FontBigger));
        assert_eq!(action_on(Key::Minus, CMD, true), Some(Action::FontSmaller));
        assert_eq!(action_on(Key::Num0, CMD, true), Some(Action::FontReset));
        assert_eq!(action_on(Key::O, cmd_shift, true), Some(Action::Overview));
        assert_eq!(action_on(Key::M, cmd_shift, true), Some(Action::CopyMode));
        assert_eq!(action_on(Key::Space, cmd_shift, true), Some(Action::QuickSelect));
        assert_eq!(action_on(Key::ArrowUp, cmd_shift, true), Some(Action::PrevPrompt));
        assert_eq!(action_on(Key::F, CMD, true), Some(Action::Find));
        assert_eq!(action_on(Key::X, Modifiers { shift: true, ..CMD }, true), Some(Action::SwapPane));
    }

    #[test]
    fn a_jis_keyboard_splits_both_ways() {
        let alt_shift = Modifiers { alt: true, shift: true, ..Default::default() };
        let ev = |key, physical| egui::Event::Key { key, physical_key: Some(physical), pressed: true, repeat: false, modifiers: alt_shift };
        // JIS: Shift and `-` is `=`; Shift and `;` is `+`.
        let mut events = vec![ev(Key::Equals, Key::Minus), ev(Key::Plus, Key::Semicolon), ev(Key::Plus, Key::Equals)];
        physical_minus(&mut events);
        let keys: Vec<Key> = events.iter().map(|e| match e {
            egui::Event::Key { key, .. } => *key,
            _ => unreachable!(),
        }).collect();
        assert_eq!(keys, [Key::Minus, Key::Plus, Key::Plus], "only JIS's `-` key changes");
        assert_eq!(action_on(Key::Minus, alt_shift, false), Some(Action::SplitDown));
        assert_eq!(action_on(Key::Plus, alt_shift, false), Some(Action::SplitRight));
        // A key written as `Alt+Shift+=` in the settings is the same key.
        let split = [(Action::SplitRight, Some(Chord::parse("Alt+Shift+=").unwrap()))];
        assert_eq!(action_with(Key::Plus, alt_shift, false, &split), Some(Action::SplitRight));
        let split = [(Action::SplitRight, Some(Chord::parse("Alt+Shift++").unwrap()))];
        assert_eq!(action_with(Key::Equals, alt_shift, false, &split), Some(Action::SplitRight));
    }
}
