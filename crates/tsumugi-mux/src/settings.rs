//! The settings file, `settings.toml`: what a person writes to change how
//! tsumugi behaves (QUESTIONS.md Q6). The server reads the tag rules from it,
//! the window the ways to tell; both read it again when it changes.
//!
//! ```toml
//! # A theme's name, or "dark", "light" or "system" (following the OS
//! # between dark_theme and light_theme). The design's 1k.
//! theme = "dark"
//! dark_theme = "tsumugi Dark"
//! light_theme = "tsumugi Light"
//!
//! # The clock in the status bar (the design's 1n). date_format is one of
//! # YYYY/MM/DD, YYYY-MM-DD, MM/DD/YYYY, DD/MM/YYYY.
//! [clock]
//! show = true
//! hour24 = true
//! date = true
//! date_format = "YYYY/MM/DD"
//! weekday = true
//!
//! # How much a pane without the keys is dimmed, in percent (1e), and
//! # whether waiting rings breathe and running tabs show a moving line.
//! [appearance]
//! dim = 35
//! animations = true
//!
//! # The window's frame: "tsumugi" draws its own title bar along the band
//! # (Windows and Linux; on macOS the band runs under the traffic lights), or
//! # "system". material is "none", or "mica" or "acrylic" (Windows 11) or
//! # "vibrancy" (macOS; the other two are it there too): the desktop shows
//! # through the band, sidebar and status bar (on restart). opacity (20 to
//! # 100 %) lets the desktop show through all of it; image is a picture
//! # behind the panes, image_opacity how much of it shows (%).
//! [window]
//! titlebar = "tsumugi"
//! material = "none"
//! opacity = 100
//! image = ""
//! image_opacity = 25
//!
//! # The window's keys, moved: an action's name and a key, or "none" to give
//! # the key back to the shell. The names: new_tab, close_tab, next_tab,
//! # prev_tab, next_waiting, split_right, split_down, zoom, search, rail,
//! # settings, input.
//! [keys]
//! new_tab = "Ctrl+Shift+N"
//!
//! # The panes' font: a font file's name (or part of it, "JetBrainsMono")
//! # or its path; "" for the Nerd Font found, else the built-in one. Its Bold
//! # and Italic files beside it are used for bold and italic text.
//! [font]
//! family = ""
//! size = 14
//! line_height = 1.0
//! ligatures = true
//!
//! # Tag a session by the folder it is in (the design's 1a).
//! [[tags.rule]]
//! folder = "~/dev/filer"
//! tag = "filer"
//!
//! # Every folder under ~/dev, by its own name.
//! [[tags.rule]]
//! folder = "~/dev/*"
//! tag = "{name}"
//!
//! # Which states tell in which way (the design's 1h).
//! [notify]
//! system = ["waiting", "error", "done"]
//! taskbar = ["waiting", "error"]
//! flash = []
//! sound = []
//! # A session waiting two minutes while no one is at the window, sent here.
//! webhook = "https://ntfy.sh/my-topic"
//! webhook_format = "ntfy"
//! webhook_after = 120
//! # Say so once when the day's estimated cost passes $20, or a 5-hour block's $10.
//! spend_day = 20
//! spend_block = 10
//!
//! # Other AI programs: what a restored session types to take one up again.
//! [agents.gemini]
//! resume = "gemini --resume latest"
//!
//! # What tokens cost, per million in US dollars, by the start of the model's id.
//! [prices."claude-opus-5-5"]
//! input = 4.0
//! output = 20.0
//! cache_read = 0.2
//!
//! # What a tab's menu runs to open its folder; {folder} is the folder.
//! # A list for more than one: the first on a click, all in a menu beside it.
//! # `file`: what Ctrl+click on a path in a pane runs ({file}, {line},
//! # {column}); empty, the system opens it.
//! [open]
//! editor = ["code {folder}", "sakura {folder}"]
//! filer = "filer {folder}"
//! file = "code --goto {file}:{line}:{column}"
//!
//! # The tab's menu (the design's 1j): items left out, and more of your own.
//! [menu]
//! hide = ["new-window"]
//!
//! [[menu.session]]
//! name = "Open lazygit here"
//! command = "wt -d {folder} lazygit"
//!
//! # How many of a session's tags the band and the cards show, then +N.
//! [tags]
//! shown = 3
//!
//! # A session on a branch like this gets the tag (with a folder too, both
//! # must match); a tag's own colour.
//! [[tags.rule]]
//! branch = "claude/*"
//! tag = "claude"
//!
//! [tags.colors]
//! claude = "#5e4a86"
//!
//! # Startup and closing (the settings screen's General).
//! [general]
//! default_folder = "~/dev"
//! keep_sessions = true
//! ask_before_close = true
//! check_updates = true
//! restart_after_update = false
//! cmd_on_mac = true
//! # Ask before a paste of several lines that the program would run line
//! # by line (it did not ask for bracketed paste), and before a large one.
//! warn_multiline_paste = true
//! warn_large_paste = true
//!
//! # What a new session runs, and how waiting is told.
//! [sessions]
//! start = "claude"
//! claude = "claude"
//! resume = true
//! quiet = 10
//!
//! # The shell sessions start, its arguments and its variables.
//! [shell]
//! program = "pwsh"
//! args = ["-NoLogo"]
//!
//! [shell.env]
//! EDITOR = "code --wait"
//!
//! [advanced]
//! backend = "auto"
//! scrollback = 10000
//! pane_log = false
//! ```

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub theme: String,
    pub dark_theme: String,
    pub light_theme: String,
    pub general: General,
    pub sessions: Sessions,
    pub shell: Shell,
    pub advanced: Advanced,
    pub clock: Clock,
    pub appearance: Appearance,
    pub font: Font,
    pub window: Window,
    /// The window's keys moved: an action's name, then a key (`Ctrl+Shift+N`)
    /// or `none`. Read by the window, which knows the keys.
    pub keys: std::collections::BTreeMap<String, String>,
    pub tags: Tags,
    pub notify: Notify,
    pub open: Open,
    pub menu: Menu,
    /// Prices per million tokens in US dollars, by the start of a model's id
    /// (`claude-opus-5-5`): the window's own are used for the rest.
    pub prices: std::collections::BTreeMap<String, Price>,
    /// AI programs other than Claude Code, by the name of their program
    /// (`codex`): told apart from a plain shell, and what a restored session
    /// types to take up its conversation again. Those in [`AGENTS`] are
    /// known without being named here.
    pub agents: std::collections::BTreeMap<String, Agent>,
}

/// The AI programs a session is told to be running by the name of a
/// process under its shell (`codex`, or `codex-x86_64-…`, or a script
/// `…/bin/gemini` run by node).
pub const AGENTS: [&str; 14] = ["claude", "codex", "gemini", "opencode", "aider", "amp", "cursor-agent", "copilot", "qwen", "crush", "goose", "droid", "auggie", "kiro-cli"];

/// What to do for one AI program.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Agent {
    /// Typed in a restored or restarted session where it ran, to take up the
    /// last conversation (`codex resume --last`). Empty: nothing typed.
    pub resume: String,
}

impl Settings {
    /// The line typed to take `agent` up again after a restart: the
    /// settings', else the one known for it.
    pub fn resume_for(&self, agent: &str) -> Option<String> {
        let set = self.agents.get(agent).map(|a| a.resume.trim().to_owned());
        let own = match agent {
            "codex" => Some("codex resume --last".to_owned()),
            _ => None,
        };
        set.or(own).filter(|l| !l.is_empty())
    }

    /// Every AI program's name: the known ones and the settings' own.
    pub fn agent_names(&self) -> Vec<String> {
        let mut out: Vec<String> = AGENTS.iter().map(|s| (*s).to_owned()).collect();
        out.extend(self.agents.keys().filter(|k| !AGENTS.contains(&k.as_str())).cloned());
        out
    }
}

/// What a model's tokens cost, in US dollars per million. A cache write is
/// 1.25 times the input (2 times for the hour-long cache); a cache read is a
/// tenth of the input unless given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
}

/// Startup and closing (the settings screen's General).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Where a new session starts when no pane is open; empty: the home folder.
    pub default_folder: String,
    /// Closing the window leaves the sessions running in the server; off,
    /// closing it ends them.
    pub keep_sessions: bool,
    /// Closing a session with a program running in it asks first.
    pub ask_before_close: bool,
    /// Look once a day for a newer release on GitHub.
    pub check_updates: bool,
    /// A window of a newer version restarts the older server at once (its
    /// shells stop, its tabs come back), rather than asking first.
    pub restart_after_update: bool,
    /// macOS: the window's keys with Cmd (Ctrl+Shift+T is Cmd+T); off, the
    /// same keys as elsewhere.
    pub cmd_on_mac: bool,
    /// Ask before pasting several lines into a program that did not ask for
    /// bracketed paste, which would run them one by one.
    pub warn_multiline_paste: bool,
    /// Ask before pasting more than [`LARGE_PASTE`] bytes.
    pub warn_large_paste: bool,
}

/// A paste this long or longer is asked about (`warn_large_paste`), as
/// Windows Terminal's 5 KiB.
pub const LARGE_PASTE: usize = 5 * 1024;

impl Default for General {
    fn default() -> Self {
        Self { default_folder: String::new(), keep_sessions: true, ask_before_close: true, check_updates: true, restart_after_update: false, cmd_on_mac: true, warn_multiline_paste: true, warn_large_paste: true }
    }
}

/// New sessions and how waiting is told (the settings screen's Sessions).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sessions {
    /// What the new-session dialog has chosen first: `claude`, `resume`, `shell`.
    pub start: String,
    /// The command that is Claude Code (`claude`, or a path to it).
    pub claude: String,
    /// After a restart, run `claude --resume` where Claude Code ran.
    pub resume: bool,
    /// Seconds of silence after which a running program reads as probably
    /// waiting; 0 turns the guess off. Unset: `TSUMUGI_QUIET_SECS`, else 10.
    pub quiet: Option<u64>,
    /// No longer used (0.55: the sidebar turns its last cards into lines
    /// itself when they do not fit); read so older files still load.
    pub compact_after: usize,
}

impl Default for Sessions {
    fn default() -> Self {
        Self { start: "claude".into(), claude: "claude".into(), resume: true, quiet: None, compact_after: 12 }
    }
}

/// The shell sessions start (the settings screen's Shell & hooks).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Shell {
    /// A program on the `PATH` or its path; empty: the system's (`pwsh`
    /// when installed on Windows, else `$SHELL`).
    pub program: String,
    pub args: Vec<String>,
    /// Variables every session gets.
    pub env: std::collections::BTreeMap<String, String>,
}

impl Shell {
    /// The program and its arguments, when one is set.
    pub fn command(&self) -> Option<(String, Vec<String>)> {
        let p = self.program.trim();
        (!p.is_empty()).then(|| (p.to_owned(), self.args.clone()))
    }
}

/// Things to touch rarely (the settings screen's Advanced).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Advanced {
    /// The graphics backend: `auto`, `gl`, `vulkan`, `dx12`, `metal`
    /// (when the window next opens).
    pub backend: String,
    /// Lines of scrollback kept per pane.
    pub scrollback: usize,
    /// Record what each pane sends and receives, for bug reports, in a file
    /// beside the state.
    pub pane_log: bool,
}

impl Default for Advanced {
    fn default() -> Self {
        Self { backend: "auto".into(), scrollback: 10_000, pane_log: false }
    }
}

/// The status bar's clock (the design's 1n).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Clock {
    pub show: bool,
    pub hour24: bool,
    pub date: bool,
    pub date_format: String,
    pub weekday: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self { show: true, hour24: true, date: true, date_format: "YYYY/MM/DD".into(), weekday: true }
    }
}

/// The date formats the clock knows.
pub const DATE_FORMATS: [&str; 4] = ["YYYY/MM/DD", "YYYY-MM-DD", "MM/DD/YYYY", "DD/MM/YYYY"];

impl Clock {
    /// A `chrono` format string: `2026/10/05 (Mon) 14:32`.
    pub fn format(&self) -> String {
        let mut out = Vec::new();
        if self.date {
            out.push(
                match self.date_format.as_str() {
                    "YYYY-MM-DD" => "%Y-%m-%d",
                    "MM/DD/YYYY" => "%m/%d/%Y",
                    "DD/MM/YYYY" => "%d/%m/%Y",
                    _ => "%Y/%m/%d",
                }
                .to_owned(),
            );
            if self.weekday {
                out.push("(%a)".into());
            }
        }
        out.push(if self.hour24 { "%H:%M".into() } else { "%-I:%M %p".into() });
        out.join(" ")
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    /// Percent a pane without the keys is dimmed, 0 to 90.
    pub dim: u8,
    /// Waiting rings breathe and running tabs show a moving line.
    pub animations: bool,
    /// The Nerd Font's icons in prompts and the sidebar, when one is found.
    pub nerd_icons: bool,
    /// The cursor of the pane with the keys: `block`, `bar` or `underline`,
    /// with `-blink` to blink.
    pub cursor: String,
}

/// The window's frame.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Window {
    /// `tsumugi` (its own title bar) or `system`.
    pub titlebar: String,
    /// `none`, `mica` or `acrylic` (Windows 11), `vibrancy` (macOS).
    pub material: String,
    /// How much of the window covers the desktop, in %, 20 to 100: less
    /// lets it show through (from when the window opens next, if it was
    /// opened solid).
    pub opacity: u8,
    /// A picture behind the panes (png or jpeg); `~/` is the home folder,
    /// a relative path is beside this file. Empty for none.
    pub image: String,
    /// How much of the picture shows through the panes, in %.
    pub image_opacity: u8,
}

impl Window {
    pub fn own_titlebar(&self) -> bool {
        self.titlebar != "system"
    }

    /// `opacity` as a share, kept from 0.2 (a window that cannot be seen
    /// is not one) to 1.
    pub fn alpha(&self) -> f32 {
        f32::from(self.opacity.clamp(20, 100)) / 100.0
    }
}

impl Default for Window {
    fn default() -> Self {
        Self { titlebar: "tsumugi".into(), material: "none".into(), opacity: 100, image: String::new(), image_opacity: 25 }
    }
}

/// The panes' font.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Font {
    /// A font file's name, part of it, or its path; empty for the default.
    pub family: String,
    /// In points, 8 to 32.
    pub size: f32,
    /// The row's height for the font's own, 0.8 to 2.
    pub line_height: f32,
    /// `->` as one arrow, where the font has it (Q8).
    pub ligatures: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self { family: String::new(), size: 14.0, line_height: 1.0, ligatures: true }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self { dim: 35, animations: true, nerd_icons: true, cursor: "block-blink".into() }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            dark_theme: "tsumugi Dark".into(),
            light_theme: "tsumugi Light".into(),
            general: General::default(),
            sessions: Sessions::default(),
            shell: Shell::default(),
            advanced: Advanced::default(),
            clock: Clock::default(),
            appearance: Appearance::default(),
            font: Font::default(),
            window: Window::default(),
            keys: std::collections::BTreeMap::new(),
            tags: Tags::default(),
            notify: Notify::default(),
            open: Open::default(),
            menu: Menu::default(),
            prices: std::collections::BTreeMap::new(),
            agents: std::collections::BTreeMap::new(),
        }
    }
}

/// The commands the tab's menu opens a folder with: `{folder}` is the
/// folder, quoted; run by the system's shell (`cmd /C`, `sh -c`), so a
/// `code.cmd` on Windows is found as `code`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Open {
    /// The editors and filers, the first the one a click runs: a command
    /// alone or a list of them.
    #[serde(deserialize_with = "one_or_more")]
    pub editor: Vec<String>,
    #[serde(deserialize_with = "one_or_more")]
    pub filer: Vec<String>,
    /// What a Ctrl+click on a file's path in a pane runs: `{file}` (quoted),
    /// `{line}` and `{column}` (1 when the output named none). Empty: the
    /// system's own way of opening it.
    pub file: String,
}

impl Default for Open {
    fn default() -> Self {
        Self { editor: vec!["code {folder}".into()], filer: vec!["filer {folder}".into()], file: "code --goto {file}:{line}:{column}".into() }
    }
}

/// The tab's right-click menu: built-in items to leave out, by their words
/// (`rename`, `note`, `tags`, `mute`, `pin`, `restart`, `duplicate`, `new-window`,
/// `filer`, `editor`, `copy-path`, `save-output`, `pr`, `close`), and items of one's own.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Menu {
    pub hide: Vec<String>,
    pub session: Vec<MenuItem>,
    /// The built-in items' order, by their words; those not named follow
    /// in their own order.
    pub order: Vec<String>,
}

/// An item of one's own: `{folder}` and `{session}` (its number) are put
/// into the command, which the system's shell runs.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuItem {
    pub name: String,
    pub command: String,
}

/// The words of the menu's own items, for `menu.hide`.
pub const MENU_ITEMS: [&str; 14] = ["rename", "note", "tags", "mute", "pin", "restart", "duplicate", "new-window", "filer", "editor", "copy-path", "save-output", "pr", "close"];

/// The menu's items in the order `order` asks for: those it names first,
/// in its order, then the rest in their own.
pub fn menu_words(order: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for w in order.iter().filter_map(|w| MENU_ITEMS.iter().copied().find(|m| m == w)) {
        if !out.contains(&w) {
            out.push(w);
        }
    }
    let rest: Vec<&'static str> = MENU_ITEMS.iter().copied().filter(|m| !out.contains(m)).collect();
    out.extend(rest);
    out
}

/// The group an item is drawn in; a line goes between two groups.
pub fn menu_group(word: &str) -> &'static str {
    match word {
        "rename" | "note" | "tags" | "mute" | "pin" => "look",
        "restart" | "duplicate" | "new-window" => "start",
        "filer" | "editor" | "copy-path" | "save-output" | "pr" => "folder",
        _ => "close",
    }
}

/// What an item says, for the settings screen.
pub fn menu_label(word: &str) -> &'static str {
    match word {
        "rename" => "Rename",
        "note" => "Note",
        "tags" => "Tags",
        "mute" => "Mute notifications",
        "pin" => "Pin to top",
        "restart" => "Restart",
        "duplicate" => "Duplicate in the same folder",
        "new-window" => "Move to a new window",
        "filer" => "Open the folder in filer",
        "editor" => "Open in the editor",
        "copy-path" => "Copy the folder path",
        "save-output" => "Save the output to a file",
        "pr" => "Create a pull request",
        "close" => "Close the session",
        _ => "",
    }
}

/// A command line with `{folder}` (quoted for the system's shell) and
/// `{session}` put in.
pub fn fill(command: &str, folder: &Path, session: u64) -> String {
    command.replace("{folder}", &shell_word(folder)).replace("{session}", &session.to_string())
}

/// A command, or a list of them; empty ones left out.
fn one_or_more<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Given {
        One(String),
        More(Vec<String>),
    }
    let all = match Given::deserialize(d)? {
        Given::One(s) => vec![s],
        Given::More(v) => v,
    };
    Ok(all.into_iter().filter(|c| !c.trim().is_empty()).collect())
}

/// What a command is called in a menu: its program's name, `sakura` for
/// `"C:\Program Files\sakura\sakura.exe" {folder}`.
pub fn program_name(command: &str) -> String {
    let c = command.trim_start();
    let first = match c.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        None => c.split_whitespace().next().unwrap_or(c),
    };
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first);
    let stem = base.strip_suffix(".exe").or_else(|| base.strip_suffix(".cmd")).or_else(|| base.strip_suffix(".bat")).unwrap_or(base);
    if stem.is_empty() { command.trim().to_owned() } else { stem.to_owned() }
}

/// `[open] file` for a file and the line and column in it.
pub fn fill_file(command: &str, file: &Path, line: u32, column: u32) -> String {
    command.replace("{file}", &shell_word(file)).replace("{line}", &line.to_string()).replace("{column}", &column.to_string())
}

/// A path as one word to the system's shell.
fn shell_word(path: &Path) -> String {
    let f = path.display().to_string();
    if cfg!(windows) { format!("\"{f}\"") } else { format!("'{}'", f.replace('\'', "'\\''")) }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tags {
    pub rule: Vec<TagRule>,
    /// A tag's own colour (`"#rrggbb"`), over the one picked from its name.
    pub colors: std::collections::BTreeMap<String, String>,
    /// How many of a session's tags the band and the cards show before
    /// `+N` (1 to `MAX_TAGS`).
    pub shown: usize,
}

impl Default for Tags {
    fn default() -> Self {
        Self { rule: Vec::new(), colors: Default::default(), shown: 3 }
    }
}

impl Tags {
    /// `shown`, kept between 1 and `MAX_TAGS`.
    pub fn shown(&self) -> usize {
        self.shown.clamp(1, crate::proto::MAX_TAGS)
    }
}

/// A session in `folder`, or anywhere under it, gets `tag`. A `folder`
/// ending in `*` matches each folder in the one before it, and `{name}` in
/// `tag` is that folder's name. A rule with `branch` (`claude/*`: `*` any
/// run of characters) asks for that git branch too, or alone without a
/// folder.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagRule {
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub branch: String,
    pub tag: String,
}

/// Which states tell in which way, by their words: `waiting`, `error`,
/// `done`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notify {
    /// The system's notification.
    pub system: Vec<String>,
    /// Counted in the number on the taskbar button.
    pub taskbar: Vec<String>,
    /// Flash the taskbar button.
    pub flash: Vec<String>,
    /// Play the system's sound.
    pub sound: Vec<String>,
    /// A session done is told only after a run this many seconds long.
    pub long_run: u64,
    /// The sounds for one that waits and one that fails: see [`SOUNDS`].
    pub sound_waiting: String,
    pub sound_error: String,
    /// Windows: nothing shows or sounds while focus mode (do not disturb) is on.
    pub focus_mode: bool,
    /// Where to send a session waiting while no one is at the window: an
    /// ntfy topic's address, a Slack incoming webhook, or any address that
    /// takes JSON. Empty: nowhere.
    pub webhook: String,
    /// What the address takes: `ntfy`, `slack` or `json`.
    pub webhook_format: String,
    /// How many seconds a session waits before it is sent.
    pub webhook_after: u64,
    /// Say so once when the day's estimated cost (US dollars) passes this;
    /// 0: never.
    pub spend_day: u64,
    /// The same for Claude Code's 5-hour block; 0: never.
    pub spend_block: u64,
}

/// The sounds `sound_waiting` and `sound_error` name.
pub const SOUNDS: [&str; 4] = ["chime", "low", "alert", "default"];

impl Default for Notify {
    /// The design's 1h: notifications for all three, the number for the
    /// two that want a person, no flashing.
    fn default() -> Self {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect();
        Self { system: words(&["waiting", "error", "done"]), taskbar: words(&["waiting", "error"]),
            flash: Vec::new(),
            sound: Vec::new(),
            long_run: 60,
            sound_waiting: "chime".into(),
            sound_error: "low".into(),
            focus_mode: true,
            webhook: String::new(),
            webhook_format: "ntfy".into(),
            webhook_after: 120,
            spend_day: 0,
            spend_block: 0,
        }
    }
}

/// Where the file is: `TSUMUGI_SETTINGS` when set; else
/// `%APPDATA%\tsumugi\settings.toml` on Windows, `~/Library/Application
/// Support/tsumugi/settings.toml` on macOS, `$XDG_CONFIG_HOME/tsumugi/
/// settings.toml` (or `~/.config/tsumugi/settings.toml`) elsewhere.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TSUMUGI_SETTINGS").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("APPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))?
    };
    Some(dir.join("tsumugi").join("settings.toml"))
}

/// The settings in `path`: the defaults when there is no file, and what is
/// wrong with it when it cannot be read.
pub fn load(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn parse(text: &str) -> Result<Settings, String> {
    let s: Settings = toml::from_str(text).map_err(|e| match e.span() {
        Some(at) => format!("line {}: {}", text[..at.start].matches('\n').count() + 1, e.message()),
        None => e.message().to_owned(),
    })?;
    if !DATE_FORMATS.contains(&s.clock.date_format.as_str()) {
        return Err(format!("clock.date_format: `{}` is not one of {}", s.clock.date_format, DATE_FORMATS.join(", ")));
    }
    if !matches!(s.window.titlebar.as_str(), "tsumugi" | "system") {
        return Err(format!("window.titlebar: `{}` is not tsumugi or system", s.window.titlebar));
    }
    if !matches!(s.window.material.as_str(), "none" | "mica" | "acrylic" | "vibrancy") {
        return Err(format!("window.material: `{}` is not none, mica, acrylic or vibrancy", s.window.material));
    }
    if !(20..=100).contains(&s.window.opacity) {
        return Err(format!("window.opacity: {} is not 20 to 100", s.window.opacity));
    }
    if s.window.image_opacity > 100 {
        return Err(format!("window.image_opacity: {} is more than 100", s.window.image_opacity));
    }
    if !matches!(s.sessions.start.as_str(), "claude" | "resume" | "shell") {
        return Err(format!("sessions.start: `{}` is not claude, resume or shell", s.sessions.start));
    }
    if !matches!(s.advanced.backend.as_str(), "auto" | "gl" | "vulkan" | "dx12" | "metal") {
        return Err(format!("advanced.backend: `{}` is not auto, gl, vulkan, dx12 or metal", s.advanced.backend));
    }
    if !(100..=1_000_000).contains(&s.advanced.scrollback) {
        return Err(format!("advanced.scrollback: {} is not between 100 and 1000000", s.advanced.scrollback));
    }
    let shapes = ["block", "bar", "underline"];
    if !shapes.contains(&s.appearance.cursor.trim_end_matches("-blink")) {
        return Err(format!("appearance.cursor: `{}` is not block, bar or underline (with -blink to blink)", s.appearance.cursor));
    }
    if !["ntfy", "slack", "json"].contains(&s.notify.webhook_format.as_str()) {
        return Err(format!("notify.webhook_format: `{}` is not ntfy, slack or json", s.notify.webhook_format));
    }
    let webhook = s.notify.webhook.trim();
    if !webhook.is_empty() && !(webhook.starts_with("https://") || webhook.starts_with("http://")) {
        return Err(format!("notify.webhook: `{webhook}` is not an http(s) address"));
    }
    if let Some((model, _)) = s.prices.iter().find(|(_, p)| p.input < 0.0 || p.output < 0.0 || p.cache_read.is_some_and(|r| r < 0.0)) {
        return Err(format!("prices.{model}: a price below zero"));
    }
    for (key, v) in [("notify.sound_waiting", &s.notify.sound_waiting), ("notify.sound_error", &s.notify.sound_error)] {
        if !SOUNDS.contains(&v.as_str()) {
            return Err(format!("{key}: `{v}` is not one of {}", SOUNDS.join(", ")));
        }
    }
    if let Some(w) = s.menu.order.iter().find(|w| !MENU_ITEMS.contains(&w.as_str())) {
        return Err(format!("menu.order: `{w}` is not one of {}", MENU_ITEMS.join(", ")));
    }
    if let Some(r) = s.tags.rule.iter().find(|r| r.folder.trim().is_empty() && r.branch.trim().is_empty()) {
        return Err(format!("tags.rule for `{}`: a folder, a branch or both", r.tag));
    }
    if !(8.0..=32.0).contains(&s.font.size) {
        return Err(format!("font.size: {} is not between 8 and 32", s.font.size));
    }
    if !(0.8..=2.0).contains(&s.font.line_height) {
        return Err(format!("font.line_height: {} is not between 0.8 and 2", s.font.line_height));
    }
    if s.appearance.dim > 90 {
        return Err(format!("appearance.dim: {} is more than 90", s.appearance.dim));
    }
    if let Some(w) = s.menu.hide.iter().find(|w| !MENU_ITEMS.contains(&w.as_str())) {
        return Err(format!("menu.hide: `{w}` is not one of {}", MENU_ITEMS.join(", ")));
    }
    for (key, words) in [("system", &s.notify.system), ("taskbar", &s.notify.taskbar), ("flash", &s.notify.flash), ("sound", &s.notify.sound)] {
        if let Some(w) = words.iter().find(|w| !matches!(w.as_str(), "waiting" | "error" | "done")) {
            return Err(format!("notify.{key}: `{w}` is not waiting, error or done"));
        }
    }
    Ok(s)
}

/// The table at the dotted `path` (`None`: the top), made when it is not
/// there; an error when the path is something else (an array of tables).
fn table_at<'a>(doc: &'a mut toml_edit::DocumentMut, path: Option<&str>) -> Result<&'a mut toml_edit::Table, String> {
    let mut t = doc.as_table_mut();
    let Some(path) = path else { return Ok(t) };
    for part in path.split('.') {
        let item = t.entry(part).or_insert_with(|| {
            let mut new = toml_edit::Table::new();
            new.set_implicit(true);
            toml_edit::Item::Table(new)
        });
        t = item.as_table_mut().ok_or_else(|| format!("[{path}] is not a table in the file"))?;
    }
    Ok(t)
}

fn document(text: &str) -> Result<toml_edit::DocumentMut, String> {
    text.parse::<toml_edit::DocumentMut>().map_err(|e| format!("settings.toml does not read, so it is left as it is: {e}"))
}

/// A key as the settings write it (`A`, `"my tag"`), as the name it stands for.
fn key_name(key: &str) -> Result<String, String> {
    let parts = toml_edit::Key::parse(key).map_err(|e| format!("`{key}` is not a key: {e}"))?;
    match parts.as_slice() {
        [one] => Ok(one.get().to_owned()),
        _ => Err(format!("`{key}` is not a single key")),
    }
}

/// `text` with `key` in `table` (`None`: the top, before any table) set to
/// `value`, already written as TOML. Only that key changes, so what a person
/// wrote around it -- comments, order, blank lines -- stays. A key not there
/// is added at the end of its table, and a table not there at the end of the
/// file. An error, and nothing to write, when the file does not read as TOML
/// or the value is not a TOML value.
pub fn set_key(text: &str, table: Option<&str>, key: &str, value: &str) -> Result<String, String> {
    let mut doc = document(text)?;
    let mut new: toml_edit::Value = value.parse().map_err(|e| format!("`{value}` is not a TOML value: {e}"))?;
    let name = key_name(key)?;
    let t = table_at(&mut doc, table)?;
    // A new table that only held tables before is shown now it has a key.
    t.set_implicit(false);
    match t.get_mut(&name).and_then(toml_edit::Item::as_value_mut) {
        Some(old) => {
            // The comment after the old value stays after the new one.
            let suffix = old.decor().suffix().and_then(|s| s.as_str()).map(str::to_owned);
            new.decor_mut().set_prefix(" ");
            new.decor_mut().set_suffix(suffix.as_deref().filter(|s| s.contains('#')).unwrap_or(""));
            *old = new;
        }
        None => {
            t.insert(&name, toml_edit::Item::Value(new));
        }
    }
    Ok(doc.to_string())
}

/// `text` without `key` in `table`; the same text when it is not there.
pub fn remove_key(text: &str, table: Option<&str>, key: &str) -> Result<String, String> {
    let mut doc = document(text)?;
    let name = key_name(key)?;
    let mut t = doc.as_table_mut();
    if let Some(path) = table {
        for part in path.split('.') {
            match t.get_mut(part).and_then(toml_edit::Item::as_table_mut) {
                Some(next) => t = next,
                None => return Ok(text.to_owned()),
            }
        }
    }
    if t.remove(&name).is_none() {
        return Ok(text.to_owned());
    }
    Ok(doc.to_string())
}

/// `text` with every `[[name]]` block taken out and `blocks` written at the
/// end in their place, each a list of keys and their values as TOML.
/// Comments inside the old blocks go with them; the rest stays.
pub fn set_tables(text: &str, name: &str, blocks: &[Vec<(&str, String)>]) -> Result<String, String> {
    let mut doc = document(text)?;
    let (parent, last) = match name.rsplit_once('.') {
        Some((p, l)) => (Some(p), l),
        None => (None, name),
    };
    let mut list = toml_edit::ArrayOfTables::new();
    for b in blocks {
        let mut t = toml_edit::Table::new();
        for (k, v) in b {
            let v: toml_edit::Value = v.parse().map_err(|e| format!("`{v}` is not a TOML value: {e}"))?;
            t.insert(&key_name(k)?, toml_edit::Item::Value(v));
        }
        list.push(t);
    }
    let t = table_at(&mut doc, parent)?;
    t.remove(last);
    if !list.is_empty() {
        t.insert(last, toml_edit::Item::ArrayOfTables(list));
    }
    Ok(doc.to_string())
}

/// A string as TOML writes it, quoted and escaped.
pub fn quote(s: &str) -> String {
    toml::Value::String(s.to_owned()).to_string()
}

/// A list of strings as TOML writes it.
pub fn quote_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "))
}

/// When the file was last changed, to read it again only then; `None` when
/// there is none.
pub fn stamp(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A new session's choices kept under a name (the design's 1g): the window
/// writes these to `profiles.toml` beside the settings, whole, so the
/// settings a person wrote by hand are never rewritten.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub folder: String,
    /// `claude`, `resume` or `shell`.
    pub start: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// More panes beside the first, each started the same ways: a second on
    /// the right, a third below it, a fourth below the first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panes: Vec<String>,
    /// Where it runs: empty for this machine, `wsl:<distribution>` or
    /// `ssh:<host>`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place: String,
}

#[derive(Default, Serialize, Deserialize)]
struct ProfileFile {
    #[serde(default)]
    profile: Vec<Profile>,
}

/// `profiles.toml`, beside the settings file.
pub fn profiles_path() -> Option<PathBuf> {
    default_path().map(|p| p.with_file_name("profiles.toml"))
}

pub fn load_profiles(path: &Path) -> Result<Vec<Profile>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str::<ProfileFile>(&text).map(|f| f.profile).map_err(|e| format!("{}: {}", path.display(), e.message())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn save_profiles(path: &Path, profiles: &[Profile]) -> std::io::Result<()> {
    let text = toml::to_string(&ProfileFile { profile: profiles.to_vec() }).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

/// The user's home folder, for a `~` in a rule.
pub fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl TagRule {
    /// The tag this rule gives a session in `cwd` on `branch`, if any.
    pub fn tag_for_session(&self, cwd: &Path, branch: &str, home: Option<&Path>) -> Option<String> {
        if self.folder.trim().is_empty() && self.branch.trim().is_empty() {
            return None;
        }
        if !self.branch.trim().is_empty() && !glob(self.branch.trim(), branch) {
            return None;
        }
        if self.folder.trim().is_empty() {
            return crate::proto::tag_name(&self.tag);
        }
        self.tag_for(cwd, home)
    }

    /// The tag this rule's folder gives a session in `cwd`, if any.
    pub fn tag_for(&self, cwd: &Path, home: Option<&Path>) -> Option<String> {
        if self.folder.trim().is_empty() {
            return None;
        }
        let folder = match self.folder.strip_prefix('~') {
            Some(rest) => home?.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(&self.folder),
        };
        let mut want: Vec<String> = parts(&folder);
        let any_child = want.last().is_some_and(|p| p == "*");
        if any_child {
            want.pop();
        }
        let have = parts(cwd);
        if have.len() < want.len() + usize::from(any_child) || have[..want.len()] != want[..] {
            return None;
        }
        let tag = if any_child {
            // The name as the folder spells it, not as compared.
            let name = cwd.components().filter(|c| matches!(c, Component::Normal(_))).nth(normal_count(&folder) - 1)?;
            self.tag.replace("{name}", &name.as_os_str().to_string_lossy())
        } else {
            self.tag.clone()
        };
        crate::proto::tag_name(&tag)
    }
}

/// Whether `text` fits `pattern`, where `*` is any run of characters.
pub fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text[first.len()..].ends_with(last) || text.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for p in &parts[1..parts.len() - 1] {
        match rest.find(p) {
            Some(at) => rest = &rest[at + p.len()..],
            None => return false,
        }
    }
    true
}

/// A path's parts to compare: the prefix and the names, without `.`, and
/// without case on Windows, where folder names are compared that way.
fn parts(p: &Path) -> Vec<String> {
    p.components()
        .filter(|c| !matches!(c, Component::CurDir | Component::RootDir))
        .map(|c| {
            let s = c.as_os_str().to_string_lossy();
            if cfg!(windows) { s.to_lowercase() } else { s.into_owned() }
        })
        .collect()
}

/// How many plain names `p` has (the `*` among them).
fn normal_count(p: &Path) -> usize {
    p.components().filter(|c| matches!(c, Component::Normal(_))).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(folder: &str, tag: &str) -> TagRule {
        TagRule { folder: folder.into(), tag: tag.into(), ..TagRule::default() }
    }

    #[test]
    fn the_font_reads_and_is_kept_in_bounds() {
        let s = parse("[font]\nfamily = \"JetBrainsMono\"\nsize = 15\nline_height = 1.2\n").expect("whole points read");
        assert_eq!(s.font, Font { family: "JetBrainsMono".into(), size: 15.0, line_height: 1.2, ligatures: true });
        assert!(parse("[font]\nsize = 4\n").is_err());
        assert!(parse("[font]\nline_height = 3.0\n").is_err());
        assert_eq!(parse("").unwrap().font, Font::default());
        assert!(parse("").unwrap().window.own_titlebar());
        assert_eq!(parse("[keys]\nnew_tab = \"Ctrl+Shift+N\"\nzoom = \"none\"\n").unwrap().keys.len(), 2);
        assert!(!parse("[window]\ntitlebar = \"system\"\n").unwrap().window.own_titlebar());
        assert!(parse("[window]\nmaterial = \"glass\"\n").is_err());
        assert_eq!(parse("[window]\nopacity = 80\nimage = \"~/bg.png\"\n").unwrap().window.alpha(), 0.8);
        assert_eq!(parse("").unwrap().window.alpha(), 1.0);
        assert!(parse("[window]\nopacity = 10\n").is_err());
        assert!(parse("[window]\nimage_opacity = 101\n").is_err());
    }

    #[test]
    fn keys_go_and_tables_are_rewritten() {
        let text = "# mine\n[shell.env]\nA = \"1\"\nB = \"2\"\n\n[[tags.rule]]\n# old\nfolder = \"~/a\"\ntag = \"a\"\n\n[clock]\nshow = true\n";
        let t = remove_key(text, Some("shell.env"), "A").unwrap();
        assert_eq!(t, "# mine\n[shell.env]\nB = \"2\"\n\n[[tags.rule]]\n# old\nfolder = \"~/a\"\ntag = \"a\"\n\n[clock]\nshow = true\n");
        assert_eq!(remove_key(text, Some("shell.env"), "Z").unwrap(), text, "not there: as it was");
        let rules = vec![vec![("folder", quote("~/b")), ("tag", quote("b"))], vec![("branch", quote("claude/*")), ("tag", quote("claude"))]];
        let t = set_tables(text, "tags.rule", &rules).unwrap();
        assert!(t.starts_with("# mine\n[shell.env]\nA = \"1\"\nB = \"2\"\n") && t.contains("[clock]\nshow = true\n") && !t.contains("# old"), "{t}");
        assert!(t.contains("[[tags.rule]]\nfolder = \"~/b\""), "{t}");
        let s = parse(&t).expect("reads");
        assert_eq!(s.tags.rule.len(), 2);
        assert_eq!(s.tags.rule[1].branch, "claude/*");
        assert_eq!(set_tables(&t, "tags.rule", &rules).unwrap(), t, "the same twice");
        assert!(parse(&set_tables(&t, "tags.rule", &[]).unwrap()).unwrap().tags.rule.is_empty());
    }

    #[test]
    fn a_rule_by_branch_tags_its_sessions() {
        let r = TagRule { branch: "claude/*".into(), tag: "claude".into(), ..TagRule::default() };
        assert_eq!(r.tag_for_session(Path::new("/x"), "claude/task-1", None).as_deref(), Some("claude"));
        assert_eq!(r.tag_for_session(Path::new("/x"), "main", None), None);
        let both = TagRule { folder: "/srv".into(), branch: "main".into(), tag: "prod".into() };
        assert_eq!(both.tag_for_session(Path::new("/srv/app"), "main", None).as_deref(), Some("prod"));
        assert_eq!(both.tag_for_session(Path::new("/srv/app"), "dev", None), None);
        assert!(glob("a*b*c", "aXXbYc") && !glob("a*b", "ac") && glob("*", "") && glob("x", "x"));
        assert!(parse("[[tags.rule]]\ntag = \"t\"\n").is_err(), "neither folder nor branch");
    }

    #[test]
    fn the_example_reads() {
        let text = include_str!("settings.rs").lines().filter_map(|l| l.strip_prefix("//! ")).skip_while(|l| !l.starts_with("# Tag")).take_while(|l| !l.starts_with("```")).collect::<Vec<_>>().join("\n");
        let s = parse(&text).expect("the example in the module's comment reads");
        let by_branch = TagRule { branch: "claude/*".into(), tag: "claude".into(), ..TagRule::default() };
        assert_eq!(s.tags.rule, vec![rule("~/dev/filer", "filer"), rule("~/dev/*", "{name}"), by_branch]);
        assert_eq!(s.tags.colors.get("claude").map(String::as_str), Some("#5e4a86"));
        assert_eq!(s.notify, Notify { flash: vec![], webhook: "https://ntfy.sh/my-topic".into(), spend_day: 20, spend_block: 10, ..Notify::default() });
        assert_eq!(s.prices.get("claude-opus-5-5"), Some(&Price { input: 4.0, output: 20.0, cache_read: Some(0.2) }));
        assert!(parse("[notify]\nwebhook = \"file:///etc\"\n").is_err(), "an http(s) address only");
        assert!(parse("[notify]\nwebhook_format = \"xml\"\n").is_err());
        assert_eq!(s.general.default_folder, "~/dev");
        assert_eq!(s.sessions.quiet, Some(10));
        assert_eq!(s.shell.command(), Some(("pwsh".to_string(), vec!["-NoLogo".to_string()])));
        assert_eq!(s.shell.env.get("EDITOR").map(String::as_str), Some("code --wait"));
        assert_eq!(s.advanced, Advanced::default());
        assert_eq!(s.open, Open { editor: vec!["code {folder}".into(), "sakura {folder}".into()], ..Open::default() });
        assert_eq!(s.menu.hide, ["new-window"]);
        assert_eq!(s.menu.session[0].command, "wt -d {folder} lazygit");
    }

    #[test]
    fn profiles_come_back_as_written() {
        let dir = std::env::temp_dir().join(format!("tsumugi-profiles-test-{}", std::process::id()));
        let path = dir.join("profiles.toml");
        assert_eq!(load_profiles(&path), Ok(vec![]), "no file, no profiles");
        let list = vec![
            Profile { name: "filer".into(), folder: "~/dev/filer".into(), start: "claude".into(), tags: vec!["review".into()], panes: vec!["claude".into(), "shell".into()], place: "ssh:box".into() },
            Profile { name: "a \"quoted\" one".into(), folder: r"C:\dev\x".into(), start: "shell".into(), tags: vec![], panes: vec![], place: String::new() },
        ];
        save_profiles(&path, &list).unwrap();
        assert_eq!(load_profiles(&path), Ok(list));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_command_gets_the_folder_quoted() {
        assert_eq!(fill("code {folder} # {session}", Path::new("/tmp/it's here"), 7), "code '/tmp/it'\\''s here' # 7");
        assert!(parse("[menu]\nhide = [\"nope\"]").unwrap_err().starts_with("menu.hide: `nope`"));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_gets_its_line_and_column() {
        let line = fill_file(&Open::default().file, Path::new("/src/a b.rs"), 12, 3);
        assert_eq!(line, "code --goto '/src/a b.rs':12:3");
    }

    #[test]
    fn one_key_changes_and_the_rest_stays() {
        let text = "# mine\ntheme = \"dark\" # was light\n\n[notify]\n# the toasts\nsystem = [\n  \"waiting\",\n  \"error\",\n]\nflash = []\n\n[[tags.rule]]\nfolder = \"~\"\ntag = \"home\"\n";
        let set = |table, key, value: &str| set_key(text, table, key, value).unwrap();
        let t = set(None, "theme", &quote("Nord"));
        assert_eq!(t, text.replace("theme = \"dark\"", "theme = \"Nord\""), "its comment stays");
        let t = set(Some("notify"), "system", &quote_list(&["done".into()]));
        assert_eq!(t, "# mine\ntheme = \"dark\" # was light\n\n[notify]\n# the toasts\nsystem = [\"done\"]\nflash = []\n\n[[tags.rule]]\nfolder = \"~\"\ntag = \"home\"\n");
        // Added: at the end of its table (the top's too), a new table at
        // the end of the file.
        let t = set(Some("notify"), "taskbar", "[]");
        assert!(t.contains("flash = []\ntaskbar = []\n"), "{t}");
        let t = set(Some("clock"), "hour24", "false");
        assert!(t.contains("[clock]\nhour24 = false\n"), "{t}");
        let t = set(None, "dark_theme", &quote("Nord"));
        assert!(t.contains("dark_theme = \"Nord\"\n") && t.find("dark_theme") < t.find("[notify]"), "{t}");
        // An array of tables takes no plain key: nothing to write.
        assert!(set_key(text, Some("tags.rule"), "tag", &quote("x")).is_err());
        // Every result still reads.
        assert!(parse(&set(Some("clock"), "hour24", "false")).is_ok());
        // An empty file.
        assert_eq!(set_key("", Some("clock"), "date", "true").unwrap(), "[clock]\ndate = true\n");
        assert_eq!(set_key("", None, "theme", &quote("light")).unwrap(), "theme = \"light\"\n");
    }

    /// What broke the hand-written editor (the source review, 2026-10-07):
    /// brackets in a string, a multi-line string, an array of arrays, a
    /// dotted table, a quoted key; and a file that does not read is left
    /// alone rather than rewritten.
    #[test]
    fn awkward_toml_is_edited_without_loss() {
        let text = "[keys]\nback = \"Ctrl+[\" # open\n\n[font]\nsize = 14\n";
        let t = set_key(text, Some("keys"), "back", &quote("Ctrl+]")).unwrap();
        assert_eq!(t, "[keys]\nback = \"Ctrl+]\" # open\n\n[font]\nsize = 14\n");
        let text = "[notify]\nwebhook = \"\"\"\nhttps://a\n\"\"\"\nsound = []\n";
        let t = set_key(text, Some("notify"), "webhook", &quote("https://b")).unwrap();
        assert!(parse(&t).is_ok() && t.contains("webhook = \"https://b\"") && t.contains("sound = []"), "{t}");
        let text = "[notify]\nsystem = [\n  [\"a\"],\n]\n[clock]\nshow = true\n";
        let t = set_key(text, Some("notify"), "flash", "[]").unwrap();
        assert!(t.contains("flash = []") && t.contains("[clock]\nshow = true"), "{t}");
        let t = set_key("", Some("tags.colors"), "\"my tag\"", &quote("#112233")).unwrap();
        assert_eq!(parse(&t).unwrap().tags.colors.get("my tag").map(String::as_str), Some("#112233"), "{t}");
        assert_eq!(remove_key(&t, Some("tags.colors"), "\"my tag\"").map(|t| parse(&t).unwrap().tags.colors.len()), Ok(0));
        assert!(set_key("[broken\n", None, "theme", &quote("x")).is_err(), "a file that does not read is not rewritten");
    }

    #[test]
    fn the_clock_writes_its_format() {
        assert_eq!(Clock::default().format(), "%Y/%m/%d (%a) %H:%M");
        let us = Clock { hour24: false, date_format: "MM/DD/YYYY".into(), weekday: false, ..Clock::default() };
        assert_eq!(us.format(), "%m/%d/%Y %-I:%M %p");
        assert_eq!(Clock { date: false, ..Clock::default() }.format(), "%H:%M");
        assert!(parse("[clock]\ndate_format = \"DD.MM\"").is_err());
    }

    #[test]
    fn nothing_written_is_the_defaults() {
        assert_eq!(parse("").unwrap(), Settings::default());
        assert_eq!(parse("[notify]\nflash = [\"waiting\"]").unwrap().notify.system, Notify::default().system);
    }

    #[test]
    fn a_mistake_is_told() {
        let e = parse("[tags]\nrules = []").unwrap_err();
        assert!(e.starts_with("line 2: ") && e.contains("rules"), "an unknown key is named, and where: {e}");
        assert!(parse("[[tags.rule]]\nfolder = \"x\"").is_err(), "a rule without a tag");
        assert_eq!(parse("[notify]\nflash = [\"wait\"]").unwrap_err(), "notify.flash: `wait` is not waiting, error or done");
    }

    #[cfg(unix)]
    #[test]
    fn rules_match_the_folder_and_under_it() {
        let home = Path::new("/home/u");
        let filer = rule("~/dev/filer", "#filer");
        assert_eq!(filer.tag_for(Path::new("/home/u/dev/filer"), Some(home)).as_deref(), Some("filer"));
        assert_eq!(filer.tag_for(Path::new("/home/u/dev/filer/src/ui"), Some(home)).as_deref(), Some("filer"));
        assert_eq!(filer.tag_for(Path::new("/home/u/dev/filer2"), Some(home)), None);
        assert_eq!(filer.tag_for(Path::new("/home/u/dev"), Some(home)), None);
        assert_eq!(filer.tag_for(Path::new("/home/u/dev/filer"), None), None, "no home, no ~");
        let each = rule("~/dev/*", "{name}");
        assert_eq!(each.tag_for(Path::new("/home/u/dev/tsumugi/crates"), Some(home)).as_deref(), Some("tsumugi"));
        assert_eq!(each.tag_for(Path::new("/home/u/dev"), Some(home)), None, "the folder itself has no name to give");
        let abs = rule("/srv/ci", "ci run");
        assert_eq!(abs.tag_for(Path::new("/srv/ci/job-1"), Some(home)).as_deref(), Some("ci-run"));
    }

    #[cfg(windows)]
    #[test]
    fn rules_ignore_case_on_windows() {
        let home = Path::new(r"C:\Users\u");
        let each = rule(r"~\Dev\*", "{name}");
        assert_eq!(each.tag_for(Path::new(r"c:\users\U\dev\Filer\src"), Some(home)).as_deref(), Some("Filer"));
        let fwd = rule("~/dev/filer", "filer");
        assert_eq!(fwd.tag_for(Path::new(r"C:\Users\u\dev\filer"), Some(home)).as_deref(), Some("filer"));
    }

    #[test]
    fn the_menu_order_puts_the_named_first() {
        let order: Vec<String> = ["close", "editor", "close", "nope"].iter().map(|s| s.to_string()).collect();
        let words = menu_words(&order);
        assert_eq!(&words[..3], &["close", "editor", "rename"]);
        assert_eq!(words.len(), MENU_ITEMS.len());
        assert_eq!(menu_words(&[]), MENU_ITEMS.to_vec());
        assert!(MENU_ITEMS.iter().all(|w| !menu_label(w).is_empty()));
    }

    #[test]
    fn an_editor_is_one_command_or_more() {
        let one = parse("[open]\nfiler = \"filer {folder}\"\n").unwrap();
        assert_eq!(one.open.filer, ["filer {folder}"]);
        let more = parse("[open]\neditor = [\"code {folder}\", \"\", \"sakura {folder}\"]\nfiler = []\n").unwrap();
        assert_eq!(more.open.editor, ["code {folder}", "sakura {folder}"], "an empty one left out");
        assert!(more.open.filer.is_empty());
        assert_eq!(program_name("code {folder}"), "code");
        assert_eq!(program_name(r#""C:\Program Files\sakura\sakura.exe" {folder}"#), "sakura");
        assert_eq!(program_name("/usr/bin/subl -n {folder}"), "subl");
        assert_eq!(program_name("code.cmd {folder}"), "code");
    }
}
