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
//! # "system". material is "none", or on Windows 11 "mica" or "acrylic": the
//! # desktop shows through the band, sidebar and status bar (on restart).
//! [window]
//! titlebar = "tsumugi"
//! material = "none"
//!
//! # The panes' font: a font file's name (or part of it, "JetBrainsMono")
//! # or its path; "" for the Nerd Font found, else the built-in one. Its Bold
//! # and Italic files beside it are used for bold and italic text.
//! [font]
//! family = ""
//! size = 14
//! line_height = 1.0
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
//!
//! # What a tab's menu runs to open its folder; {folder} is the folder.
//! [open]
//! editor = "code {folder}"
//! filer = "filer {folder}"
//!
//! # The tab's menu (the design's 1j): items left out, and more of your own.
//! [menu]
//! hide = ["new-window"]
//!
//! [[menu.session]]
//! name = "Open lazygit here"
//! command = "wt -d {folder} lazygit"
//! ```

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub theme: String,
    pub dark_theme: String,
    pub light_theme: String,
    pub clock: Clock,
    pub appearance: Appearance,
    pub font: Font,
    pub window: Window,
    pub tags: Tags,
    pub notify: Notify,
    pub open: Open,
    pub menu: Menu,
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
}

/// The window's frame.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Window {
    /// `tsumugi` (its own title bar) or `system`.
    pub titlebar: String,
    /// `none`, `mica` or `acrylic` (Windows 11).
    pub material: String,
}

impl Window {
    pub fn own_titlebar(&self) -> bool {
        self.titlebar != "system"
    }
}

impl Default for Window {
    fn default() -> Self {
        Self { titlebar: "tsumugi".into(), material: "none".into() }
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
}

impl Default for Font {
    fn default() -> Self {
        Self { family: String::new(), size: 14.0, line_height: 1.0 }
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self { dim: 35, animations: true }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            dark_theme: "tsumugi Dark".into(),
            light_theme: "tsumugi Light".into(),
            clock: Clock::default(),
            appearance: Appearance::default(),
            font: Font::default(),
            window: Window::default(),
            tags: Tags::default(),
            notify: Notify::default(),
            open: Open::default(),
            menu: Menu::default(),
        }
    }
}

/// The commands the tab's menu opens a folder with: `{folder}` is the
/// folder, quoted; run by the system's shell (`cmd /C`, `sh -c`), so a
/// `code.cmd` on Windows is found as `code`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Open {
    pub editor: String,
    pub filer: String,
}

impl Default for Open {
    fn default() -> Self {
        Self { editor: "code {folder}".into(), filer: "filer {folder}".into() }
    }
}

/// The tab's right-click menu: built-in items to leave out, by their words
/// (`rename`, `tags`, `mute`, `pin`, `restart`, `duplicate`, `new-window`,
/// `filer`, `editor`, `copy-path`, `close`), and items of one's own.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Menu {
    pub hide: Vec<String>,
    pub session: Vec<MenuItem>,
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
pub const MENU_ITEMS: [&str; 11] = ["rename", "tags", "mute", "pin", "restart", "duplicate", "new-window", "filer", "editor", "copy-path", "close"];

/// A command line with `{folder}` (quoted for the system's shell) and
/// `{session}` put in.
pub fn fill(command: &str, folder: &Path, session: u64) -> String {
    let f = folder.display().to_string();
    let quoted = if cfg!(windows) { format!("\"{f}\"") } else { format!("'{}'", f.replace('\'', "'\\''")) };
    command.replace("{folder}", &quoted).replace("{session}", &session.to_string())
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tags {
    pub rule: Vec<TagRule>,
}

/// A session in `folder`, or anywhere under it, gets `tag`. A `folder`
/// ending in `*` matches each folder in the one before it, and `{name}` in
/// `tag` is that folder's name.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagRule {
    pub folder: String,
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
}

impl Default for Notify {
    /// The design's 1h: notifications for all three, the number for the
    /// two that want a person, no flashing.
    fn default() -> Self {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect();
        Self { system: words(&["waiting", "error", "done"]), taskbar: words(&["waiting", "error"]), flash: Vec::new(), sound: Vec::new() }
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
    if !matches!(s.window.material.as_str(), "none" | "mica" | "acrylic") {
        return Err(format!("window.material: `{}` is not none, mica or acrylic", s.window.material));
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

/// `text` with `key` in `table` (`None`: the top, before any table) set to
/// `value`, already written as TOML. Only that key's line changes (its
/// array, if it runs over several lines), so what a person wrote around it
/// -- comments, order, blank lines -- stays. A key not there is added at
/// the end of its table, and a table not there at the end of the file.
pub fn set_key(text: &str, table: Option<&str>, key: &str, value: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let header = |l: &str| -> Option<String> {
        let t = l.trim();
        let inner = t.strip_prefix("[[").and_then(|r| r.split("]]").next()).or_else(|| t.strip_prefix('[').and_then(|r| r.split(']').next()))?;
        // An array of tables never takes a plain key from here.
        Some(if t.starts_with("[[") { format!("[[{}]]", inner.trim()) } else { inner.trim().to_owned() })
    };
    let is_key = |l: &str| {
        let t = l.trim_start();
        t.strip_prefix(key).is_some_and(|rest| rest.trim_start().starts_with('='))
    };
    let mut section: Option<String> = None;
    let mut start = None;
    let mut last_in_table = None;
    let mut first_header = None;
    for (k, l) in lines.iter().enumerate() {
        if let Some(h) = header(l) {
            first_header.get_or_insert(k);
            section = Some(h);
            if section.as_deref() == table {
                last_in_table = Some(k);
            }
            continue;
        }
        if section.as_deref() != table {
            continue;
        }
        if !l.trim().is_empty() && !l.trim_start().starts_with('#') {
            last_in_table = Some(k);
        }
        if start.is_none() && is_key(l) {
            start = Some(k);
        }
    }
    let line = format!("{key} = {value}\n");
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    match start {
        Some(k) => {
            // An array over several lines ends where its brackets close.
            let mut end = k;
            let mut depth: i32 = 0;
            for (j, l) in lines.iter().enumerate().skip(k) {
                let v = if j == k { l.split_once('=').map_or("", |(_, v)| v) } else { l };
                depth += v.matches('[').count() as i32 - v.matches(']').count() as i32;
                end = j;
                if depth <= 0 {
                    break;
                }
            }
            let indent: String = lines[k].chars().take_while(|c| c.is_whitespace()).collect();
            out.splice(k..=end, [format!("{indent}{line}")]);
        }
        None => match (table, last_in_table) {
            (_, Some(k)) => {
                if !out[k].ends_with('\n') {
                    out[k].push('\n');
                }
                out.insert(k + 1, line);
            }
            (Some(t), None) => {
                if out.last().is_some_and(|l| !l.ends_with('\n')) {
                    out.push("\n".into());
                }
                if !out.is_empty() {
                    out.push("\n".into());
                }
                out.push(format!("[{t}]\n{line}"));
            }
            (None, None) => out.insert(first_header.unwrap_or(out.len()), if first_header.is_some() { format!("{line}\n") } else { line }),
        },
    }
    out.concat()
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
    /// The tag this rule gives a session in `cwd`, if any.
    pub fn tag_for(&self, cwd: &Path, home: Option<&Path>) -> Option<String> {
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
        TagRule { folder: folder.into(), tag: tag.into() }
    }

    #[test]
    fn the_font_reads_and_is_kept_in_bounds() {
        let s = parse("[font]\nfamily = \"JetBrainsMono\"\nsize = 15\nline_height = 1.2\n").expect("whole points read");
        assert_eq!(s.font, Font { family: "JetBrainsMono".into(), size: 15.0, line_height: 1.2 });
        assert!(parse("[font]\nsize = 4\n").is_err());
        assert!(parse("[font]\nline_height = 3.0\n").is_err());
        assert_eq!(parse("").unwrap().font, Font::default());
        assert!(parse("").unwrap().window.own_titlebar());
        assert!(!parse("[window]\ntitlebar = \"system\"\n").unwrap().window.own_titlebar());
        assert!(parse("[window]\nmaterial = \"glass\"\n").is_err());
    }

    #[test]
    fn the_example_reads() {
        let text = include_str!("settings.rs").lines().filter_map(|l| l.strip_prefix("//! ")).skip_while(|l| !l.starts_with("# Tag")).take_while(|l| !l.starts_with("```")).collect::<Vec<_>>().join("\n");
        let s = parse(&text).expect("the example in the module's comment reads");
        assert_eq!(s.tags.rule, vec![rule("~/dev/filer", "filer"), rule("~/dev/*", "{name}")]);
        assert_eq!(s.notify, Notify { flash: vec![], ..Notify::default() });
        assert_eq!(s.open, Open::default());
        assert_eq!(s.menu.hide, ["new-window"]);
        assert_eq!(s.menu.session[0].command, "wt -d {folder} lazygit");
    }

    #[test]
    fn profiles_come_back_as_written() {
        let dir = std::env::temp_dir().join(format!("tsumugi-profiles-test-{}", std::process::id()));
        let path = dir.join("profiles.toml");
        assert_eq!(load_profiles(&path), Ok(vec![]), "no file, no profiles");
        let list = vec![
            Profile { name: "filer".into(), folder: "~/dev/filer".into(), start: "claude".into(), tags: vec!["review".into()] },
            Profile { name: "a \"quoted\" one".into(), folder: r"C:\dev\x".into(), start: "shell".into(), tags: vec![] },
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

    #[test]
    fn one_key_changes_and_the_rest_stays() {
        let text = "# mine\ntheme = \"dark\" \n\n[notify]\n# the toasts\nsystem = [\n  \"waiting\",\n  \"error\",\n]\nflash = []\n\n[[tags.rule]]\nfolder = \"~\"\ntag = \"home\"\n";
        let t = set_key(text, None, "theme", &quote("Nord"));
        assert_eq!(t, text.replace("theme = \"dark\" ", "theme = \"Nord\""));
        let t = set_key(text, Some("notify"), "system", &quote_list(&["done".into()]));
        assert_eq!(t, "# mine\ntheme = \"dark\" \n\n[notify]\n# the toasts\nsystem = [\"done\"]\nflash = []\n\n[[tags.rule]]\nfolder = \"~\"\ntag = \"home\"\n");
        // Added: at the end of its table (the top's too), a new table at
        // the end of the file.
        let t = set_key(text, Some("notify"), "taskbar", "[]");
        assert!(t.contains("flash = []\ntaskbar = []\n\n[[tags.rule]]"), "{t}");
        let t = set_key(text, Some("clock"), "hour24", "false");
        assert!(t.ends_with("tag = \"home\"\n\n[clock]\nhour24 = false\n"), "{t}");
        let t = set_key(text, None, "dark_theme", &quote("Nord"));
        assert!(t.contains("theme = \"dark\" \ndark_theme = \"Nord\"\n\n[notify]"), "{t}");
        // An array of tables' key is not a table's.
        let t = set_key(text, Some("tags.rule"), "tag", &quote("x"));
        assert!(t.contains("tag = \"home\""), "{t}");
        // Every result still reads.
        assert!(parse(&set_key(text, Some("clock"), "hour24", "false")).is_ok());
        // An empty file.
        assert_eq!(set_key("", Some("clock"), "date", "true"), "[clock]\ndate = true\n");
        assert_eq!(set_key("", None, "theme", &quote("light")), "theme = \"light\"\n");
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
}
