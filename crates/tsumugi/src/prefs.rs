//! The settings screen (the design's 1m and "Settings: every page",
//! `Ctrl+,`): nine pages down the left under a search field, each a few
//! cards of rows. It shows what `config.toml` says and hands back what to
//! change; the window writes it (one line or one table of the file at a
//! time, so the rest stays as written) and reads it again. What the machine
//! says rather than the file (the shells installed, the hooks, starting at
//! sign-in) comes in [`Seen::facts`].

use eframe::egui::{self, Color32, FontId, RichText};
use ito_common::{Clock, CommonChange};
use ito_notify::webhook::Place;
use ito_pane::Palette;
use ito_prefs::{button, field, row, section, select, sep, status, switch, Drafts, Hook, KeyWords, Look, Words};
use tsumugi_mux::settings::{self as cfg, MenuItem, Profile, Settings, TagRule};

use crate::facts::Facts;
use crate::i18n::{tr, tr_desc, trf};
use crate::sort::Sort;
use crate::theme::{Colors, Theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Appearance,
    Keys,
    Notifications,
    Sessions,
    Tags,
    Theme,
    Shell,
    Advanced,
    About,
}

impl Page {
    pub const ALL: [Page; 10] = [
        Page::General,
        Page::Appearance,
        Page::Theme,
        Page::Sessions,
        Page::Tags,
        Page::Shell,
        Page::Keys,
        Page::Notifications,
        Page::Advanced,
        Page::About,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::General => tr("prefs.page.general"),
            Page::Appearance => tr("prefs.page.appearance"),
            Page::Keys => tr("prefs.page.keys"),
            Page::Notifications => tr("prefs.page.notifications"),
            Page::Sessions => tr("prefs.page.sessions"),
            Page::Tags => tr("prefs.page.tags"),
            Page::Theme => tr("prefs.page.theme"),
            Page::Shell => tr("prefs.page.shell"),
            Page::Advanced => tr("prefs.page.advanced"),
            Page::About => more().p_about,
        }
    }

    fn lead(self) -> &'static str {
        match self {
            Page::General => tr("prefs.lead.general"),
            Page::Appearance => tr("prefs.lead.appearance"),
            Page::Keys => tr("prefs.lead.keys"),
            Page::Notifications => tr("prefs.lead.notifications"),
            Page::Sessions => tr("prefs.lead.sessions"),
            Page::Tags => tr("prefs.lead.tags"),
            Page::Theme => tr("prefs.lead.theme"),
            Page::Shell => tr("prefs.lead.shell"),
            Page::Advanced => tr("prefs.lead.advanced"),
            Page::About => more().p_about_lead,
        }
    }
}

/// The rows each page has, for the search field, by their keys in the
/// language tables: a page is listed while the words are in one of its rows.
/// [`index`] adds the rows the shared parts draw. The screen's own test
/// checks that each is drawn on its page.
const INDEX: &[(Page, &str)] = &[
    (Page::General, "prefs.general.on_start"),
    (Page::General, "prefs.general.default_folder"),
    (Page::General, "prefs.general.autostart"),
    (Page::General, "prefs.general.keep_sessions"),
    (Page::General, "prefs.general.ask_close"),
    (Page::General, "prefs.general.check_updates"),
    (Page::General, "prefs.general.restart_after_update"),
    (Page::General, "prefs.general.copy_on_select"),
    (Page::General, "prefs.general.warn_multiline"),
    (Page::General, "prefs.general.warn_large"),
    (Page::Appearance, "prefs.appearance.nerd"),
    (Page::Appearance, "prefs.appearance.titlebar"),
    (Page::Appearance, "prefs.appearance.material"),
    (Page::Appearance, "prefs.appearance.opacity"),
    (Page::Appearance, "prefs.appearance.image"),
    (Page::Appearance, "prefs.appearance.image_strength"),
    (Page::Appearance, "prefs.appearance.quake_key"),
    (Page::Appearance, "prefs.appearance.quake_height"),
    (Page::Appearance, "prefs.appearance.dim"),
    (Page::Appearance, "prefs.appearance.cursor"),
    (Page::Appearance, "prefs.appearance.cursor_glide"),
    (Page::Appearance, "prefs.appearance.animations"),
    (Page::Appearance, "prefs.appearance.visual_bell"),
    (Page::Keys, "prefs.keys.use_cmd"),
    (Page::Keys, "prefs.keys.nth_tab"),
    (Page::Keys, "prefs.keys.move"),
    (Page::Keys, "prefs.keys.resize"),
    (Page::Notifications, "prefs.notify.system"),
    (Page::Notifications, "prefs.notify.taskbar"),
    (Page::Notifications, "prefs.notify.flash"),
    (Page::Notifications, "prefs.notify.sound"),
    (Page::Notifications, "prefs.notify.long_run"),
    (Page::Notifications, "prefs.notify.answers"),
    (Page::Notifications, "prefs.notify.stay"),
    (Page::Notifications, "prefs.notify.webhook_after"),
    (Page::Notifications, "prefs.notify.sound_waiting"),
    (Page::Notifications, "prefs.notify.sound_error"),
    (Page::Notifications, "prefs.notify.focus"),
    (Page::Notifications, "prefs.notify.spend_day"),
    (Page::Notifications, "prefs.notify.spend_block"),
    (Page::Notifications, "prefs.head.quiet_tags"),
    (Page::Sessions, "prefs.sessions.default_program"),
    (Page::Sessions, "prefs.sessions.claude_command"),
    (Page::Sessions, "prefs.sessions.resume"),
    (Page::Sessions, "prefs.sessions.quiet"),
    (Page::Sessions, "prefs.sessions.sort"),
    (Page::Sessions, "prefs.sessions.new_profile"),
    (Page::Sessions, "prefs.open.editor"),
    (Page::Sessions, "prefs.open.kura"),
    (Page::Sessions, "prefs.open.file"),
    (Page::Sessions, "prefs.head.tab_menu"),
    (Page::Tags, "prefs.tags.new_rule"),
    (Page::Tags, "prefs.tags.colour_new"),
    (Page::Tags, "prefs.tags.shown"),
    (Page::Tags, "prefs.tags.edit_tag"),
    (Page::Tags, "prefs.tags.quiet"),
    (Page::Shell, "prefs.shell.claude_hooks"),
    (Page::Shell, "prefs.shell.integration"),
    (Page::Shell, "prefs.shell.lines"),
    (Page::Shell, "prefs.shell.conpty"),
    (Page::Advanced, "prefs.advanced.mux"),
    (Page::Advanced, "prefs.advanced.restart_server"),
    (Page::Advanced, "prefs.advanced.backend"),
    (Page::Advanced, "prefs.advanced.command_there"),
    (Page::Advanced, "prefs.advanced.reached"),
    (Page::Advanced, "prefs.advanced.scrollback"),
    (Page::Advanced, "prefs.advanced.pane_log"),
    (Page::Advanced, "prefs.advanced.secure_input"),
    (Page::Advanced, "prefs.advanced.settings_folder"),
    (Page::Advanced, "prefs.advanced.usage"),
    (Page::Advanced, "prefs.advanced.saved_tabs"),
];

/// The words the search field looks in, page by page: [`INDEX`] in the
/// language in force, the shared parts' rows (the language, the clock, the
/// theme) and the keys' names; in English too, so either finds a row.
pub fn index() -> Vec<(Page, &'static str)> {
    let w = words();
    let keys = crate::keys::NAMED.map(|n| n.0);
    // Rows this system does not draw are not found either.
    let here = |k: &&str| crate::secure::HERE || *k != "prefs.advanced.secure_input";
    let mut out: Vec<(Page, &'static str)> = INDEX.iter().filter(|(_, k)| here(k)).map(|(p, k)| (*p, tr(k))).collect();
    let shared = |w: &'static Words| {
        [
            (Page::General, w.language),
            (Page::General, w.scale),
            (Page::General, w.show_time),
            (Page::General, w.time_format),
            (Page::General, w.show_date),
            (Page::General, w.date_format),
            (Page::General, w.weekday),
            (Page::Theme, w.mode),
            (Page::Theme, w.preview),
            (Page::Notifications, w.notify_webhook),
            (Page::Notifications, w.webhook_add),
        ]
    };
    out.extend(shared(w));
    out.extend(ito_prefs::window_index(more()).into_iter().map(|k| (Page::General, k)));
    out.extend(ito_prefs::fonts_index(more()).into_iter().filter(|k| !ito_prefs::FONT_KEYS.contains(k)).map(|k| (Page::Appearance, k)));
    out.extend(ito_prefs::shell_index(shell_words()).into_iter().filter(|k| !ito_prefs::SHELL_KEYS.contains(k)).map(|k| (Page::Shell, k)));
    out.extend(
        ito_prefs::mono_font_index(more())
            .into_iter()
            .filter(|k| *k != ito_prefs::MONO_FONT_KEY && *k != more().mono_system)
            .map(|k| (Page::Appearance, k)),
    );
    out.extend(ito_prefs::backup_index(more()).into_iter().map(|k| (Page::Advanced, k)));
    // There is no build to show (no commit is known), so its label is not drawn.
    out.extend(ito_prefs::about_index(more(), w).into_iter().filter(|k| *k != more().commit).map(|k| (Page::About, k)));
    out.extend(keys.map(|a| (Page::Keys, tr_desc(crate::keys::title(a)))));
    out.extend(keys_index(key_words()).map(|k| (Page::Keys, k)));
    if crate::i18n::current_language() != "en" {
        out.extend(keys.map(|a| (Page::Keys, crate::keys::title(a))));
        out.extend(INDEX.iter().filter(|(_, k)| here(k)).map(|(p, k)| (*p, crate::i18n::tr_en(k))));
        out.extend(shared(&ito_prefs::EN));
        out.extend(ito_prefs::window_index(&ito_prefs::MORE_EN).into_iter().map(|k| (Page::General, k)));
        out.extend(
            ito_prefs::fonts_index(&ito_prefs::MORE_EN)
                .into_iter()
                .filter(|k| !ito_prefs::FONT_KEYS.contains(k))
                .map(|k| (Page::Appearance, k)),
        );
        out.extend(
            ito_prefs::shell_index(&ito_prefs::SHELL_EN)
                .into_iter()
                .filter(|k| !ito_prefs::SHELL_KEYS.contains(k))
                .map(|k| (Page::Shell, k)),
        );
        let en = &ito_prefs::MORE_EN;
        out.extend(
            ito_prefs::mono_font_index(en)
                .into_iter()
                .filter(|k| *k != ito_prefs::MONO_FONT_KEY && *k != en.mono_system)
                .map(|k| (Page::Appearance, k)),
        );
        out.extend(ito_prefs::backup_index(en).into_iter().map(|k| (Page::Advanced, k)));
        out.extend(ito_prefs::about_index(en, &ito_prefs::EN).into_iter().filter(|k| *k != en.commit).map(|k| (Page::About, k)));
        out.extend(keys_index(&ito_prefs::KEYS_EN).map(|k| (Page::Keys, k)));
    }
    out
}

/// The screen while it is open.
#[derive(Default)]
pub struct Screen {
    pub page: Page,
    /// What the search field above the pages holds.
    pub query: String,
    pub edit: Edit,
    /// A control had the keys at the end of the last frame: an Esc then
    /// leaves it (egui lets go of it before this frame is drawn), and only
    /// the next Esc leaves the screen.
    held: bool,
}

/// What is being changed on the screen and not yet written.
#[derive(Default)]
pub struct Edit {
    /// The Keys page's search, filters and the key being pressed for an
    /// action.
    pub keys: ito_prefs::KeysEdit,
    /// The settings search as the Keys page last saw it: one that names an
    /// operation narrows the table to it.
    keys_search: String,
    /// The text fields' words while they are typed in, and just after.
    drafts: Drafts,
    profile: Option<ProfileDraft>,
    /// The tag rule being written: a new one, or the one at `at` from "Edit".
    rule: RuleDraft,
    /// The Shell card's environment variables being edited.
    env: ito_prefs::ShellEdit,
    /// The tag "Edit a tag" has, and its new name.
    tag: Option<String>,
    rename: String,
    /// A new item of one's own for the tab's menu: its words and command.
    item: (String, String),
    /// The file to import.
    /// The hooks' own lines are shown, to copy.
    lines: bool,
}

#[derive(Default)]
struct RuleDraft {
    at: Option<usize>,
    folder: String,
    branch: String,
    tag: String,
}

struct ProfileDraft {
    /// The name it had; `None` for a new one.
    was: Option<String>,
    name: String,
    folder: String,
    start: String,
    tags: String,
    panes: Vec<String>,
    place: String,
    /// A theme's name; empty for the window's.
    theme: String,
    /// The font size as typed; empty for the window's.
    font_size: String,
}

impl ProfileDraft {
    fn of(p: &Profile) -> Self {
        Self {
            was: Some(p.name.clone()),
            name: p.name.clone(),
            folder: p.folder.clone(),
            start: p.start.clone(),
            tags: p.tags.join(", "),
            panes: p.panes.clone(),
            place: p.place.clone(),
            theme: p.theme.clone(),
            font_size: p.font_size.map(|s| s.to_string()).unwrap_or_default(),
        }
    }

    fn profile(&self) -> Profile {
        let tags = self.tags.split(',').filter_map(tsumugi_mux::proto::tag_name).collect();
        Profile {
            name: self.name.trim().to_owned(),
            folder: self.folder.trim().to_owned(),
            start: self.start.clone(),
            tags,
            panes: self.panes.clone(),
            place: self.place.trim().to_owned(),
            theme: self.theme.clone(),
            font_size: self.size(),
        }
    }

    /// The font size typed, if it is one (6 to 72 points).
    fn size(&self) -> Option<f32> {
        self.font_size.trim().trim_end_matches("pt").trim().parse::<f32>().ok().filter(|s| (6.0..=72.0).contains(s))
    }
}

/// What the screen was asked to change.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// One key of `config.toml`: its table (`None` at the top), its name
    /// and its value as TOML.
    Set(Option<&'static str>, &'static str, String),
    /// The Keys page's changes, for keymap.toml.
    Keys(Vec<ito_keymap::Edit>),
    /// One key of a table named at run time (`[tags.colors]`, `[shell.env]`),
    /// the key as TOML writes it; `None` takes it out.
    SetIn(String, String, Option<String>),
    /// One key of common.toml, which kura and yagura read too: the
    /// language, the theme and the clock.
    Common(CommonChange),
    /// Every `[[tags.rule]]`, in place of those there.
    Rules(Vec<TagRule>),
    /// Every `[[menu.session]]`, in place of those there.
    MenuItems(Vec<MenuItem>),
    MuteTag(String, bool),
    RenameTag(String, String),
    TestNotification,
    /// The Feedback card's button: the bug report, as F12 opens it.
    BugReport,
    /// The Window card's "forget the saved place".
    ForgetPlace,
    /// Every `[[notify.webhooks]]`, in place of those there and of the one
    /// address of before (`[notify] webhook`).
    Webhooks(Vec<Place>),
    /// "Send a test here": that webhook only.
    TestWebhook(Place),
    PlaySound(String),
    /// A profile, in place of the one with the name it had (`None`: new).
    SaveProfile(Option<String>, Profile),
    DeleteProfile(String),
    Sort(Sort),
    AlwaysRestore(bool),
    Autostart(bool),
    /// Open with the system: the settings folder, or the file.
    OpenFolder,
    OpenFile,
    Copy(String),
    /// Put Claude Code's hooks in its settings, or take them out.
    Hooks(bool),
    /// Put the shell integration in the shell's profile, or take it out.
    ShellHook(String, bool),
    RestartServer,
    /// Another page of the screen (the screen does this itself).
    GoTo(Page),
    Backup(ito_prefs::Backup),
    Close,
}

/// What the screen shows.
pub struct Seen<'a> {
    pub settings: &'a Settings,
    pub themes: &'a [Theme],
    /// The theme in force.
    pub current: Colors,
    pub profiles: &'a [Profile],
    /// The tags of the sessions there are, and those quieted.
    pub tags: &'a [String],
    pub muted_tags: &'a [String],
    pub sort: Sort,
    pub always_restore: bool,
    /// A Nerd Font is installed.
    pub nerd: bool,
    /// The monospace fonts installed, the file the panes are in, and which
    /// of bold, italic and bold italic were found beside it.
    pub font_names: &'a [String],
    pub font_file: Option<String>,
    pub faces: [bool; 3],
    pub server_up: String,
    /// Where the server listens.
    pub address: String,
    pub settings_path: String,
    pub state_path: String,
    /// `None` while they are being read.
    pub facts: Option<&'a Facts>,
    /// `language` in the common.toml uchmk's apps share (`auto` when unset).
    pub language: &'a str,
    /// `scale` in common.toml: the whole window's size (1.0 when unset).
    pub scale: f32,
    /// `mono_font` in common.toml.
    pub mono: ito_common::MonoFont,
    /// `[window]` in common.toml.
    pub window: ito_common::WindowPrefs,
    /// The theme as common.toml picks it (`theme`, `dark_theme`,
    /// `light_theme`), and its clock.
    pub choice: (&'a str, &'a str, &'a str),
    pub clock: &'a Clock,
    /// The window's keys as keymap.toml last read ([`crate::keys::Keymap::layers`]).
    pub keymap: &'a [(String, Vec<ito_keymap::Binding<Option<crate::keys::Action>>>)],
}

/// The shared parts' words (the search, the language, the clock, the
/// theme) in the language tsumugi's menus are in.
fn words() -> &'static Words {
    Words::of(crate::i18n::current_language())
}

/// The Keys page's words, likewise.
/// The Keys page's words the search may find it by: only those always on
/// it (with one layer there is no layer filter, and the rest are tooltips
/// or come only while a key is being taken).
fn keys_index(w: &KeyWords) -> [&'static str; 2] {
    [w.by_key, w.changed_only]
}

/// The words of the shared Window card and About page, likewise.
fn more() -> &'static ito_prefs::MoreWords {
    ito_prefs::MoreWords::of(crate::i18n::current_language())
}

fn shell_words() -> &'static ito_prefs::ShellWords {
    ito_prefs::ShellWords::of(crate::i18n::current_language())
}

fn key_words() -> &'static KeyWords {
    KeyWords::of(crate::i18n::current_language())
}

pub fn show(ui: &mut egui::Ui, pal: &Palette, screen: &mut Screen, seen: &Seen) -> Vec<Change> {
    let mut out = Vec::new();
    let pages: Vec<(&str, &str)> = Page::ALL.iter().map(|p| (p.title(), p.lead())).collect();
    let index: Vec<(usize, &str)> =
        index().into_iter().map(|(p, words)| (Page::ALL.iter().position(|q| *q == p).unwrap_or(0), words)).collect();
    let nav = ito_prefs::Nav { pages: &pages, index: &index, words: words(), file: "config.toml" };
    let page = Page::ALL.iter().position(|p| *p == screen.page).unwrap_or(0);
    let mut state = ito_prefs::State { page, query: std::mem::take(&mut screen.query), held: screen.held };
    // Esc leaves the screen, unless a control or the Keys page (a key being
    // pressed for an action, or looked for) has it.
    // While a key is being taken, every key goes to the Keys page: Ctrl+Tab
    // or Ctrl+F there are keys being given, not the screen's.
    let pressed = if screen.edit.keys.busy() { ito_prefs::Keys::default() } else { ito_prefs::Keys::standard(ui, state.busy(ui)) };
    let edit = &mut screen.edit;
    let done = ito_prefs::show(ui, seen.current, &mut state, &nav, pressed, |ui, page, l| {
        match Page::ALL[page] {
            Page::General => general(ui, l, seen, edit, &mut out),
            Page::Appearance => appearance(ui, l, seen, edit, &mut out),
            Page::Keys => keys(ui, l, seen, edit, &mut out),
            Page::Notifications => notifications(ui, l, seen, edit, &mut out),
            Page::Sessions => sessions(ui, l, seen, edit, &mut out),
            Page::Tags => tags(ui, l, seen, edit, &mut out),
            Page::Theme => theme(ui, l, pal, seen, &mut out),
            Page::Shell => shell(ui, l, seen, edit, &mut out),
            Page::Advanced => advanced(ui, l, seen, edit, &mut out),
            Page::About => about(ui, l, &mut out),
        }
        follow_focus(ui);
    });
    (screen.page, screen.query, screen.held) = (Page::ALL[state.page], state.query, state.held);
    if screen.page == Page::Keys {
        follow_search(&mut screen.edit, &screen.query);
    }
    // Keys taken on the Keys page stop with it: left to another page by a
    // click, they would keep Esc and Ctrl+Tab from the screen.
    if screen.page != Page::Keys {
        screen.edit.keys.capture = None;
        screen.edit.keys.listening = false;
    }
    if done.close {
        out.insert(0, Change::Close);
    }
    if done.open_file {
        out.push(Change::OpenFile);
    }
    out.retain(|change| match change {
        Change::GoTo(p) => {
            screen.page = *p;
            false
        }
        _ => true,
    });
    out
}

/// A table key as TOML writes it: bare when it can be.
fn toml_key(name: &str) -> String {
    if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        name.to_owned()
    } else {
        cfg::quote(name)
    }
}

fn general(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let g = &seen.settings.general;
    let set = |key: &'static str, v: bool| Change::Set(Some("general"), key, v.to_string());
    section(ui, l, tr("prefs.head.startup"), |ui| {
        let note = tr("prefs.general.language_note");
        out.extend(ito_prefs::language_row(ui, l, words(), seen.language, note).map(Change::Common));
        sep(ui, l);
        let note = tr("prefs.general.scale_note");
        out.extend(ito_prefs::scale_row(ui, l, words(), seen.scale, note).map(Change::Common));
        sep(ui, l);
        let starts = [(true, tr("prefs.general.restore")), (false, tr("prefs.general.ask"))];
        if let Some(on) = row(ui, l, tr("prefs.general.on_start"), tr("prefs.general.on_start_note"), |ui| {
            select(ui, "on-start", seen.always_restore, &starts)
        }) {
            out.push(Change::AlwaysRestore(on));
        }
        sep(ui, l);
        let home = if cfg!(windows) { "%USERPROFILE%" } else { "~" };
        if let Some(t) = row(ui, l, tr("prefs.general.default_folder"), tr("prefs.general.default_folder_note"), |ui| {
            field(ui, &mut edit.drafts, "default-folder", &g.default_folder, home, 200.0)
        }) {
            out.push(Change::Set(Some("general"), "default_folder", cfg::quote(t.trim())));
        }
        sep(ui, l);
        let on = seen.facts.map(|f| f.autostart);
        if row(ui, l, tr("prefs.general.autostart"), tr("prefs.general.autostart_note"), |ui| match on {
            Some(on) => switch(ui, l, on),
            None => {
                status(ui, "…", l.c.dim);
                false
            }
        }) {
            out.push(Change::Autostart(!on.unwrap_or(false)));
        }
    });
    let window = ito_prefs::window_card(ui, l, more(), &seen.window);
    out.extend(window.changes.into_iter().map(Change::Common));
    if window.forget {
        out.push(Change::ForgetPlace);
    }
    section(ui, l, tr("prefs.head.closing"), |ui| {
        if row(ui, l, tr("prefs.general.keep_sessions"), tr("prefs.general.keep_note"), |ui| switch(ui, l, g.keep_sessions)) {
            out.push(set("keep_sessions", !g.keep_sessions));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.general.ask_close"), tr("prefs.general.ask_close_note"), |ui| switch(ui, l, g.ask_before_close)) {
            out.push(set("ask_before_close", !g.ask_before_close));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.general.check_updates"), tr("prefs.general.updates_note"), |ui| switch(ui, l, g.check_updates)) {
            out.push(set("check_updates", !g.check_updates));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.general.restart_after_update"), tr("prefs.general.restart_note"), |ui| {
            switch(ui, l, g.restart_after_update)
        }) {
            out.push(set("restart_after_update", !g.restart_after_update));
        }
    });
    section(ui, l, tr("prefs.head.copy_paste"), |ui| {
        let copy_key = if cfg!(target_os = "macos") { tr("prefs.general.copy_cmd") } else { tr("prefs.general.copy_ctrl") };
        if row(ui, l, tr("prefs.general.copy_on_select"), copy_key, |ui| switch(ui, l, g.copy_on_select)) {
            out.push(set("copy_on_select", !g.copy_on_select));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.general.warn_multiline"), tr("prefs.general.multiline_note"), |ui| switch(ui, l, g.warn_multiline_paste)) {
            out.push(set("warn_multiline_paste", !g.warn_multiline_paste));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.general.warn_large"), tr("prefs.general.large_note"), |ui| switch(ui, l, g.warn_large_paste)) {
            out.push(set("warn_large_paste", !g.warn_large_paste));
        }
    });
    out.extend(ito_prefs::clock_card(ui, l, words(), seen.clock).into_iter().map(Change::Common));
}

fn appearance(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let f = &seen.settings.font;
    let a = &seen.settings.appearance;
    let using = match &seen.font_file {
        Some(file) => {
            let styles: Vec<&str> = [tr("prefs.appearance.bold"), tr("prefs.appearance.italic"), tr("prefs.appearance.bold_italic")]
                .into_iter()
                .zip(seen.faces)
                .filter(|(_, on)| *on)
                .map(|(s, _)| s)
                .collect();
            let styles = if styles.is_empty() {
                tr("prefs.appearance.no_styles").to_owned()
            } else {
                trf("prefs.appearance.with", &[&styles.join(tr("prefs.common.list_sep"))])
            };
            format!("{file} ({styles})")
        }
        None if !f.family.is_empty() => trf("prefs.appearance.not_found", &[&f.family]),
        None => tr("prefs.appearance.builtin").to_owned(),
    };
    let now = ito_prefs::Fonts {
        family: &f.family,
        families: seen.font_names,
        size: f.size,
        line_height: Some(f.line_height),
        ligatures: Some(f.ligatures),
        using: &using,
        size_range: (8.0, 32.0),
    };
    for ch in ito_prefs::fonts_card(ui, l, more(), &mut edit.drafts, &now) {
        out.push(match ch {
            ito_prefs::FontChange::Family(v) => Change::Set(Some("font"), "family", cfg::quote(&v)),
            ito_prefs::FontChange::Size(v) => Change::Set(Some("font"), "size", v.to_string()),
            ito_prefs::FontChange::LineHeight(v) => Change::Set(Some("font"), "line_height", format!("{v:.2}")),
            ito_prefs::FontChange::Ligatures(v) => Change::Set(Some("font"), "ligatures", v.to_string()),
        });
    }
    out.extend(ito_prefs::mono_font_card(ui, l, more(), seen.mono).map(Change::Common));
    section(ui, l, tr("prefs.appearance.nerd"), |ui| {
        let found = if seen.nerd { tr("prefs.appearance.nerd_found") } else { tr("prefs.appearance.nerd_missing") };
        if row(ui, l, tr("prefs.appearance.nerd"), &trf("prefs.appearance.nerd_note", &[found]), |ui| switch(ui, l, a.nerd_icons)) {
            out.push(Change::Set(Some("appearance"), "nerd_icons", (!a.nerd_icons).to_string()));
        }
    });
    let w = &seen.settings.window;
    section(ui, l, tr("prefs.head.window"), |ui| {
        let own = w.own_titlebar();
        let note = if cfg!(target_os = "macos") { tr("prefs.appearance.title_mac") } else { tr("prefs.appearance.title_other") };
        if row(ui, l, tr("prefs.appearance.titlebar"), note, |ui| switch(ui, l, own)) {
            out.push(Change::Set(Some("window"), "titlebar", cfg::quote(if own { "system" } else { "tsumugi" })));
        }
        if cfg!(windows) || cfg!(target_os = "macos") {
            sep(ui, l);
            let (note, kinds): (&str, &[(&str, &str)]) = if cfg!(windows) {
                (tr("prefs.appearance.mica_note"), &[("none", tr("prefs.common.none")), ("mica", "Mica"), ("acrylic", "Acrylic")])
            } else {
                (tr("prefs.appearance.vibrancy_note"), &[("none", tr("prefs.common.none")), ("vibrancy", "Vibrancy")])
            };
            if let Some(m) = row(ui, l, tr("prefs.appearance.material"), note, |ui| select(ui, "material", w.material.as_str(), kinds)) {
                out.push(Change::Set(Some("window"), "material", cfg::quote(m)));
            }
        }
        sep(ui, l);
        let note = tr("prefs.appearance.opacity_note");
        if let Some(t) = row(ui, l, tr("prefs.appearance.opacity"), note, |ui| {
            field(ui, &mut edit.drafts, "opacity", &w.opacity.to_string(), "100", 80.0)
        }) {
            if let Ok(v) = t.trim().trim_end_matches('%').trim().parse::<u8>() {
                out.push(Change::Set(Some("window"), "opacity", v.clamp(20, 100).to_string()));
            }
        }
        sep(ui, l);
        let note = tr("prefs.appearance.image_note");
        if let Some(t) =
            row(ui, l, tr("prefs.appearance.image"), note, |ui| field(ui, &mut edit.drafts, "image", &w.image, "~/Pictures/bg.png", 220.0))
        {
            out.push(Change::Set(Some("window"), "image", cfg::quote(t.trim())));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.appearance.image_strength"), tr("prefs.appearance.image_strength_note"), |ui| {
            field(ui, &mut edit.drafts, "image_opacity", &w.image_opacity.to_string(), "25", 80.0)
        }) {
            if let Ok(v) = t.trim().trim_end_matches('%').trim().parse::<u8>() {
                out.push(Change::Set(Some("window"), "image_opacity", v.min(100).to_string()));
            }
        }
        sep(ui, l);
        let note = tr("prefs.appearance.quake_note");
        if let Some(t) =
            row(ui, l, tr("prefs.appearance.quake_key"), note, |ui| field(ui, &mut edit.drafts, "quake", &w.quake, "Ctrl+`", 140.0))
        {
            out.push(Change::Set(Some("window"), "quake", cfg::quote(t.trim())));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.appearance.quake_height"), tr("prefs.appearance.quake_height_note"), |ui| {
            field(ui, &mut edit.drafts, "quake_height", &w.quake_height.to_string(), "50", 80.0)
        }) {
            if let Ok(v) = t.trim().trim_end_matches('%').trim().parse::<u8>() {
                out.push(Change::Set(Some("window"), "quake_height", v.clamp(20, 100).to_string()));
            }
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.appearance.dim"), tr("prefs.appearance.dim_note"), |ui| {
            field(ui, &mut edit.drafts, "dim", &a.dim.to_string(), "35", 80.0)
        }) {
            if let Ok(v) = t.trim().trim_end_matches('%').trim().parse::<u8>() {
                out.push(Change::Set(Some("appearance"), "dim", v.min(90).to_string()));
            }
        }
        sep(ui, l);
        let shapes = [
            ("block-blink", tr("prefs.appearance.block_blink")),
            ("block", tr("prefs.appearance.block")),
            ("bar-blink", tr("prefs.appearance.bar_blink")),
            ("bar", tr("prefs.appearance.bar")),
            ("underline-blink", tr("prefs.appearance.underline_blink")),
            ("underline", tr("prefs.appearance.underline")),
        ];
        if let Some(s) = row(ui, l, tr("prefs.appearance.cursor"), tr("prefs.appearance.cursor_note"), |ui| {
            select(ui, "cursor", a.cursor.as_str(), &shapes)
        }) {
            out.push(Change::Set(Some("appearance"), "cursor", cfg::quote(s)));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.appearance.cursor_glide"), tr("prefs.appearance.cursor_glide_note"), |ui| switch(ui, l, a.cursor_glide)) {
            out.push(Change::Set(Some("appearance"), "cursor_glide", (!a.cursor_glide).to_string()));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.appearance.animations"), tr("prefs.appearance.animations_note"), |ui| switch(ui, l, a.animations)) {
            out.push(Change::Set(Some("appearance"), "animations", (!a.animations).to_string()));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.appearance.visual_bell"), tr("prefs.appearance.visual_bell_note"), |ui| switch(ui, l, a.visual_bell)) {
            out.push(Change::Set(Some("appearance"), "visual_bell", (!a.visual_bell).to_string()));
        }
    });
}

/// The settings search found the Keys page by an operation (its rows are
/// many, the one looked for may be far down): the table is narrowed to it,
/// and opened up again when the search goes. What is typed in the table's
/// own search is left alone.
fn follow_search(edit: &mut Edit, query: &str) {
    if edit.keys_search == query {
        return;
    }
    let q = query.trim().to_lowercase();
    let names = |n: &&(crate::keys::Action, &str, &str, &str)| {
        let title = crate::keys::title(n.0);
        n.1.contains(&q) || tr_desc(title).to_lowercase().contains(&q) || title.to_lowercase().contains(&q)
    };
    if edit.keys.query.is_empty() || edit.keys.query == edit.keys_search.trim() {
        if !q.is_empty() && crate::keys::NAMED.iter().any(|n| names(&n)) {
            edit.keys.query = query.trim().to_owned();
        } else {
            edit.keys.query.clear();
        }
    }
    edit.keys_search = query.to_owned();
}

fn keys(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    use crate::keys::{title, Chord, NAMED};
    let c = l.c;
    let mac = crate::keys::mac();
    section(ui, l, tr("prefs.head.mac"), |ui| {
        let on = seen.settings.general.cmd_on_mac;
        let note = if cfg!(target_os = "macos") { tr("prefs.keys.cmd_note") } else { tr("prefs.keys.cmd_note_other") };
        if row(ui, l, tr("prefs.keys.use_cmd"), note, |ui| switch(ui, l, on)) {
            out.push(Change::Set(Some("general"), "cmd_on_mac", (!on).to_string()));
        }
    });
    let fixed = |win: &str, m: &str| (if mac { m } else { win }).replace("{0}", tr("prefs.keys.arrows"));
    let fixed_row = |ui: &mut egui::Ui, title: &str, key: &str| {
        row(ui, l, title, "", |ui| {
            ui.label(RichText::new(key).font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.0))).color(c.dim));
        });
    };
    // Above the table, so they are seen without scrolling past it.
    section(ui, l, tr("prefs.head.fixed"), |ui| {
        fixed_row(ui, tr("prefs.keys.nth_tab"), &fixed("Ctrl+Alt+1 … 9", "Cmd+1 … 9"));
        sep(ui, l);
        fixed_row(ui, tr("prefs.keys.move"), &fixed("Alt+{0}", "Cmd+Option+{0}"));
        sep(ui, l);
        fixed_row(ui, tr("prefs.keys.resize"), &fixed("Alt+Shift+{0}", "Cmd+Ctrl+{0}"));
    });
    // The actions with no key of their own have a row too, to give one.
    let unbound: Vec<(&str, &str, &str)> =
        NAMED.iter().filter(|n| (if mac { n.3 } else { n.2 }).is_empty()).map(|n| ("window", n.1, title(n.0))).collect();
    let layer_name = |_: &str| tr("prefs.keys.window").to_owned();
    let tr_key = |d: &str| tr_desc(d).to_owned();
    // A key Claude Code or the shells use is said, as the window takes it
    // from them.
    let other = |_: &str, on: &[ito_keys::Key]| -> Vec<String> {
        let label = on.first().and_then(|k| Chord::from_notation(k).ok()).map(|c| c.label()).unwrap_or_default();
        crate::keys::other(on).map(|whose| trf("prefs.keys.clash_other", &[&label, tr_desc(whose)])).into_iter().collect()
    };
    let table = ito_prefs::KeyTable {
        now: seen.keymap,
        built_in: crate::keys::built_in(mac),
        noop: "none",
        own: "keymap.toml",
        layer_name: &layer_name,
        tr: &tr_key,
        extra: &other,
        unbound: &unbound,
        sup: cfg!(target_os = "macos"),
    };
    if let Some(edits) = ito_prefs::keys_page(ui, l, key_words(), &table, &mut edit.keys) {
        out.push(Change::Keys(edits));
    }
    ui.label(RichText::new(tr("prefs.keys.footer")).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(c.dim));
}

/// The sounds by what the list says.
/// A span of time as the lists say it: `90 s`, `5 min`, `5 min or more`.
fn span(secs: u64, more: bool) -> String {
    let (key, n) = match (secs >= 60 && secs.is_multiple_of(60), more) {
        (true, true) => ("prefs.notify.mins_or_more", secs / 60),
        (true, false) => ("prefs.notify.mins", secs / 60),
        (false, true) => ("prefs.notify.secs_or_more", secs),
        (false, false) => ("prefs.notify.secs", secs),
    };
    trf(key, &[&n.to_string()])
}

fn sound_names() -> [(&'static str, &'static str); 4] {
    [
        ("chime", tr("prefs.notify.chime")),
        ("low", tr("prefs.notify.low")),
        ("alert", tr("prefs.notify.alert")),
        ("default", tr("prefs.notify.default")),
    ]
}

/// The states a webhook may be told of, in the table's order.
const STATES: [&str; 3] = ["waiting", "error", "done"];

fn notifications(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let n = &seen.settings.notify;
    // The webhooks, the one address of before first: a row each in the
    // table, and their addresses and formats under "Away".
    let mut hooks: Vec<Hook> = n.places().iter().map(|p| Hook::of(p, &STATES, true)).collect();
    let was = hooks.clone();
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            reading_order(ui, egui::Id::new("notify-buttons"), |ui| {
                if button(ui, l, tr("prefs.notify.reset")) {
                    let d = cfg::Notify::default();
                    for (key, list) in [("system", d.system), ("taskbar", d.taskbar), ("flash", d.flash), ("sound", d.sound)] {
                        out.push(Change::Set(Some("notify"), key, cfg::quote_list(&list)));
                    }
                }
                if button(ui, l, tr("prefs.notify.test")) {
                    out.push(Change::TestNotification);
                }
            });
        });
    });
    ui.add_space(8.0);
    section(ui, l, tr("prefs.head.when"), |ui| {
        // The design's table: the way to tell, then a column per state, the
        // label's column 1.6 times as wide as each state's, a line between
        // rows.
        let states = [
            ("waiting", tr("prefs.notify.waits"), c.wait),
            ("error", tr("prefs.notify.fails"), c.err),
            ("done", tr("prefs.notify.finishes"), c.done),
        ];
        let gap = 16.0;
        let width = ui.available_width();
        let unit = (width - 3.0 * gap) / 4.6;
        // Each row's room laid out by hand, so every column has its share
        // and everything sits in the middle of its row.
        let columns = |row: egui::Rect| -> [egui::Rect; 4] {
            let mut x = row.left();
            let mut take = |w: f32| {
                let r = egui::Rect::from_min_max(egui::pos2(x, row.top()), egui::pos2(x + w, row.bottom()));
                x += w + gap;
                r
            };
            [take(unit * 1.6), take(unit), take(unit), take(unit)]
        };
        let (head, _) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::hover());
        for ((_, words, color), r) in states.iter().zip(&columns(head)[1..]) {
            let zoomed_size = ito_theme::zoomed(ui.ctx(), 13.0);
            let text = ui.fonts_mut(|f| f.layout_no_wrap(words.to_string(), FontId::proportional(zoomed_size), crate::chrome::ink(*color)));
            let left = r.center().x - (text.size().x + 14.0) / 2.0;
            ui.painter().circle_filled(egui::pos2(left + 4.0, r.center().y), 4.0, *color);
            ui.painter().galley(egui::pos2(left + 14.0, r.center().y - text.size().y / 2.0), text, *color);
        }
        // A way's row: its words, then a switch per state; the state whose
        // switch was flipped.
        let line = |ui: &mut egui::Ui, label: &str, note: &str, on: [bool; 3]| -> Option<usize> {
            sep(ui, l);
            // A line's height of room round the two lines of words, so the
            // note does not touch the rule under it (seen on Windows,
            // 2026-10-07).
            let (row, _) = ui.allocate_exact_size(egui::vec2(width, 66.0), egui::Sense::hover());
            let cols = columns(row);
            let words = egui::Rect::from_center_size(cols[0].center(), egui::vec2(cols[0].width(), 40.0));
            // Children of their own: a scope would move the card's cursor
            // to their bottom and take back the row's room.
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(words));
            inner.spacing_mut().item_spacing.y = 2.0;
            inner.label(RichText::new(label).size(ito_theme::zoomed(ui.ctx(), 13.0)).color(c.strong()));
            inner.add(egui::Label::new(RichText::new(note).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(c.dim)).truncate());
            let mut flipped = None;
            for (k, r) in cols[1..].iter().enumerate() {
                let at = egui::Rect::from_center_size(r.center(), egui::vec2(40.0, 22.0));
                if switch(&mut ui.new_child(egui::UiBuilder::new().max_rect(at)), l, on[k]) {
                    flipped = Some(k);
                }
            }
            flipped
        };
        for (key, label, note, list) in [
            ("system", tr("prefs.notify.system"), tr("prefs.notify.system_note"), &n.system),
            ("taskbar", tr("prefs.notify.taskbar"), tr("prefs.notify.taskbar_note"), &n.taskbar),
            ("flash", tr("prefs.notify.flash"), tr("prefs.notify.flash_note"), &n.flash),
            ("sound", tr("prefs.notify.sound"), tr("prefs.notify.sound_note"), &n.sound),
        ] {
            let on = STATES.map(|word| list.iter().any(|w| w == word));
            if let Some(k) = line(ui, label, note, on) {
                let mut next: Vec<String> = list.iter().filter(|w| *w != STATES[k]).cloned().collect();
                if !on[k] {
                    next.push(STATES[k].to_string());
                }
                out.push(Change::Set(Some("notify"), key, cfg::quote_list(&next)));
            }
        }
        // A row per webhook, by its address.
        let count = hooks.len();
        for (k, h) in hooks.iter_mut().enumerate() {
            let note = if h.url.is_empty() { words().webhook_empty } else { h.url.as_str() };
            let on = [0, 1, 2].map(|s| h.on.get(s).copied().unwrap_or(false));
            if let Some(s) = line(ui, &ito_prefs::hook_label(words(), count, k), note, on) {
                h.on = on.to_vec();
                h.on[s] = !on[s];
            }
        }
    });
    section(ui, l, tr("prefs.head.details"), |ui| {
        let mut lengths: Vec<(u64, String)> =
            std::iter::once((0, tr("prefs.notify.any_length").to_owned())).chain([30, 60, 300, 600].map(|v| (v, span(v, true)))).collect();
        if !lengths.iter().any(|(v, _)| *v == n.long_run) {
            lengths.push((n.long_run, span(n.long_run, true)));
        }
        let lengths: Vec<(u64, &str)> = lengths.iter().map(|(v, t)| (*v, t.as_str())).collect();
        if let Some(v) =
            row(ui, l, tr("prefs.notify.long_run"), tr("prefs.notify.long_run_note"), |ui| select(ui, "long-run", n.long_run, &lengths))
        {
            out.push(Change::Set(Some("notify"), "long_run", v.to_string()));
        }
        for (key, label, now) in [
            ("sound_waiting", tr("prefs.notify.sound_waiting"), &n.sound_waiting),
            ("sound_error", tr("prefs.notify.sound_error"), &n.sound_error),
        ] {
            sep(ui, l);
            let picked = row(ui, l, label, "", |ui| {
                crate::chrome::in_order(ui, |ui| {
                    let picked = select(ui, key, now.as_str(), &sound_names());
                    if ui.small_button("▶").on_hover_text(tr("prefs.notify.play")).clicked() {
                        out.push(Change::PlaySound(now.clone()));
                    }
                    picked
                })
            });
            if let Some(s) = picked {
                out.push(Change::Set(Some("notify"), key, cfg::quote(s)));
            }
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.notify.answers"), tr("prefs.notify.answers_note"), |ui| switch(ui, l, n.answer_buttons)) {
            out.push(Change::Set(Some("notify"), "answer_buttons", (!n.answer_buttons).to_string()));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.notify.stay"), tr("prefs.notify.stay_note"), |ui| switch(ui, l, n.stay)) {
            out.push(Change::Set(Some("notify"), "stay", (!n.stay).to_string()));
        }
        sep(ui, l);
        let note = if cfg!(windows) { tr("prefs.notify.focus_note_win") } else { tr("prefs.notify.focus_note_other") };
        if row(ui, l, tr("prefs.notify.focus"), note, |ui| switch(ui, l, n.focus_mode)) {
            out.push(Change::Set(Some("notify"), "focus_mode", (!n.focus_mode).to_string()));
        }
    });
    section(ui, l, tr("prefs.head.away"), |ui| {
        let test = ito_prefs::webhook_rows(ui, l, words(), &mut hooks, STATES.len(), &mut edit.drafts);
        if let Some(h) = test.and_then(|k| hooks.get(k)) {
            out.push(Change::TestWebhook(h.place(&STATES)));
        }
        sep(ui, l);
        let mut afters: Vec<(u64, String)> = [30, 60, 120, 300, 900].map(|v| (v, span(v, false))).into_iter().collect();
        if !afters.iter().any(|(v, _)| *v == n.webhook_after) {
            afters.push((n.webhook_after, span(n.webhook_after, false)));
        }
        let afters: Vec<(u64, &str)> = afters.iter().map(|(v, t)| (*v, t.as_str())).collect();
        if let Some(v) = row(ui, l, tr("prefs.notify.webhook_after"), tr("prefs.notify.webhook_after_note"), |ui| {
            select(ui, "webhook-after", n.webhook_after, &afters)
        }) {
            out.push(Change::Set(Some("notify"), "webhook_after", v.to_string()));
        }
    });
    if hooks != was {
        out.push(Change::Webhooks(hooks.iter().map(|h| h.place(&STATES)).collect()));
    }
    section(ui, l, tr("prefs.head.costs"), |ui| {
        // Once a day, once a 5-hour block, at the estimate (API prices).
        let pick = |now: u64, lines: &[u64]| -> Vec<(u64, String)> {
            let mut v: Vec<(u64, String)> = std::iter::once((0, tr("prefs.notify.never").to_owned()))
                .chain(lines.iter().map(|d| (*d, trf("prefs.notify.past", &[&format!("${d}")]))))
                .collect();
            if !v.iter().any(|(d, _)| *d == now) {
                v.push((now, trf("prefs.notify.past", &[&format!("${now}")])));
            }
            v
        };
        let days = pick(n.spend_day, &[5, 10, 20, 50, 100, 200]);
        let days: Vec<(u64, &str)> = days.iter().map(|(v, t)| (*v, t.as_str())).collect();
        if let Some(v) =
            row(ui, l, tr("prefs.notify.spend_day"), tr("prefs.notify.spend_day_note"), |ui| select(ui, "spend-day", n.spend_day, &days))
        {
            out.push(Change::Set(Some("notify"), "spend_day", v.to_string()));
        }
        sep(ui, l);
        let blocks = pick(n.spend_block, &[5, 10, 20, 50]);
        let blocks: Vec<(u64, &str)> = blocks.iter().map(|(v, t)| (*v, t.as_str())).collect();
        if let Some(v) = row(ui, l, tr("prefs.notify.spend_block"), tr("prefs.notify.spend_block_note"), |ui| {
            select(ui, "spend-block", n.spend_block, &blocks)
        }) {
            out.push(Change::Set(Some("notify"), "spend_block", v.to_string()));
        }
    });
    section(ui, l, tr("prefs.head.quiet_tags"), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("prefs.notify.quiet_line")).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(c.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.link(tr("prefs.notify.edit_tags")).clicked() {
                    out.push(Change::GoTo(Page::Tags));
                }
            });
        });
        ui.add_space(6.0);
        if seen.tags.is_empty() {
            ui.label(RichText::new(tr("prefs.notify.no_tags")).color(c.dim));
        }
        chips(ui, seen.tags, seen.muted_tags, |t, quiet| out.push(Change::MuteTag(t, !quiet)));
        ui.add_space(8.0);
    });
}

/// The tags as chips, a quiet one struck through; `click` with a tag and
/// whether it is quiet.
fn chips(ui: &mut egui::Ui, tags: &[String], quiet: &[String], mut click: impl FnMut(String, bool)) {
    let c = crate::theme::colors();
    ui.horizontal_wrapped(|ui| {
        for t in tags {
            let q = quiet.contains(t);
            let zoomed_size = ito_theme::zoomed(ui.ctx(), 11.0);
            let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(zoomed_size), c.fg).size().x) + 12.0;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::click());
            let chip = crate::chrome::tag_chip(ui.painter(), r.min, t, q);
            if q {
                ui.painter().line_segment([chip.left_center(), chip.right_center()], egui::Stroke::new(1.0, c.dim));
            }
            if resp.on_hover_text(if q { tr("prefs.notify.chip_quiet") } else { tr("prefs.notify.chip_click") }).clicked() {
                click(t.clone(), q);
            }
        }
    });
}

fn starts() -> [(&'static str, &'static str); 3] {
    [("claude", "Claude Code"), ("resume", tr("prefs.sessions.resume_last")), ("shell", tr("prefs.sessions.shell"))]
}

fn profile_words(p: &Profile) -> String {
    let short = |s: &str| match s {
        "resume" => tr("prefs.sessions.resume_short"),
        "shell" => tr("prefs.sessions.shell"),
        _ => "Claude Code",
    };
    let mut kinds = vec![short(&p.start)];
    kinds.extend(p.panes.iter().map(|s| short(s)));
    let mut s = kinds.join(" + ");
    if !p.panes.is_empty() {
        s.push_str(tr("prefs.sessions.split"));
    }
    s.push_str(&format!(" · {}", p.folder));
    if !p.place.is_empty() {
        s.push_str(&format!(" · {}", trf("prefs.sessions.on", &[&p.place])));
    }
    if !p.theme.is_empty() {
        s.push_str(&format!(" · {}", p.theme));
    }
    if let Some(size) = p.font_size {
        s.push_str(&format!(" · {size}pt"));
    }
    if !p.tags.is_empty() {
        s.push_str(&format!(" · {}", p.tags.join(tr("prefs.common.list_sep"))));
    }
    s
}

fn sessions(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let s = &seen.settings.sessions;
    section(ui, l, tr("prefs.head.new_sessions"), |ui| {
        if let Some(v) = row(ui, l, tr("prefs.sessions.default_program"), tr("prefs.sessions.default_program_note"), |ui| {
            select(ui, "start", s.start.as_str(), &starts())
        }) {
            out.push(Change::Set(Some("sessions"), "start", cfg::quote(v)));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.sessions.claude_command"), tr("prefs.sessions.claude_command_note"), |ui| {
            field(ui, &mut edit.drafts, "claude", &s.claude, "claude", 200.0)
        }) {
            let t = t.trim();
            out.push(Change::Set(Some("sessions"), "claude", cfg::quote(if t.is_empty() { "claude" } else { t })));
        }
        sep(ui, l);
        let note = trf("prefs.sessions.resume_note", &[&s.claude]);
        if row(ui, l, tr("prefs.sessions.resume"), &note, |ui| switch(ui, l, s.resume)) {
            out.push(Change::Set(Some("sessions"), "resume", (!s.resume).to_string()));
        }
    });
    section(ui, l, tr("prefs.head.waiting"), |ui| {
        let now = s.quiet.map_or_else(String::new, |q| q.to_string());
        let quiet = row(ui, l, tr("prefs.sessions.quiet"), tr("prefs.sessions.quiet_note"), |ui| {
            field(ui, &mut edit.drafts, "quiet", &now, "10", 80.0)
        });
        if let Some(t) = quiet {
            let t = t.trim().trim_end_matches('s').trim();
            if t.is_empty() {
                out.push(Change::SetIn("sessions".into(), "quiet".into(), None));
            } else if let Ok(v) = t.parse::<u64>() {
                out.push(Change::Set(Some("sessions"), "quiet", v.to_string()));
            }
        }
        sep(ui, l);
        let sorts: Vec<(Sort, &str)> = Sort::ALL.iter().map(|s| (*s, s.label())).collect();
        if let Some(v) = row(ui, l, tr("prefs.sessions.sort"), tr("prefs.sessions.sort_note"), |ui| select(ui, "sort", seen.sort, &sorts)) {
            out.push(Change::Sort(v));
        }
    });
    section(ui, l, tr("prefs.head.profiles"), |ui| {
        for p in seen.profiles {
            let (e, d) = row(ui, l, &p.name, &profile_words(p), |ui| {
                (button(ui, l, tr("prefs.common.delete")), button(ui, l, tr("prefs.common.edit")))
            });
            if d {
                edit.profile = Some(ProfileDraft::of(p));
            }
            if e {
                out.push(Change::DeleteProfile(p.name.clone()));
            }
            sep(ui, l);
        }
        if row(ui, l, tr("prefs.sessions.new_profile"), tr("prefs.sessions.new_profile_note"), |ui| button(ui, l, tr("prefs.common.add"))) {
            let home = cfg::home().map(|h| h.display().to_string()).unwrap_or_default();
            edit.profile = Some(ProfileDraft {
                was: None,
                name: String::new(),
                folder: home,
                start: "claude".into(),
                tags: String::new(),
                panes: Vec::new(),
                place: String::new(),
                theme: String::new(),
                font_size: String::new(),
            });
        }
        let mut done = None;
        if let Some(d) = &mut edit.profile {
            sep(ui, l);
            ui.add_space(10.0);
            let head = if d.was.is_some() { tr("prefs.sessions.edit_profile") } else { tr("prefs.sessions.a_new_profile") };
            ui.label(RichText::new(head).size(ito_theme::zoomed(ui.ctx(), 13.0)).strong().color(c.strong()));
            egui::Grid::new("profile-edit").num_columns(2).spacing(egui::vec2(14.0, 8.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("prefs.sessions.name")).color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                ui.end_row();
                ui.label(RichText::new(tr("prefs.sessions.folder")).color(c.dim));
                ui.add(
                    egui::TextEdit::singleline(&mut d.folder)
                        .desired_width(360.0)
                        .font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.5))),
                );
                ui.end_row();
                ui.label(RichText::new(tr("prefs.sessions.starts")).color(c.dim));
                if let Some(v) = select(ui, "profile-start", d.start.as_str(), &starts()) {
                    d.start = v.to_owned();
                }
                ui.end_row();
                ui.label(RichText::new(tr("prefs.common.tags")).color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.tags).hint_text(tr("prefs.sessions.comma_between")).desired_width(260.0));
                ui.end_row();
                ui.label(RichText::new(tr("prefs.sessions.runs_on")).color(c.dim));
                ui.add(
                    egui::TextEdit::singleline(&mut d.place)
                        .hint_text(tr("prefs.sessions.place_hint"))
                        .font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.5)))
                        .desired_width(260.0),
                );
                ui.end_row();
                ui.label(RichText::new(tr("prefs.sessions.theme")).color(c.dim));
                let mut themes = vec![("", tr("prefs.sessions.window_theme"))];
                themes.extend(seen.themes.iter().map(|t| (t.name.as_str(), t.name.as_str())));
                if let Some(v) = select(ui, "profile-theme", d.theme.as_str(), &themes) {
                    d.theme = v.to_owned();
                }
                ui.end_row();
                ui.label(RichText::new(tr("prefs.sessions.font_size")).color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.font_size).hint_text(tr("prefs.sessions.font_size_hint")).desired_width(80.0));
                ui.end_row();
                let mut gone = None;
                for (k, pane) in d.panes.iter_mut().enumerate() {
                    ui.label(
                        RichText::new(
                            [tr("prefs.sessions.pane_right"), tr("prefs.sessions.pane_below"), tr("prefs.sessions.pane_below_first")]
                                [k.min(2)],
                        )
                        .color(c.dim),
                    );
                    ui.horizontal(|ui| {
                        if let Some(v) = select(ui, &format!("profile-pane-{k}"), pane.as_str(), &starts()) {
                            *pane = v.to_owned();
                        }
                        if ui.small_button(tr("prefs.common.remove")).clicked() {
                            gone = Some(k);
                        }
                    });
                    ui.end_row();
                }
                if let Some(k) = gone {
                    d.panes.remove(k);
                }
            });
            if d.panes.len() < 3 && ui.small_button(tr("prefs.sessions.add_pane")).clicked() {
                d.panes.push("shell".into());
            }
            ui.add_space(6.0);
            let p = d.profile();
            let taken = seen.profiles.iter().any(|o| o.name == p.name && Some(&o.name) != d.was.as_ref());
            ui.horizontal(|ui| {
                let ok = !p.name.is_empty() && !p.folder.is_empty() && !taken && (d.font_size.trim().is_empty() || p.font_size.is_some());
                if taken {
                    ui.label(RichText::new(tr("prefs.sessions.taken")).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(c.err));
                }
                let (save, cancel) = crate::chrome::foot(ui, egui::Button::new(tr("prefs.common.save")), ok, tr("prefs.common.cancel"));
                if save.clicked() {
                    done = Some(Some(Change::SaveProfile(d.was.clone(), p)));
                }
                if cancel.clicked() {
                    done = Some(None);
                }
            });
            ui.add_space(10.0);
        }
        if let Some(change) = done {
            out.extend(change);
            edit.profile = None;
        }
    });
    let o = &seen.settings.open;
    section(ui, l, tr("prefs.head.open_with"), |ui| {
        commands(ui, l, edit, out, "editor", tr("prefs.open.editor"), tr("prefs.open.editor_note"), &o.editor, "code {folder}");
        sep(ui, l);
        commands(ui, l, edit, out, "filer", tr("prefs.open.kura"), tr("prefs.open.kura_note"), &o.filer, "kura {folder}");
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.open.file"), tr("prefs.open.file_note"), |ui| {
            field(ui, &mut edit.drafts, "file", &o.file, "code --goto {file}:{line}:{column}", 220.0)
        }) {
            out.push(Change::Set(Some("open"), "file", cfg::quote(&t)));
        }
    });
    tab_menu(ui, l, seen, edit, out);
}

/// Scrolls the page to the control Tab just moved to, when it is below the
/// fold (#60).
fn follow_focus(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let now = ctx.memory(|m| m.focused());
    let key = egui::Id::new("prefs-focused");
    let was = ctx.data(|d| d.get_temp::<Option<egui::Id>>(key)).flatten();
    if now != was {
        ctx.data_mut(|d| d.insert_temp(key, now));
        if let Some(rect) = now.and_then(|id| ctx.read_response(id)).map(|r| r.rect) {
            ui.scroll_to_rect(rect, None);
        }
    }
}

/// Controls that sit at the right of a row, added left to right: egui's Tab
/// follows the order they are added in, so a row laid out right to left
/// would be walked backwards (#47). The width is the last frame's, so the
/// first frame of a row shows it a little to the left.
fn reading_order<R>(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let was = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let free = ui.available_rect_before_wrap();
    let at = egui::pos2((free.right() - was).max(free.left()), free.top());
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let shown = ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(at, free.max)).layout(layout), add);
    let now = shown.response.rect.width();
    if (now - was).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, now));
        ui.ctx().request_repaint();
    }
    shown.inner
}

/// `[open] editor` or `filer`: a field for each command, the first the one
/// a click runs, and an empty one under them to add another (more than one
/// opens beside the menu's item).
#[allow(clippy::too_many_arguments)]
fn commands(
    ui: &mut egui::Ui,
    l: Look,
    edit: &mut Edit,
    out: &mut Vec<Change>,
    key: &'static str,
    label: &str,
    note: &str,
    now: &[String],
    hint: &str,
) {
    let write = |list: &[String]| Change::Set(Some("open"), key, cfg::quote_list(list));
    for (k, c) in now.iter().enumerate() {
        let (words, about) = if k == 0 {
            (label.to_owned(), trf("prefs.open.first_note", &[note]))
        } else {
            (format!("{label} {}", k + 1), trf("prefs.open.beside_as", &[&tsumugi_mux::settings::program_name(c)]))
        };
        let (typed, gone) = row(ui, l, &words, &about, |ui| {
            reading_order(ui, egui::Id::new(("open-with-row", key, k)), |ui| {
                let typed = field(ui, &mut edit.drafts, &format!("{key}-{k}"), c, hint, 220.0);
                (typed, ui.small_button("×").on_hover_text(tr("prefs.open.take_out")).clicked())
            })
        });
        let mut next = now.to_vec();
        if gone {
            next.remove(k);
            out.push(write(&next));
        } else if let Some(t) = typed {
            if t.trim().is_empty() {
                next.remove(k);
            } else {
                next[k] = t;
            }
            out.push(write(&next));
        }
    }
    let first = now.is_empty();
    let words = if first { label.to_owned() } else { tr("prefs.open.add_another").to_owned() };
    let about = if first { note.to_owned() } else { trf("prefs.open.another_note", &[&tsumugi_mux::settings::program_name(&now[0])]) };
    if let Some(t) = row(ui, l, &words, &about, |ui| {
        field(ui, &mut edit.drafts, &format!("{key}-new"), "", if first { hint } else { "sakura {folder}" }, 220.0)
    }) {
        if !t.trim().is_empty() {
            let mut next = now.to_vec();
            next.push(t);
            out.push(write(&next));
        }
    }
}

/// A built-in item of the tab's menu as the menu itself says it.
fn menu_label(word: &str) -> &'static str {
    match word {
        "rename" => tr("prefs.common.rename"),
        "note" => tr("prefs.menu.note"),
        "tags" => tr("prefs.common.tags"),
        "mute" => tr("tab.mute"),
        "pin" => tr("tab.pin"),
        "read-only" => tr("tab.read_only"),
        "restart" => tr("tab.restart"),
        "duplicate" => tr("tab.duplicate"),
        "new-window" => tr("tab.new_window"),
        "filer" => tr("tab.open_filer"),
        "editor" => tr("tab.open_editor"),
        "copy-path" => tr("tab.copy_path"),
        "save-output" => tr("tab.save_output"),
        "work-log" => tr("tab.work_log"),
        "pr" => tr("tab.pr"),
        "close" => tr("tab.close"),
        _ => cfg::menu_label(word),
    }
}

/// The tab's right-click menu: which items show, their order, and items of
/// one's own.
fn tab_menu(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let m = &seen.settings.menu;
    let words = cfg::menu_words(&m.order);
    section(ui, l, tr("prefs.head.tab_menu"), |ui| {
        for (k, word) in words.iter().enumerate() {
            if k > 0 {
                sep(ui, l);
            }
            let shown = !m.hide.iter().any(|h| h == word);
            let (up, down, flip) = row(ui, l, menu_label(word), "", |ui| {
                crate::chrome::in_order(ui, |ui| {
                    let up = ui.add_enabled(k > 0, egui::Button::new("↑").small()).on_hover_text(tr("prefs.menu.higher")).clicked();
                    let down =
                        ui.add_enabled(k + 1 < words.len(), egui::Button::new("↓").small()).on_hover_text(tr("prefs.menu.lower")).clicked();
                    ui.add_space(8.0);
                    let flip = switch(ui, l, shown);
                    (up, down, flip)
                })
            });
            if up || down {
                let mut next: Vec<String> = words.iter().map(|w| w.to_string()).collect();
                next.swap(k, if up { k - 1 } else { k + 1 });
                out.push(Change::Set(Some("menu"), "order", cfg::quote_list(&next)));
            }
            if flip {
                let mut hide: Vec<String> = m.hide.iter().filter(|h| h != word).cloned().collect();
                if shown {
                    hide.push(word.to_string());
                }
                out.push(Change::Set(Some("menu"), "hide", cfg::quote_list(&hide)));
            }
        }
        for (k, item) in m.session.iter().enumerate() {
            sep(ui, l);
            if row(ui, l, &item.name, &item.command, |ui| button(ui, l, tr("prefs.common.remove"))) {
                let mut next = m.session.clone();
                next.remove(k);
                out.push(Change::MenuItems(next));
            }
        }
        sep(ui, l);
        ui.add_space(10.0);
        ui.label(RichText::new(tr("prefs.menu.own_item")).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(c.dim));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut edit.item.0).hint_text(tr("prefs.menu.its_words")).desired_width(160.0));
            ui.add(
                egui::TextEdit::singleline(&mut edit.item.1)
                    .hint_text("lazygit -p {folder}")
                    .desired_width(260.0)
                    .font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.5))),
            );
            let ok = !edit.item.0.trim().is_empty() && !edit.item.1.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new(tr("prefs.common.add"))).clicked() {
                let mut next = m.session.clone();
                next.push(MenuItem { name: edit.item.0.trim().to_owned(), command: edit.item.1.trim().to_owned() });
                out.push(Change::MenuItems(next));
                edit.item = Default::default();
            }
        });
        ui.add_space(10.0);
    });
}

fn tags(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let t = &seen.settings.tags;
    let mut known: Vec<String> = seen.tags.to_vec();
    for name in t.rule.iter().map(|r| &r.tag).chain(t.colors.keys()).filter(|n| !n.contains('{')) {
        if !known.contains(name) {
            known.push(name.clone());
        }
    }
    // The tags themselves first (the one to edit, how they are coloured and
    // how many a session takes), then the rules that hand them out.
    section(ui, l, tr("prefs.head.tags"), |ui| {
        let names: Vec<(&str, &str)> = known.iter().map(|n| (n.as_str(), n.as_str())).collect();
        let now = edit.tag.clone().unwrap_or_default();
        let picked = row(ui, l, tr("prefs.tags.edit_tag"), tr("prefs.tags.edit_tag_note"), |ui| {
            if names.is_empty() {
                status(ui, tr("prefs.tags.none_yet"), c.dim);
                None
            } else {
                let mut all = vec![("", tr("prefs.tags.choose"))];
                all.extend(names.iter().copied());
                select(ui, "edit-tag", now.as_str(), &all)
            }
        });
        if let Some(p) = picked {
            edit.tag = (!p.is_empty()).then(|| p.to_owned());
            edit.rename = p.to_owned();
        }
        if let Some(tag) = edit.tag.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let zoomed_size = ito_theme::zoomed(ui.ctx(), 11.0);
                let (r, _) = ui.allocate_exact_size(
                    egui::vec2(
                        ui.fonts_mut(|f| f.layout_no_wrap(tag.clone(), FontId::proportional(zoomed_size), c.fg).size().x) + 12.0,
                        ito_theme::zoomed(ui.ctx(), 16.0),
                    ),
                    egui::Sense::hover(),
                );
                crate::chrome::tag_chip(ui.painter(), r.min, &tag, false);
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut edit.rename).desired_width(140.0));
                let new = tsumugi_mux::proto::tag_name(&edit.rename);
                if ui.add_enabled(new.as_ref().is_some_and(|n| *n != tag), egui::Button::new(tr("prefs.common.rename"))).clicked() {
                    if let Some(new) = new {
                        out.push(Change::RenameTag(tag.clone(), new.clone()));
                        edit.tag = Some(new);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("prefs.tags.colour")).color(c.dim));
                let own = t.colors.get(&tag);
                if ui.selectable_label(own.is_none(), tr("prefs.common.automatic")).clicked() && own.is_some() {
                    out.push(Change::SetIn("tags.colors".into(), toml_key(&tag), None));
                }
                for hex in crate::chrome::TAG_PALETTE {
                    let fill = Color32::from_hex(hex).unwrap_or(c.dim);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                    ui.painter().rect_filled(r, 4.0, fill);
                    if own.is_some_and(|o| o.eq_ignore_ascii_case(hex)) {
                        ui.painter().rect_stroke(r.expand(2.0), 5.0, egui::Stroke::new(1.5, c.strong()), egui::StrokeKind::Outside);
                    }
                    if resp.on_hover_text(hex).clicked() {
                        out.push(Change::SetIn("tags.colors".into(), toml_key(&tag), Some(cfg::quote(hex))));
                    }
                }
            });
            ui.add_space(6.0);
            let quiet = seen.muted_tags.contains(&tag);
            if row(ui, l, tr("prefs.tags.quiet"), tr("prefs.tags.quiet_note"), |ui| switch(ui, l, quiet)) {
                out.push(Change::MuteTag(tag.clone(), !quiet));
            }
        }
        sep(ui, l);
        row(ui, l, tr("prefs.tags.colour_new"), tr("prefs.tags.colour_new_note"), |ui| {
            select(ui, "tag-colour-kind", "auto", &[("auto", tr("prefs.common.automatic"))]);
        });
        sep(ui, l);
        row(ui, l, tr("prefs.tags.shown"), &trf("prefs.tags.shown_note", &[&tsumugi_mux::proto::MAX_TAGS.to_string()]), |ui| {
            const COUNTS: [(usize, &str); 5] = [(1, "1"), (2, "2"), (3, "3"), (4, "4"), (5, "5")];
            if let Some(n) = select(ui, "tags-shown", t.shown(), &COUNTS[..tsumugi_mux::proto::MAX_TAGS]) {
                out.push(Change::Set(Some("tags"), "shown", n.to_string()));
            }
        });
    });
    section(ui, l, tr("prefs.head.auto_tags"), |ui| {
        // The rule being edited went (the file changed under it).
        if edit.rule.at.is_some_and(|k| k >= t.rule.len()) {
            edit.rule = RuleDraft::default();
        }
        for (k, r) in t.rule.iter().enumerate() {
            let what = match (r.folder.is_empty(), r.branch.is_empty()) {
                (false, true) => trf("prefs.tags.rule_folder", &[&r.folder]),
                (true, false) => trf("prefs.tags.rule_branch", &[&r.branch]),
                _ => trf("prefs.tags.rule_both", &[&r.folder, &r.branch]),
            };
            let (gone, change) = row(ui, l, &what, "", |ui| {
                let gone = button(ui, l, tr("prefs.common.remove"));
                ui.add_space(4.0);
                let change = edit.rule.at != Some(k) && button(ui, l, tr("prefs.common.edit"));
                ui.add_space(8.0);
                let zoomed_size = ito_theme::zoomed(ui.ctx(), 11.0);
                let w = ui.fonts_mut(|f| f.layout_no_wrap(r.tag.clone(), FontId::proportional(zoomed_size), c.fg).size().x) + 12.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::hover());
                crate::chrome::tag_chip(ui.painter(), rect.min, &r.tag, false);
                (gone, change)
            });
            if gone {
                let mut next = t.rule.clone();
                next.remove(k);
                out.push(Change::Rules(next));
                edit.rule = RuleDraft::default();
            }
            if change {
                edit.rule = RuleDraft { at: Some(k), folder: r.folder.clone(), branch: r.branch.clone(), tag: r.tag.clone() };
            }
            sep(ui, l);
        }
        let (title, verb) = if edit.rule.at.is_some() {
            (tr("prefs.tags.edit_rule"), tr("prefs.common.save"))
        } else {
            (tr("prefs.tags.new_rule"), tr("prefs.common.add"))
        };
        row(ui, l, title, tr("prefs.tags.rule_note"), |ui| {
            crate::chrome::in_order(ui, |ui| {
                let d = &mut edit.rule;
                ui.add(
                    egui::TextEdit::singleline(&mut d.folder)
                        .hint_text(tr("prefs.tags.folder_hint"))
                        .desired_width(130.0)
                        .font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.5))),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut d.branch)
                        .hint_text(tr("prefs.tags.branch_hint"))
                        .desired_width(110.0)
                        .font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.5))),
                );
                ui.add(egui::TextEdit::singleline(&mut d.tag).hint_text(tr("prefs.tags.tag_hint")).desired_width(80.0));
                let ok = (!d.folder.trim().is_empty() || !d.branch.trim().is_empty()) && !d.tag.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new(verb)).clicked() {
                    let rule =
                        TagRule { folder: d.folder.trim().to_owned(), branch: d.branch.trim().to_owned(), tag: d.tag.trim().to_owned() };
                    let mut next = t.rule.clone();
                    match d.at {
                        Some(k) => next[k] = rule,
                        None => next.push(rule),
                    }
                    out.push(Change::Rules(next));
                    *d = RuleDraft::default();
                }
                if d.at.is_some() && button(ui, l, tr("prefs.common.cancel")) {
                    *d = RuleDraft::default();
                }
            })
        });
    });
}

fn shell(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let sh = &seen.settings.shell;
    let auto = trf("prefs.shell.auto", &[&ito_pane::shell_label(None)]);
    let found: &[(String, String)] = seen.facts.map(|f| f.shells.as_slice()).unwrap_or(&[]);
    let now = ito_prefs::Shell { program: &sh.program, auto: &auto, shells: found, args: &sh.args, env: &sh.env };
    let words = shell_words();
    for ch in ito_prefs::shell_card(ui, l, words, &mut edit.drafts, &mut edit.env, &now) {
        out.push(match ch {
            ito_prefs::ShellChange::Program(p) => Change::Set(Some("shell"), "program", cfg::quote(&p)),
            ito_prefs::ShellChange::Args(list) => Change::Set(Some("shell"), "args", cfg::quote_list(&list)),
            ito_prefs::ShellChange::SetVar { name, value } => Change::SetIn("shell.env".into(), toml_key(&name), Some(cfg::quote(&value))),
            ito_prefs::ShellChange::RemoveVar(name) => Change::SetIn("shell.env".into(), toml_key(&name), None),
        });
    }
    section(ui, l, tr("prefs.head.hooks"), |ui| {
        let facts = seen.facts;
        let hooks = facts.map(|f| f.hooks);
        let flip = row(ui, l, tr("prefs.shell.claude_hooks"), tr("prefs.shell.claude_hooks_note"), |ui| {
            let flip = match hooks {
                Some(on) => button(ui, l, if on { tr("prefs.common.remove") } else { tr("prefs.common.add") }),
                None => false,
            };
            ui.add_space(8.0);
            match hooks {
                Some(true) => status(ui, tr("prefs.common.installed"), c.done),
                Some(false) => status(ui, tr("prefs.common.not_installed"), c.dim),
                None => status(ui, "…", c.dim),
            }
            flip
        });
        if flip {
            out.push(Change::Hooks(!hooks.unwrap_or(false)));
        }
        sep(ui, l);
        let name = facts.map(|f| f.shell.clone()).unwrap_or_default();
        let shown = std::path::Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let note = trf("prefs.shell.integration_note", &[if shown.is_empty() { tr("prefs.shell.the_shell") } else { &shown }]);
        let hook = facts.map(|f| f.shell_hook);
        let flip = row(ui, l, tr("prefs.shell.integration"), &note, |ui| {
            let flip = match hook {
                Some(Some(on)) => button(ui, l, if on { tr("prefs.common.remove") } else { tr("prefs.common.install") }),
                _ => false,
            };
            ui.add_space(8.0);
            match hook {
                Some(Some(true)) => status(ui, tr("prefs.common.installed"), c.done),
                Some(Some(false)) => status(ui, tr("prefs.common.not_installed"), c.dim),
                Some(None) => status(ui, &trf("prefs.shell.no_hook", &[&shown]), c.dim),
                None => status(ui, "…", c.dim),
            }
            flip
        });
        if flip {
            out.push(Change::ShellHook(name.clone(), !matches!(hook, Some(Some(true)))));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.shell.lines"), tr("prefs.shell.lines_note"), |ui| {
            button(ui, l, if edit.lines { tr("prefs.common.hide") } else { tr("prefs.common.show") })
        }) {
            edit.lines = !edit.lines;
        }
        if edit.lines {
            // This tsumugi's own commands, by its full path (`hooks::commands`).
            let json = crate::hooks::add("").unwrap_or_default();
            let json = json.trim_end();
            ui.label(RichText::new(json).font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 11.5))).color(c.fg));
            if ui.small_button(tr("prefs.shell.copy")).clicked() {
                out.push(Change::Copy(json.to_owned()));
            }
            ui.add_space(6.0);
            let line = if cfg!(windows) { "tsumugi shell-hook pwsh >> $PROFILE" } else { "tsumugi shell-hook bash >> ~/.bashrc" };
            ui.label(RichText::new(line).font(FontId::monospace(ito_theme::zoomed(ui.ctx(), 12.0))).color(c.fg));
            if ui.small_button(format!("{} ", tr("prefs.shell.copy"))).clicked() {
                out.push(Change::Copy(line.to_owned()));
            }
            ui.add_space(8.0);
        }
    });
    if cfg!(windows) {
        section(ui, l, tr("prefs.head.windows"), |ui| {
            row(ui, l, tr("prefs.shell.conpty"), tr("prefs.shell.conpty_note"), |ui| match seen.facts.map(|f| f.conpty) {
                Some(true) => status(ui, tr("prefs.shell.bundled"), c.done),
                Some(false) => status(ui, tr("prefs.shell.system_conpty"), c.wait),
                None => status(ui, "…", c.dim),
            });
        });
    }
}

fn advanced(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let a = &seen.settings.advanced;
    section(ui, l, tr("prefs.head.server"), |ui| {
        row(ui, l, tr("prefs.advanced.mux"), &seen.address, |ui| status(ui, &trf("prefs.advanced.running", &[&seen.server_up]), c.done));
        sep(ui, l);
        if row(ui, l, tr("prefs.advanced.restart_server"), tr("prefs.advanced.restart_server_note"), |ui| {
            button(ui, l, tr("prefs.common.restart"))
        }) {
            out.push(Change::RestartServer);
        }
    });
    section(ui, l, tr("prefs.head.drawing"), |ui| {
        let mut backends = vec![("auto", tr("prefs.advanced.auto")), ("gl", "GL"), ("vulkan", "Vulkan")];
        if cfg!(windows) {
            backends.push(("dx12", "DirectX 12"));
        }
        if cfg!(target_os = "macos") {
            backends.push(("metal", "Metal"));
        }
        if !backends.iter().any(|(b, _)| *b == a.backend) {
            backends.push((a.backend.as_str(), a.backend.as_str()));
        }
        let note = tr("prefs.advanced.backend_note");
        if let Some(b) = row(ui, l, tr("prefs.advanced.backend"), note, |ui| select(ui, "backend", a.backend.as_str(), &backends)) {
            out.push(Change::Set(Some("advanced"), "backend", cfg::quote(b)));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, tr("prefs.advanced.scrollback"), tr("prefs.advanced.scrollback_note"), |ui| {
            field(ui, &mut edit.drafts, "scrollback", &a.scrollback.to_string(), "10000", 100.0)
        }) {
            if let Ok(v) = t.trim().replace(['_', ',', ' '], "").parse::<usize>() {
                out.push(Change::Set(Some("advanced"), "scrollback", v.clamp(100, 1_000_000).to_string()));
            }
        }
    });
    let r = &seen.settings.remote;
    section(ui, l, tr("prefs.head.machines"), |ui| {
        let note = tr("prefs.advanced.remote_note");
        if let Some(t) = row(ui, l, tr("prefs.advanced.command_there"), note, |ui| {
            field(ui, &mut edit.drafts, "remote_command", &r.command, "tsumugi", 200.0)
        }) {
            let t = t.trim();
            out.push(Change::Set(Some("remote"), "command", cfg::quote(if t.is_empty() { "tsumugi" } else { t })));
        }
        sep(ui, l);
        let hosts = if r.hosts.is_empty() { tr("prefs.advanced.none_yet").to_owned() } else { r.hosts.join(tr("prefs.common.list_sep")) };
        row(ui, l, tr("prefs.advanced.reached"), tr("prefs.advanced.reached_note"), |ui| status(ui, &hosts, c.fg));
    });
    section(ui, l, tr("prefs.head.files"), |ui| {
        if row(ui, l, tr("prefs.advanced.pane_log"), tr("prefs.advanced.pane_log_note"), |ui| switch(ui, l, a.pane_log)) {
            out.push(Change::Set(Some("advanced"), "pane_log", (!a.pane_log).to_string()));
        }
        if crate::secure::HERE
            && row(ui, l, tr("prefs.advanced.secure_input"), tr("prefs.advanced.secure_input_note"), |ui| switch(ui, l, a.secure_input))
        {
            out.push(Change::Set(Some("advanced"), "secure_input", (!a.secure_input).to_string()));
        }
        sep(ui, l);
        if row(ui, l, tr("prefs.advanced.settings_folder"), &seen.settings_path, |ui| button(ui, l, tr("prefs.common.open"))) {
            out.push(Change::OpenFolder);
        }
        sep(ui, l);
        row(ui, l, tr("prefs.advanced.saved_tabs"), &seen.state_path, |_| ());
        sep(ui, l);
        row(ui, l, tr("prefs.advanced.usage"), tr("prefs.advanced.usage_note"), |ui| status(ui, tr("prefs.common.none"), c.fg));
    });
    out.extend(ito_prefs::backup_card(ui, l, more(), crate::export::default_file()).into_iter().map(Change::Backup));
}

fn about(ui: &mut egui::Ui, l: Look, out: &mut Vec<Change>) {
    let system = ito_bugreport::os_line();
    let about =
        ito_prefs::About { version: env!("CARGO_PKG_VERSION"), commit: None, system: &system, license: Some(env!("CARGO_PKG_LICENSE")) };
    for action in ito_prefs::about_page(ui, l, more(), words(), "tsumugi", &about) {
        match action {
            ito_prefs::AboutAction::Report => out.push(Change::BugReport),
        }
    }
}

fn theme(ui: &mut egui::Ui, l: Look, pal: &Palette, seen: &Seen, out: &mut Vec<Change>) {
    let s = seen.settings;
    let note = tr("prefs.theme.note");
    let mut more = None;
    let changes = ito_prefs::theme_page(ui, l, words(), seen.themes, seen.choice, note, |ui| {
        preview(ui, pal);
        // What else the window looks like, a click away on its page.
        let f = &s.font;
        let family = if f.family.is_empty() { tr("prefs.common.automatic") } else { f.family.as_str() };
        let material = match s.window.material.as_str() {
            "none" => tr("prefs.common.none"),
            "mica" => "Mica",
            "acrylic" => "Acrylic",
            other => other,
        };
        let motion = if s.appearance.animations { tr("prefs.common.on") } else { tr("prefs.common.off") };
        ui.add_space(6.0);
        let line = trf("prefs.theme.line", &[family, &f.size.to_string(), material, motion]);
        if ui
            .add(egui::Label::new(RichText::new(line).size(ito_theme::zoomed(ui.ctx(), 12.0)).color(l.c.dim)).sense(egui::Sense::click()))
            .on_hover_text(tr("prefs.theme.hover"))
            .clicked()
        {
            more = Some(Change::GoTo(Page::Appearance));
        }
    });
    out.extend(changes.into_iter().map(Change::Common));
    out.extend(more);
}

/// A small window in the theme in force: the sidebar's three kinds of card
/// and a pane asking a question.
fn preview(ui: &mut egui::Ui, pal: &Palette) {
    let c = crate::theme::colors();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(460.0), 260.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 10.0, c.bg);
    p.rect_stroke(rect, 10.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    let side = egui::Rect::from_min_size(rect.min, egui::vec2(170.0, rect.height()));
    p.rect_filled(side.shrink(1.0), 9.0, c.side);
    // The sidebar's cards as the window draws them: the state's ring, the
    // name, the folder, the words; the running one's line along its top.
    let card = |y: f32, ring: Color32, fill: Color32, title: &str, folder: &str, words: &str, color: Color32| {
        let r = egui::Rect::from_min_size(egui::pos2(side.left() + 10.0, side.top() + y), egui::vec2(150.0, 52.0));
        p.rect_filled(r, 8.0, fill);
        p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, ring), egui::StrokeKind::Inside);
        p.text(r.left_top() + egui::vec2(10.0, 6.0), egui::Align2::LEFT_TOP, title, FontId::proportional(12.0), c.strong());
        p.text(r.left_top() + egui::vec2(10.0, 22.0), egui::Align2::LEFT_TOP, folder, FontId::monospace(10.0), c.dim);
        p.text(r.left_top() + egui::vec2(10.0, 36.0), egui::Align2::LEFT_TOP, words, FontId::proportional(10.5), crate::chrome::ink(color));
        r
    };
    card(12.0, c.wait, c.wait_bg(), tr("prefs.theme.preview_approve"), "~/dev/filer", &trf("state.waiting_long", &["2m"]), c.wait);
    let running = card(
        72.0,
        c.run.gamma_multiply(0.6),
        c.panel,
        tr("prefs.theme.preview_split"),
        "~/dev/tsumugi",
        &trf("state.running", &["4m"]),
        c.dim,
    );
    p.line_segment([running.left_top() + egui::vec2(30.0, 1.0), running.left_top() + egui::vec2(80.0, 1.0)], egui::Stroke::new(2.0, c.run));
    card(
        132.0,
        c.err.gamma_multiply(0.7),
        c.panel,
        tr("prefs.theme.preview_ci"),
        "~/dev/filer",
        &trf("state.error", &[tr("prefs.theme.preview_exited")]),
        c.err,
    );
    card(192.0, c.done.gamma_multiply(0.5), c.panel, tr("prefs.theme.preview_scope"), "~/notes", &trf("state.done", &["20m"]), c.dim);
    let pane = egui::Rect::from_min_max(egui::pos2(side.right() + 10.0, rect.top() + 10.0), rect.max - egui::vec2(10.0, 10.0));
    p.rect_filled(pane, 8.0, pal.bg);
    p.rect_stroke(pane, 8.0, egui::Stroke::new(1.0, c.wait), egui::StrokeKind::Inside);
    let mono = FontId::monospace(ito_theme::zoomed(ui.ctx(), 11.5));
    let lines: [(&str, Color32); 6] = [
        ("● Update(src/terminal.rs)", c.done),
        ("  ⎿  Updated 3 lines", c.dim),
        ("-   self.send_as(text)", c.err),
        ("+   self.send_as(bytes)", c.done),
        ("Make this edit?", pal.fg),
        ("❯ 1. Yes", c.wait),
    ];
    for (k, (t, color)) in lines.iter().enumerate() {
        p.text(pane.left_top() + egui::vec2(12.0, 12.0 + k as f32 * 20.0), egui::Align2::LEFT_TOP, *t, mono.clone(), *color);
    }
}

/// The screen drawn without a window: each page's rows are there, and a
/// click, a key or typing hands back the change it should.
#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: f32 = 1200.0;

    struct Run {
        ctx: egui::Context,
        settings: Settings,
        facts: Facts,
        themes: Vec<Theme>,
        clock: Clock,
    }

    impl Run {
        fn new() -> Self {
            let facts = Facts {
                shells: vec![("bash".into(), "bash".into())],
                hooks: false,
                shell: "bash".into(),
                shell_hook: Some(false),
                autostart: false,
                conpty: true,
            };
            Self {
                ctx: egui::Context::default(),
                settings: Settings::default(),
                facts,
                themes: crate::theme::builtin(),
                clock: Clock::default(),
            }
        }

        /// One frame with these events: what was changed, and every text
        /// drawn with where.
        fn frame(&self, screen: &mut Screen, events: Vec<egui::Event>) -> (Vec<Change>, Vec<(String, egui::Rect)>) {
            let tags = vec!["review".to_string()];
            let names = vec!["Fira Code".to_string()];
            let seen = Seen {
                settings: &self.settings,
                themes: &self.themes,
                current: crate::theme::colors(),
                keymap: crate::keys::built_in(false),
                profiles: &[],
                tags: &tags,
                muted_tags: &[],
                sort: Sort::default(),
                always_restore: false,
                nerd: false,
                font_names: &names,
                font_file: None,
                faces: [false; 3],
                server_up: "3m".into(),
                address: "/run/tsumugi.sock".into(),
                settings_path: "/home/u/.config/tsumugi".into(),
                state_path: "/home/u/.local/state/tsumugi/state".into(),
                facts: Some(&self.facts),
                language: "auto",
                scale: 1.0,
                mono: Default::default(),
                window: Default::default(),
                choice: ("dark", "tsumugi Dark", "tsumugi Light"),
                clock: &self.clock,
            };
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(WIDE, 2600.0))),
                events,
                ..Default::default()
            };
            let mut changes = Vec::new();
            let out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| changes = show(ui, &Palette::default(), screen, &seen));
            });
            let mut out = out;
            out.textures_delta.clear();
            let mut texts = Vec::new();
            fn walk(s: &egui::Shape, v: &mut Vec<(String, egui::Rect)>) {
                match s {
                    egui::Shape::Text(t) => v.push((t.galley.text().to_owned(), t.galley.rect.translate(t.pos.to_vec2()))),
                    egui::Shape::Vec(list) => list.iter().for_each(|s| walk(s, v)),
                    _ => {}
                }
            }
            for c in &out.shapes {
                walk(&c.shape, &mut texts);
            }
            (changes, texts)
        }

        /// Draw twice (the first frame lays out), then click at `at`.
        fn click(&self, screen: &mut Screen, at: egui::Pos2) -> Vec<Change> {
            let press = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let mut all = self.frame(screen, vec![egui::Event::PointerMoved(at)]).0;
            all.extend(self.frame(screen, vec![press(true)]).0);
            all.extend(self.frame(screen, vec![press(false)]).0);
            all
        }

        fn key(&self, screen: &mut Screen, key: egui::Key, modifiers: egui::Modifiers) -> Vec<Change> {
            let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
            self.frame(screen, vec![ev(true), ev(false)]).0
        }
    }

    /// The theme page's Mode sits just under the heading, the list of themes
    /// right under it -- not centred in the page's height (seen on Windows,
    /// 2026-10-07: a bare right-to-left layout took all the height left).
    #[test]
    fn the_theme_pages_mode_stays_under_its_heading() {
        let run = Run::new();
        let mut screen = at_page(Page::Theme);
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let heading = find(&texts, "Colours, applied as you pick.").expect("the heading").bottom();
        let mode = find(&texts, "Mode").expect("Mode").top();
        let list = find(&texts, "THEME").expect("the list's heading").top();
        assert!(mode - heading < 80.0, "Mode at {mode}, the heading ends at {heading}");
        assert!(list - mode < 80.0, "the list at {list}, Mode at {mode}");
        // At the left, under the heading's start, and the choices after it.
        let left = find(&texts, "Colours, applied as you pick.").unwrap().left();
        let (mode_x, follow, dark) =
            (find(&texts, "Mode").unwrap().left(), find(&texts, "Follow OS").unwrap().left(), find(&texts, "Dark").unwrap().left());
        assert!(
            (mode_x - left).abs() < 4.0 && mode_x < follow && follow < dark,
            "Mode at {mode_x}, the heading at {left}, Follow OS at {follow}, Dark at {dark}"
        );
    }

    /// A row of several controls reads, and is tabbed through, left to
    /// right (seen on Windows, 2026-10-07: Tab went tag, folder, kind), and
    /// still ends at the right edge with the other rows.
    #[test]
    fn a_rows_controls_go_left_to_right() {
        let run = Run::new();
        let mut screen = at_page(Page::Tags);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let x = |w: &str| find(&texts, w).unwrap_or_else(|| panic!("{w}")).left();
        let (folder, branch, tag, add) = (x("folder: ~/dev/*"), x("branch: claude/*"), x("tag"), x("Add"));
        assert!(folder < branch && branch < tag && tag < add, "folder {folder}, branch {branch}, tag {tag}, Add {add}");
        let add_right = find(&texts, "Add").unwrap().right();
        // The card's right edge, less its padding (as in `a_switch_writes_its_key`).
        let edge = 48.0 + 241.0 + 820.0 - 18.0;
        assert!((add_right - edge).abs() < 12.0, "Add ends at {add_right}, the other rows at {edge}");
    }

    #[test]
    fn the_keys_line_up_in_their_column() {
        let run = Run::new();
        let mut screen = at_page(Page::Keys);
        for _ in 0..3 {
            run.frame(&mut screen, Vec::new());
        }
        let (_, texts) = run.frame(&mut screen, Vec::new());
        // Each row's first key starts at the keys' column, whatever the
        // operation's name is long.
        let caps: Vec<f32> = ["Ctrl+Shift+T", "Ctrl+Tab", "Ctrl+Shift+Tab", "Ctrl+I", "Ctrl+Shift+B", "Alt++", "Ctrl+Shift+Z"]
            .iter()
            .map(|w| find(&texts, w).unwrap_or_else(|| panic!("{w}")).left())
            .collect();
        assert!(caps.iter().all(|x| (x - caps[0]).abs() < 1.0), "{caps:?}");
    }

    fn at_page(page: Page) -> Screen {
        Screen { page, ..Screen::default() }
    }

    /// Where a text was drawn: the first exact match, else the first that
    /// holds it.
    fn find<'a>(texts: &'a [(String, egui::Rect)], words: &str) -> Option<&'a egui::Rect> {
        texts.iter().find(|(t, _)| t == words).or_else(|| texts.iter().find(|(t, _)| t.contains(words))).map(|(_, r)| r)
    }

    /// The other way round: each row the pages draw is in the index, or the
    /// search said nothing for it ("copy" found no page, the real machine).
    #[test]
    fn every_row_is_in_the_search_index() {
        let source = include_str!("prefs.rs");
        let needle = ["row(ui", ", l, tr(\""].concat();
        let index = index();
        let missing: Vec<&str> = source
            .split(needle.as_str())
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .filter(|k| crate::secure::HERE || *k != "prefs.advanced.secure_input")
            .map(tr)
            .filter(|label| !index.iter().any(|(_, words)| *label == *words || (words.len() >= 8 && label.contains(words))))
            .collect();
        assert!(source.split(needle.as_str()).count() > 50, "the rows' labels are looked up by key");
        assert!(missing.is_empty(), "rows the search cannot find: {missing:?}");
    }

    /// Rows drawn only on some systems or once something is open: the window
    /// material (Windows and macOS), ConPTY (Windows), a tag's own rows.
    const SOMETIMES: &[&str] = &["Window material", "ConPTY", "Quiet"];

    #[test]
    fn every_page_draws_the_rows_its_search_finds() {
        let run = Run::new();
        for page in Page::ALL {
            let mut screen = at_page(page);
            run.frame(&mut screen, Vec::new());
            let (_, texts) = run.frame(&mut screen, Vec::new());
            assert!(find(&texts, page.title()).is_some(), "{page:?}: its title");
            for (_, words) in index().into_iter().filter(|(p, words)| *p == page && !SOMETIMES.contains(words)) {
                // The Keys page is longer than the screen: its operations
                // are found by the search narrowing the table.
                let texts = if page == Page::Keys && find(&texts, words).is_none() {
                    let mut screen = Screen { query: words.into(), ..at_page(page) };
                    run.frame(&mut screen, Vec::new());
                    run.frame(&mut screen, Vec::new()).1
                } else {
                    texts.clone()
                };
                assert!(find(&texts, words).is_some(), "{page:?} has no `{words}`: {:?}", texts.iter().map(|t| &t.0).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn the_nav_goes_to_a_page_and_the_search_narrows_it() {
        let run = Run::new();
        let mut screen = Screen::default();
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let keys = *texts.iter().find(|(t, r)| t == "Keys" && r.left() < 240.0).map(|(_, r)| r).expect("Keys in the nav");
        run.click(&mut screen, keys.center());
        assert_eq!(screen.page, Page::Keys);

        // (Not "scrollback": a key's operation says it, so Keys has it too.)
        screen.query = "graphics backend".into();
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert_eq!(screen.page, Page::Advanced, "to the page that has it");
        let nav: Vec<&str> = texts.iter().filter(|(_, r)| r.left() < 240.0).map(|(t, _)| t.as_str()).collect();
        assert!(nav.contains(&"Advanced") && !nav.contains(&"General"), "{nav:?}");

        screen.query = "no such setting".into();
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Nothing matches").is_some());
    }

    #[test]
    fn a_switch_writes_its_key() {
        let run = Run::new();
        let mut screen = Screen::default();
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let label = *find(&texts, "Check for updates").unwrap();
        // The switch is at the card's right edge, in the middle of the row.
        let card_right = 48.0 + 241.0 + 820.0 - 18.0;
        let changes = run.click(&mut screen, egui::pos2(card_right - 20.0, label.top() + 16.0));
        assert_eq!(changes, vec![Change::Set(Some("general"), "check_updates", "false".into())]);
    }

    #[test]
    fn the_clock_writes_the_common_file() {
        let run = Run::new();
        let mut screen = Screen::default();
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let label = *find(&texts, "Show the date").unwrap();
        let card_right = 48.0 + 241.0 + 820.0 - 18.0;
        let changes = run.click(&mut screen, egui::pos2(card_right - 20.0, label.top() + 16.0));
        assert_eq!(changes, vec![Change::Common(CommonChange { table: Some("clock"), key: "date", value: "false".into() })]);
    }

    #[test]
    fn a_field_writes_on_enter() {
        let run = Run::new();
        let mut screen = at_page(Page::Sessions);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let now = *texts.iter().find(|(t, r)| t == "claude" && r.left() > 600.0).map(|(_, r)| r).expect("the field's words");
        run.click(&mut screen, egui::pos2(now.right() + 1.0, now.center().y));
        let typed = run.frame(&mut screen, vec![egui::Event::Text("-x".into())]).0;
        assert!(typed.is_empty(), "nothing while typing: {typed:?}");
        let changes = run.key(&mut screen, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(changes, vec![Change::Set(Some("sessions"), "claude", "\"-x\"".into())]);
        // Shown as typed until the file says it. A field takes the keys with
        // its words selected, so typing replaces them.
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "-x").is_some());
    }

    #[test]
    fn a_key_is_set_with_enter_and_clashes_are_said() {
        let run = Run::new();
        let ctrl = egui::Modifiers { ctrl: true, ..Default::default() };
        let ctrl_shift = egui::Modifiers { ctrl: true, shift: true, ..Default::default() };
        let mut screen = at_page(Page::Keys);
        run.frame(&mut screen, Vec::new());
        let capture = |run: &str| ito_prefs::Capture { layer: "window".into(), run: run.into(), keys: Vec::new(), replace: None };
        screen.edit.keys.capture = Some(capture("new_tab"));
        // Pressed, it is not written until Enter.
        let changes = run.key(&mut screen, egui::Key::K, ctrl_shift);
        assert!(changes.is_empty(), "{changes:?}");
        let changes = run.key(&mut screen, egui::Key::Enter, egui::Modifiers::NONE);
        let [Change::Keys(edits)] = &changes[..] else { panic!("{changes:?}") };
        let [ito_keymap::Edit::Add { layer, on, run: name, .. }] = &edits[..] else { panic!("{edits:?}") };
        assert_eq!((layer.as_str(), ito_keys::render_seq(on).as_str(), name.as_str()), ("window", "<C-S-k>", "new_tab"));
        assert!(screen.edit.keys.capture.is_none());

        // Another of the window's keys, and one of Claude Code's: said, and
        // nothing written.
        screen.edit.keys.capture = Some(capture("new_tab"));
        let changes = run.key(&mut screen, egui::Key::W, ctrl_shift);
        assert!(changes.is_empty(), "{changes:?}");
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let said = |texts: &[(String, egui::Rect)], part: &str| texts.iter().any(|(t, _)| t.contains(part));
        assert!(said(&texts, "Close the session"), "{:?}", texts.iter().map(|t| &t.0).collect::<Vec<_>>());
        run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        screen.edit.keys.capture = Some(capture("new_tab"));
        let changes = run.key(&mut screen, egui::Key::C, ctrl);
        assert!(changes.is_empty());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(said(&texts, "Claude Code's interrupt"), "{:?}", texts.iter().map(|t| &t.0).collect::<Vec<_>>());

        // Esc while a key is being changed leaves the key, not the screen.
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(changes.is_empty());
        assert!(screen.edit.keys.capture.is_none());
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(changes, vec![Change::Close]);
    }

    /// The settings search that names an operation narrows the table to it;
    /// the search gone, the table is whole again. One that names none
    /// leaves the table's own search.
    #[test]
    fn the_settings_search_narrows_the_keys_table() {
        let run = Run::new();
        let mut screen = Screen { query: "browser".into(), ..at_page(Page::Keys) };
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert_eq!((screen.page, screen.edit.keys.query.as_str()), (Page::Keys, "browser"));
        assert!(find(&texts, "Browser: the page of a port this tab listens on").is_some());
        assert!(find(&texts, "New session").is_none(), "only the operations it names");
        screen.query.clear();
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert_eq!(screen.edit.keys.query, "");
        run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "New session").is_some() || find(&run.frame(&mut screen, Vec::new()).1, "New session").is_some());
        screen.edit.keys.query = "zoom".into();
        screen.query = "use cmd".into();
        run.frame(&mut screen, Vec::new());
        assert_eq!(screen.edit.keys.query, "zoom", "kept");
    }

    /// A key being pressed for an action stops when the page is left.
    #[test]
    fn leaving_the_keys_page_lets_the_keys_go() {
        let run = Run::new();
        let mut screen = at_page(Page::Keys);
        screen.edit.keys.capture =
            Some(ito_prefs::Capture { layer: "window".into(), run: "new_tab".into(), keys: Vec::new(), replace: None });
        screen.page = Page::General;
        run.frame(&mut screen, Vec::new());
        assert!(!screen.edit.keys.busy());
    }

    /// What was typed in the Keys table's own search is not taken over.
    #[test]
    fn the_keys_tables_own_search_stays() {
        let mut edit = Edit::default();
        follow_search(&mut edit, "zoom");
        assert_eq!(edit.keys.query, "zoom");
        follow_search(&mut edit, "");
        assert_eq!(edit.keys.query, "");
        edit.keys.query = "tab".into();
        follow_search(&mut edit, "browser");
        assert_eq!(edit.keys.query, "tab");
        follow_search(&mut edit, "");
        assert_eq!(edit.keys.query, "tab");
        // In other words than English too, by the English name.
        edit.keys.query.clear();
        follow_search(&mut edit, "Zoom one pane");
        assert_eq!(edit.keys.query, "Zoom one pane");
    }

    /// While a key is being taken, Ctrl+Tab is that key, not the next page.
    #[test]
    fn a_key_being_taken_keeps_ctrl_tab() {
        let run = Run::new();
        let mut screen = at_page(Page::Keys);
        screen.edit.keys.capture =
            Some(ito_prefs::Capture { layer: "window".into(), run: "new_tab".into(), keys: Vec::new(), replace: None });
        let tab =
            egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::CTRL };
        run.frame(&mut screen, vec![tab]);
        assert_eq!(screen.page, Page::Keys);
        let keys = screen.edit.keys.capture.as_ref().map(|c| ito_keys::render_seq(&c.keys));
        assert_eq!(keys.as_deref(), Some("<C-Tab>"));
    }

    #[test]
    fn the_machine_says_what_is_installed() {
        let mut run = Run::new();
        let mut screen = at_page(Page::Shell);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Not installed").is_some());
        assert!(find(&texts, "Install").is_some());
        run.facts.hooks = true;
        run.facts.shell_hook = None;
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Installed").is_some());
        assert!(find(&texts, "No hook for bash").is_some());
        assert!(find(&texts, "Remove").is_some());
    }

    #[test]
    fn rows_from_the_file_show_and_go() {
        let mut run = Run::new();
        run.settings.tags.rule = vec![
            TagRule { folder: "~/dev/*".into(), tag: "{name}".into(), ..TagRule::default() },
            TagRule { branch: "claude/*".into(), tag: "claude".into(), ..TagRule::default() },
        ];
        run.settings.menu.session = vec![MenuItem { name: "lazygit".into(), command: "lazygit -p {folder}".into() }];
        let mut screen = at_page(Page::Tags);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Folder ~/dev/*").is_some() && find(&texts, "Branch claude/*").is_some());
        let first = *find(&texts, "Folder ~/dev/*").unwrap();
        let remove = texts
            .iter()
            .filter(|(t, _)| t == "Remove")
            .map(|(_, r)| *r)
            .find(|r| (r.center().y - first.top() - 16.0).abs() < 14.0)
            .expect("its Remove");
        let changes = run.click(&mut screen, remove.center());
        assert_eq!(changes, vec![Change::Rules(vec![run.settings.tags.rule[1].clone()])]);

        // Edit: the rule comes into the fields, and Save puts it back in place.
        let edit = texts
            .iter()
            .filter(|(t, _)| t == "Edit")
            .map(|(_, r)| *r)
            .find(|r| (r.center().y - first.top() - 16.0).abs() < 14.0)
            .expect("its Edit");
        assert!(run.click(&mut screen, edit.center()).is_empty());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Edit rule").is_some() && find(&texts, "Cancel").is_some());
        // The field is below the rule's own chip.
        let tag = texts
            .iter()
            .filter(|(t, _)| t == "{name}")
            .map(|(_, r)| *r)
            .max_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("the tag's field");
        run.click(&mut screen, egui::pos2(tag.right() + 1.0, tag.center().y));
        run.frame(&mut screen, vec![egui::Event::Text("-x".into())]);
        let (_, texts) = run.frame(&mut screen, Vec::new());
        let save = *find(&texts, "Save").expect("Save");
        let changes = run.click(&mut screen, save.center());
        let mut want = run.settings.tags.rule.clone();
        want[0].tag = "{name}-x".into();
        assert_eq!(changes, vec![Change::Rules(want)]);

        let mut screen = at_page(Page::Sessions);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "lazygit -p {folder}").is_some(), "one's own menu item");
        assert!(find(&texts, "Copy the folder path").is_some(), "the built-in items");
    }

    #[test]
    fn the_keys_walk_the_pages_and_the_controls() {
        let run = Run::new();
        let ctrl = egui::Modifiers::CTRL;
        let mut screen = Screen::default();
        run.frame(&mut screen, Vec::new());
        run.key(&mut screen, egui::Key::Tab, ctrl);
        assert_eq!(screen.page, Page::Appearance);
        run.key(&mut screen, egui::Key::Tab, ctrl | egui::Modifiers::SHIFT);
        run.key(&mut screen, egui::Key::Tab, ctrl | egui::Modifiers::SHIFT);
        assert_eq!(screen.page, Page::About, "round from the first");
        run.key(&mut screen, egui::Key::PageDown, ctrl);
        assert_eq!(screen.page, Page::General);

        // Tab: the search, then into the page, past the list of pages.
        run.key(&mut screen, egui::Key::Tab, egui::Modifiers::NONE);
        run.frame(&mut screen, Vec::new());
        run.key(&mut screen, egui::Key::Tab, egui::Modifiers::NONE);
        run.frame(&mut screen, Vec::new());
        let at = run.ctx.memory(|m| m.focused()).and_then(|id| run.ctx.read_response(id)).map(|r| r.rect).expect("a control has the keys");
        assert!(at.left() > 240.0, "in the page: {at:?}");
        // Esc lets go of it first; only the next leaves the screen.
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!changes.contains(&Change::Close), "{changes:?}");
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(changes.contains(&Change::Close));
    }

    #[test]
    fn a_switch_takes_space() {
        let run = Run::new();
        let mut screen = Screen { page: Page::Notifications, ..Screen::default() };
        run.frame(&mut screen, Vec::new());
        // Tab along until a switch has the keys: the table's first.
        let mut changes = Vec::new();
        for _ in 0..12 {
            run.key(&mut screen, egui::Key::Tab, egui::Modifiers::NONE);
            run.frame(&mut screen, Vec::new());
            let r = run.ctx.memory(|m| m.focused()).and_then(|id| run.ctx.read_response(id)).map(|r| r.rect);
            if r.is_some_and(|r| r.width() == 40.0 && r.height() == 22.0) {
                changes = run.key(&mut screen, egui::Key::Space, egui::Modifiers::NONE);
                break;
            }
        }
        assert_eq!(
            changes,
            vec![Change::Set(Some("notify"), "system", "[\"error\", \"done\"]".into())],
            "the first switch: system, waiting"
        );
    }

    /// The one address of before is the first webhook: its row says where,
    /// "Add an address" writes it with a new one, and "Send a test here"
    /// sends to it only.
    #[test]
    fn the_webhooks_add_and_test_one_by_one() {
        let mut run = Run::new();
        run.settings.notify.webhook = "https://ntfy.sh/a".into();
        run.settings.notify.webhook_states = vec!["done".into()];
        let mut screen = at_page(Page::Notifications);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "https://ntfy.sh/a").is_some(), "its address");
        let old = Place { url: "https://ntfy.sh/a".into(), format: "ntfy".into(), states: vec!["done".into()] };
        let add = *find(&texts, "Add an address").expect("Add an address");
        let new = Place { url: String::new(), format: "ntfy".into(), states: STATES.map(String::from).to_vec() };
        assert_eq!(run.click(&mut screen, add.center()), vec![Change::Webhooks(vec![old.clone(), new])]);
        let test = *find(&texts, "Send a test here").expect("Send a test here");
        assert_eq!(run.click(&mut screen, test.center()), vec![Change::TestWebhook(old)]);
    }

    /// A profile's theme and font size go through the editor and back; a
    /// size that is not one is left out.
    #[test]
    fn a_profile_keeps_its_theme_and_font_size() {
        let p = Profile {
            name: "work".into(),
            folder: "~/dev".into(),
            start: "claude".into(),
            tags: vec![],
            panes: vec![],
            place: String::new(),
            theme: "Nord".into(),
            font_size: Some(16.0),
        };
        let mut d = ProfileDraft::of(&p);
        assert_eq!(d.profile(), p);
        assert!(profile_words(&p).ends_with(" · Nord · 16pt"), "{}", profile_words(&p));
        d.font_size = "18 pt".into();
        assert_eq!(d.profile().font_size, Some(18.0));
        for no in ["big", "3", "200", ""] {
            d.font_size = no.into();
            assert_eq!(d.profile().font_size, None, "{no:?}");
        }
    }
}
