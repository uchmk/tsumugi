//! The settings screen (the design's 1m and "Settings: every page",
//! `Ctrl+,`): nine pages down the left under a search field, each a few
//! cards of rows. It shows what `settings.toml` says and hands back what to
//! change; the window writes it (one line or one table of the file at a
//! time, so the rest stays as written) and reads it again. What the machine
//! says rather than the file (the shells installed, the hooks, starting at
//! sign-in) comes in [`Seen::facts`].

use std::collections::HashMap;

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::settings::{self as cfg, MenuItem, Profile, Settings, TagRule, DATE_FORMATS};
use tsumugi_pane::Palette;

use crate::facts::Facts;
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
}

impl Page {
    pub const ALL: [Page; 9] =
        [Page::General, Page::Appearance, Page::Keys, Page::Notifications, Page::Sessions, Page::Tags, Page::Theme, Page::Shell, Page::Advanced];

    pub fn title(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Keys => "Keys",
            Page::Notifications => "Notifications",
            Page::Sessions => "Sessions & profiles",
            Page::Tags => "Tags",
            Page::Theme => "Theme",
            Page::Shell => "Shell & hooks",
            Page::Advanced => "Advanced",
        }
    }

    fn lead(self) -> &'static str {
        match self {
            Page::General => "Startup, what closing the window does, and the clock.",
            Page::Appearance => "Type, window and motion. Colours are on the Theme page.",
            Page::Keys => "Click a key, then press the new one. A clash with another key or with Claude Code is named.",
            Page::Notifications => "Only while you are not at a tsumugi window. Inside it, rings and tabs say it.",
            Page::Sessions => "What a new session runs, how waiting is told, saved sets of sessions and the tab's menu.",
            Page::Tags => "Tags a session gets by itself, and their colours.",
            Page::Theme => "Colours, applied as you pick.",
            Page::Shell => "The shell inside panes, and how sessions tell tsumugi what they are doing.",
            Page::Advanced => "Things to touch rarely.",
        }
    }
}

/// The rows each page has, for the search field: a page is listed while the
/// words are in one of its rows. The screen's own test checks that each is
/// drawn on its page.
pub const INDEX: &[(Page, &str)] = &[
    (Page::General, "Language"),
    (Page::General, "On start"),
    (Page::General, "Default folder"),
    (Page::General, "Start the server at sign-in"),
    (Page::General, "Keep sessions running when the window closes"),
    (Page::General, "Ask before closing a running session"),
    (Page::General, "Check for updates"),
    (Page::General, "Show the time in the status bar"),
    (Page::General, "Time format"),
    (Page::General, "Show the date"),
    (Page::General, "Date format"),
    (Page::General, "Show the weekday"),
    (Page::Appearance, "Font"),
    (Page::Appearance, "Size"),
    (Page::Appearance, "Line height"),
    (Page::Appearance, "Ligatures"),
    (Page::Appearance, "Nerd Font icons"),
    (Page::Appearance, "tsumugi's own title bar"),
    (Page::Appearance, "Dim unfocused panes"),
    (Page::Appearance, "Cursor"),
    (Page::Appearance, "Animations"),
    (Page::Keys, "Preset"),
    (Page::Keys, "Use Cmd on macOS"),
    (Page::Keys, "New session"),
    (Page::Keys, "Close the session"),
    (Page::Keys, "Rename the tab"),
    (Page::Keys, "Duplicate in the same folder"),
    (Page::Keys, "Next tab"),
    (Page::Keys, "Previous tab"),
    (Page::Keys, "Go to the session waiting longest"),
    (Page::Keys, "The waiting sessions, answered together"),
    (Page::Keys, "The Nth tab"),
    (Page::Keys, "Split right"),
    (Page::Keys, "Split down"),
    (Page::Keys, "Move between panes"),
    (Page::Keys, "Resize the pane"),
    (Page::Keys, "Zoom"),
    (Page::Notifications, "System notification"),
    (Page::Notifications, "Taskbar count"),
    (Page::Notifications, "Taskbar flash"),
    (Page::Notifications, "Sound"),
    (Page::Notifications, "Tell about a finish"),
    (Page::Notifications, "Sound for waits"),
    (Page::Notifications, "Sound for fails"),
    (Page::Notifications, "focus mode"),
    (Page::Notifications, "QUIET TAGS"),
    (Page::Sessions, "Default program"),
    (Page::Sessions, "Claude Code command"),
    (Page::Sessions, "Resume conversations after a restart"),
    (Page::Sessions, "probably waiting"),
    (Page::Sessions, "Sort"),
    (Page::Sessions, "Offer compact rows above"),
    (Page::Sessions, "New profile"),
    (Page::Sessions, "Editor command"),
    (Page::Sessions, "filer command"),
    (Page::Sessions, "TAB MENU"),
    (Page::Tags, "New rule"),
    (Page::Tags, "Colour new tags"),
    (Page::Tags, "Tags per session"),
    (Page::Tags, "Edit a tag"),
    (Page::Theme, "Mode"),
    (Page::Theme, "PREVIEW"),
    (Page::Shell, "Default shell"),
    (Page::Shell, "Arguments"),
    (Page::Shell, "Environment"),
    (Page::Shell, "Claude Code hooks"),
    (Page::Shell, "Shell integration"),
    (Page::Advanced, "Mux server"),
    (Page::Advanced, "Restart the server"),
    (Page::Advanced, "Graphics backend"),
    (Page::Advanced, "Scrollback"),
    (Page::Advanced, "Log what each pane sends and receives"),
    (Page::Advanced, "Settings folder"),
    (Page::Advanced, "Export or import settings"),
    (Page::Advanced, "Usage data"),
];

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
    /// The key the next press goes to (`[keys]`'s name), while one is being
    /// changed.
    pub capturing: Option<&'static str>,
    /// The press that clashed: the key's name, what it clashes with, and the
    /// chord, to use anyway.
    clash: Option<(&'static str, String, String)>,
    /// The text fields' words while they are typed in, and just after.
    drafts: HashMap<String, Draft>,
    profile: Option<ProfileDraft>,
    /// A new tag rule: by branch rather than folder, its pattern, its tag.
    rule: (bool, String, String),
    /// The variables being edited, from "Edit".
    env: Option<Vec<(String, String)>>,
    /// The tag "Edit a tag" has, and its new name.
    tag: Option<String>,
    rename: String,
    /// A new item of one's own for the tab's menu: its words and command.
    item: (String, String),
    /// The file to import.
    import: String,
    /// The hooks' own lines are shown, to copy.
    lines: bool,
}

struct Draft {
    text: String,
    /// Sent to the file, and what the file said then: shown until the file
    /// says something else, or for a moment.
    sent: Option<(String, std::time::Instant)>,
}

struct ProfileDraft {
    /// The name it had; `None` for a new one.
    was: Option<String>,
    name: String,
    folder: String,
    start: String,
    tags: String,
    panes: Vec<String>,
}

impl ProfileDraft {
    fn of(p: &Profile) -> Self {
        Self { was: Some(p.name.clone()), name: p.name.clone(), folder: p.folder.clone(), start: p.start.clone(), tags: p.tags.join(", "), panes: p.panes.clone() }
    }

    fn profile(&self) -> Profile {
        let tags = self.tags.split(',').filter_map(tsumugi_mux::proto::tag_name).collect();
        Profile { name: self.name.trim().to_owned(), folder: self.folder.trim().to_owned(), start: self.start.clone(), tags, panes: self.panes.clone() }
    }
}

/// What the screen was asked to change.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// One key of `settings.toml`: its table (`None` at the top), its name
    /// and its value as TOML.
    Set(Option<&'static str>, &'static str, String),
    /// One key of a table named at run time (`[tags.colors]`, `[shell.env]`),
    /// the key as TOML writes it; `None` takes it out.
    SetIn(String, String, Option<String>),
    /// Every `[[tags.rule]]`, in place of those there.
    Rules(Vec<TagRule>),
    /// Every `[[menu.session]]`, in place of those there.
    MenuItems(Vec<MenuItem>),
    MuteTag(String, bool),
    RenameTag(String, String),
    TestNotification,
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
    Export,
    Import(String),
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
}

/// The colours and the search's words, for every row.
#[derive(Clone, Copy)]
struct Look<'a> {
    c: Colors,
    pal: &'a Palette,
    /// The search, lower case; empty for none.
    q: &'a str,
}

pub fn show(ui: &mut egui::Ui, pal: &Palette, screen: &mut Screen, seen: &Seen) -> Vec<Change> {
    let c = seen.current;
    let mut out = Vec::new();
    // Esc leaves the screen, unless a control or a key being changed has it.
    let busy = screen.held || ui.memory(|m| m.focused().is_some()) || screen.edit.capturing.is_some();
    if !busy && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        out.push(Change::Close);
    }
    // The pages by the keys, as tabs are elsewhere: Ctrl+Tab and
    // Ctrl+PageDown the next, with Shift or PageUp the one before; Ctrl+F
    // to the search.
    let (next, back, find) = ui.input_mut(|i| {
        let ctrl = egui::Modifiers::CTRL;
        let back = i.consume_key(ctrl | egui::Modifiers::SHIFT, egui::Key::Tab) || i.consume_key(ctrl, egui::Key::PageUp);
        let next = i.consume_key(ctrl, egui::Key::Tab) || i.consume_key(ctrl, egui::Key::PageDown);
        (next, back, i.consume_key(egui::Modifiers::COMMAND, egui::Key::F))
    });
    if next || back {
        let k = Page::ALL.iter().position(|p| *p == screen.page).unwrap_or(0);
        let n = Page::ALL.len();
        screen.page = Page::ALL[if next { (k + 1) % n } else { (k + n - 1) % n }];
        screen.query.clear();
        // The keys to the page's first control with the next Tab.
        ui.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
    }
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, c.bg);
    // The nav down the left: the search, the pages, and the file.
    let nav = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 240.0, rect.bottom()));
    ui.painter().rect_filled(nav, 0.0, c.side);
    ui.painter().line_segment([nav.right_top(), nav.right_bottom()], egui::Stroke::new(1.0, c.border));
    let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink2(egui::vec2(10.0, 16.0))));
    let search = egui::TextEdit::singleline(&mut screen.query)
        .id(egui::Id::new("prefs-search"))
        .hint_text("🔍  Search settings")
        .desired_width(f32::INFINITY)
        .margin(egui::vec2(10.0, 8.0))
        .font(FontId::proportional(12.5));
    let searched = nav_ui.add(search);
    if find {
        searched.request_focus();
    }
    nav_ui.add_space(10.0);
    let q = screen.query.trim().to_lowercase();
    let hits = |p: Page| -> usize {
        if q.is_empty() {
            return 1;
        }
        INDEX.iter().filter(|(at, words)| *at == p && words.to_lowercase().contains(&q)).count() + usize::from(p.title().to_lowercase().contains(&q))
    };
    let shown: Vec<Page> = Page::ALL.into_iter().filter(|p| hits(*p) > 0).collect();
    // The search moves to the first page that has it.
    if !q.is_empty() && !shown.contains(&screen.page) {
        if let Some(first) = shown.first() {
            screen.page = *first;
        }
    }
    if searched.lost_focus() && nav_ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Some(first) = shown.first() {
            screen.page = *first;
        }
    }
    if shown.is_empty() {
        nav_ui.label(RichText::new("Nothing matches").size(12.5).color(c.dim));
    }
    for p in shown {
        let on = screen.page == p;
        // Mouse only: the keys go page to page with Ctrl+Tab, and Tab goes
        // from the search straight into the page.
        let (r, resp) = nav_ui.allocate_exact_size(egui::vec2(nav_ui.available_width(), 32.0), egui::Sense::CLICK);
        if on {
            nav_ui.painter().rect_filled(r, 6.0, c.chosen());
        } else if resp.hovered() {
            nav_ui.painter().rect_filled(r, 6.0, c.hover());
        }
        let color = if on { c.strong() } else { c.dim };
        nav_ui.painter().text(egui::pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, p.title(), FontId::proportional(13.0), color);
        if !q.is_empty() {
            nav_ui.painter().text(egui::pos2(r.right() - 10.0, r.center().y), egui::Align2::RIGHT_CENTER, hits(p).to_string(), FontId::proportional(11.0), c.faint());
        }
        if resp.clicked() {
            screen.page = p;
        }
    }

    // The page.
    let l = Look { c, pal, q: &q };
    let body = egui::Rect::from_min_max(egui::pos2(nav.right() + 1.0, rect.top()), rect.max);
    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body));
    let page = screen.page;
    let edit = &mut screen.edit;
    egui::ScrollArea::vertical().id_salt(("prefs-page", page.title())).auto_shrink([false, false]).show(&mut body_ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 48, right: 48, top: 28, bottom: 40 }).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(820.0));
            ui.label(RichText::new(page.title()).size(24.0).strong().color(c.strong()));
            ui.add_space(2.0);
            ui.label(RichText::new(page.lead()).size(13.5).color(c.dim));
            ui.add_space(16.0);
            match page {
                Page::General => general(ui, l, seen, edit, &mut out),
                Page::Appearance => appearance(ui, l, seen, edit, &mut out),
                Page::Keys => keys(ui, l, seen, edit, &mut out),
                Page::Notifications => notifications(ui, l, seen, edit, &mut out),
                Page::Sessions => sessions(ui, l, seen, edit, &mut out),
                Page::Tags => tags(ui, l, seen, edit, &mut out),
                Page::Theme => theme(ui, l, seen, &mut out),
                Page::Shell => shell(ui, l, seen, edit, &mut out),
                Page::Advanced => advanced(ui, l, seen, edit, &mut out),
            }
        });
    });
    // The file last, so Tab comes to it after the page.
    let foot = egui::Rect::from_min_max(egui::pos2(nav.left() + 10.0, nav.bottom() - 48.0), egui::pos2(nav.right() - 10.0, nav.bottom() - 16.0));
    let file = egui::Button::new(RichText::new("Open settings.toml").size(12.5).color(c.dim)).fill(Color32::TRANSPARENT).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(6.0);
    let file = nav_ui.put(foot, file);
    focus_ring(&nav_ui, &file);
    if file.clicked() {
        out.push(Change::OpenFile);
    }
    screen.held = ui.memory(|m| m.focused().is_some());
    out.retain(|change| match change {
        Change::GoTo(p) => {
            screen.page = *p;
            false
        }
        _ => true,
    });
    out
}

/// A card of rows under a small heading.
fn section(ui: &mut egui::Ui, l: Look, head: &str, rows: impl FnOnce(&mut egui::Ui)) {
    let c = l.c;
    let hit = !l.q.is_empty() && head.to_lowercase().contains(l.q);
    ui.label(RichText::new(head).size(11.0).strong().color(if hit { c.wait } else { c.dim }));
    ui.add_space(6.0);
    egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(18, 2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        rows(ui);
    });
    ui.add_space(22.0);
}

/// The line between two rows of a card, across the whole card.
fn sep(ui: &mut egui::Ui, l: Look) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(r.x_range().expand(18.0), r.center().y, egui::Stroke::new(1.0, l.c.border));
}

/// A row: its label and note on the left, its control on the right. Lit
/// when the search's words are in it.
fn row<R>(ui: &mut egui::Ui, l: Look, label: &str, note: &str, control: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let c = l.c;
    let hit = !l.q.is_empty() && (label.to_lowercase().contains(l.q) || note.to_lowercase().contains(l.q));
    let behind = ui.painter().add(egui::Shape::Noop);
    let mut r = None;
    let shown = ui.horizontal(|ui| {
        ui.set_min_height(48.0);
        // The control first, on the right; the words wrap in what is left,
        // never under it.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r = Some(control(ui));
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    // A label alone sits in the middle of the row, as the
                    // control does.
                    ui.add_space(if note.is_empty() { 15.0 } else { 8.0 });
                    ui.label(RichText::new(label).size(13.0).color(c.strong()));
                    if !note.is_empty() {
                        ui.label(RichText::new(note).size(12.0).color(c.dim));
                    }
                    ui.add_space(6.0);
                });
            });
        });
    });
    if hit {
        let lit = egui::Shape::rect_filled(shown.response.rect.expand2(egui::vec2(10.0, 0.0)), 6.0, c.wait.gamma_multiply(0.14));
        ui.painter().set(behind, lit);
    }
    r.expect("the control ran")
}

/// The design's switch: a pill with a knob, cyan when on.
fn switch(ui: &mut egui::Ui, l: Look, on: bool) -> bool {
    let c = l.c;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 11.0, if on { c.run } else { c.border_strong() });
    let x = if on { rect.right() - 11.0 } else { rect.left() + 11.0 };
    p.circle_filled(egui::pos2(x, rect.center().y), 8.0, if on { c.on_accent() } else { c.dim });
    focus_ring(ui, &resp);
    resp.clicked()
}

/// A ring round the control that has the keys, so Tab shows where it is.
fn focus_ring(ui: &egui::Ui, r: &egui::Response) {
    if r.has_focus() {
        ui.painter().rect_stroke(r.rect.expand(3.0), 8.0, egui::Stroke::new(1.5, crate::chrome::cyan()), egui::StrokeKind::Outside);
    }
}

/// A list to pick one of: the value picked, when it changed.
fn select<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, now: T, options: &[(T, &str)]) -> Option<T> {
    let mut pick = now;
    let shown = options.iter().find(|(v, _)| *v == now).map_or("", |(_, t)| *t).to_owned();
    egui::ComboBox::from_id_salt(("prefs", id)).selected_text(shown).width(190.0).show_ui(ui, |ui| {
        for (v, t) in options {
            ui.selectable_value(&mut pick, *v, *t);
        }
    });
    (pick != now).then_some(pick)
}

/// The design's plain button: an outline, words in the text colour.
fn button(ui: &mut egui::Ui, l: Look, words: &str) -> bool {
    let b = egui::Button::new(RichText::new(words).size(12.5).color(l.c.fg)).fill(Color32::TRANSPARENT).stroke(egui::Stroke::new(1.0, l.c.border_strong())).corner_radius(7.0).min_size(egui::vec2(0.0, 30.0));
    let r = ui.add(b);
    focus_ring(ui, &r);
    r.clicked()
}

fn status(ui: &mut egui::Ui, words: &str, color: Color32) {
    ui.label(RichText::new(words).size(12.5).color(color));
}

/// A one-line field that writes when Enter is pressed or it is left (Esc
/// leaves it as it was): the new words, when they differ.
fn field(ui: &mut egui::Ui, drafts: &mut HashMap<String, Draft>, id: &str, now: &str, hint: &str, width: f32) -> Option<String> {
    let eid = egui::Id::new(("prefs-field", id));
    let focused = ui.memory(|m| m.has_focus(eid));
    if !focused {
        let done = drafts.get(id).is_some_and(|d| match &d.sent {
            Some((was, at)) => now != was || at.elapsed() > std::time::Duration::from_secs(2),
            None => true,
        });
        if done {
            drafts.remove(id);
        }
    }
    let mut text = drafts.get(id).map_or_else(|| now.to_owned(), |d| d.text.clone());
    let edit = egui::TextEdit::singleline(&mut text).id(eid).hint_text(hint).desired_width(width).font(FontId::monospace(12.5)).margin(egui::vec2(10.0, 6.0));
    let resp = ui.add(edit);
    if resp.has_focus() || resp.changed() {
        drafts.insert(id.to_owned(), Draft { text: text.clone(), sent: None });
    }
    if resp.lost_focus() {
        drafts.remove(id);
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) || text == now {
            return None;
        }
        drafts.insert(id.to_owned(), Draft { text: text.clone(), sent: Some((now.to_owned(), std::time::Instant::now())) });
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
        return Some(text);
    }
    None
}

/// A key's cap, as the design draws one.
fn keycap(ui: &mut egui::Ui, l: Look, words: &str, lit: bool) -> egui::Response {
    let c = l.c;
    let stroke = if lit { c.run } else { c.border_strong() };
    let b = egui::Button::new(RichText::new(words).font(FontId::monospace(12.0)).color(c.strong())).fill(c.side).stroke(egui::Stroke::new(1.0, stroke)).corner_radius(6.0);
    ui.add(b)
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
    section(ui, l, "STARTUP", |ui| {
        row(ui, l, "Language", "Menus and messages; only English in this version", |ui| {
            select(ui, "language", "en", &[("en", "English")]);
        });
        sep(ui, l);
        let starts = [(true, "Restore the last sessions"), (false, "Ask (Welcome back)")];
        if let Some(on) = row(ui, l, "On start", "What the window shows first after a restart", |ui| select(ui, "on-start", seen.always_restore, &starts)) {
            out.push(Change::AlwaysRestore(on));
        }
        sep(ui, l);
        let home = if cfg!(windows) { "%USERPROFILE%" } else { "~" };
        if let Some(t) = row(ui, l, "Default folder", "Where a new session starts when no pane is open", |ui| field(ui, &mut edit.drafts, "default-folder", &g.default_folder, home, 200.0)) {
            out.push(Change::Set(Some("general"), "default_folder", cfg::quote(t.trim())));
        }
        sep(ui, l);
        let on = seen.facts.map(|f| f.autostart);
        if row(ui, l, "Start the server at sign-in", "Sessions are ready before you open the window", |ui| match on {
            Some(on) => switch(ui, l, on),
            None => {
                status(ui, "…", l.c.dim);
                false
            }
        }) {
            out.push(Change::Autostart(!on.unwrap_or(false)));
        }
    });
    section(ui, l, "CLOSING", |ui| {
        if row(ui, l, "Keep sessions running when the window closes", "The server keeps them; open the window to get them back. Off: closing the window stops them", |ui| switch(ui, l, g.keep_sessions)) {
            out.push(set("keep_sessions", !g.keep_sessions));
        }
        sep(ui, l);
        if row(ui, l, "Ask before closing a running session", "Only when something is still running", |ui| switch(ui, l, g.ask_before_close)) {
            out.push(set("ask_before_close", !g.ask_before_close));
        }
        sep(ui, l);
        if row(ui, l, "Check for updates", "At each start, from GitHub releases; a note when there is a newer one", |ui| switch(ui, l, g.check_updates)) {
            out.push(set("check_updates", !g.check_updates));
        }
    });
    let k = &seen.settings.clock;
    section(ui, l, "CLOCK", |ui| {
        let set = |key: &'static str, v: bool| Change::Set(Some("clock"), key, v.to_string());
        if row(ui, l, "Show the time in the status bar", "Stays visible when the window is maximized or full screen", |ui| switch(ui, l, k.show)) {
            out.push(set("show", !k.show));
        }
        sep(ui, l);
        if let Some(h) = row(ui, l, "Time format", "", |ui| select(ui, "clock-hours", k.hour24, &[(true, "14:32 (24-hour)"), (false, "2:32 PM (12-hour)")])) {
            out.push(set("hour24", h));
        }
        sep(ui, l);
        if row(ui, l, "Show the date", "Next to the time", |ui| switch(ui, l, k.date)) {
            out.push(set("date", !k.date));
        }
        sep(ui, l);
        let formats: Vec<(&str, &str)> = DATE_FORMATS.iter().map(|f| (*f, *f)).collect();
        if let Some(f) = row(ui, l, "Date format", "", |ui| select(ui, "clock-date", k.date_format.as_str(), &formats)) {
            out.push(Change::Set(Some("clock"), "date_format", cfg::quote(f)));
        }
        sep(ui, l);
        if row(ui, l, "Show the weekday", "", |ui| switch(ui, l, k.weekday)) {
            out.push(set("weekday", !k.weekday));
        }
    });
}

fn appearance(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let f = &seen.settings.font;
    let a = &seen.settings.appearance;
    section(ui, l, "TEXT", |ui| {
        let mut families: Vec<(&str, &str)> = vec![("", "Automatic")];
        families.extend(seen.font_names.iter().map(|n| (n.as_str(), n.as_str())));
        if !f.family.is_empty() && !seen.font_names.contains(&f.family) {
            families.push((f.family.as_str(), f.family.as_str()));
        }
        if let Some(family) = row(ui, l, "Font", "Monospace fonts installed; Automatic is the Nerd Font found, else the built-in one", |ui| select(ui, "font-family", f.family.as_str(), &families)) {
            out.push(Change::Set(Some("font"), "family", cfg::quote(family)));
        }
        let using = match &seen.font_file {
            Some(file) => {
                let styles: Vec<&str> = ["bold", "italic", "bold italic"].into_iter().zip(seen.faces).filter(|(_, on)| *on).map(|(s, _)| s).collect();
                let styles = if styles.is_empty() { "no bold or italic file beside it".to_owned() } else { format!("with {}", styles.join(", ")) };
                format!("{file} ({styles})")
            }
            None if !f.family.is_empty() => format!("`{}` was not found: the built-in font", f.family),
            None => "The built-in font".to_owned(),
        };
        ui.label(RichText::new(using).size(11.5).color(c.dim));
        ui.add_space(6.0);
        sep(ui, l);
        if let Some(t) = row(ui, l, "Size", "In points, 8 to 32", |ui| field(ui, &mut edit.drafts, "font-size", &f.size.to_string(), "13", 80.0)) {
            if let Ok(v) = t.trim().parse::<f32>() {
                out.push(Change::Set(Some("font"), "size", v.clamp(8.0, 32.0).to_string()));
            }
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "Line height", "The rows' height for the font's own, 0.8 to 2", |ui| field(ui, &mut edit.drafts, "line-height", &format!("{:.2}", f.line_height), "1.00", 80.0)) {
            if let Ok(v) = t.trim().parse::<f32>() {
                out.push(Change::Set(Some("font"), "line_height", format!("{:.2}", v.clamp(0.8, 2.0))));
            }
        }
        sep(ui, l);
        if row(ui, l, "Ligatures", "Draw -> and != as single shapes, in a font that has them (Fira Code, JetBrains Mono, Cascadia Code)", |ui| switch(ui, l, f.ligatures)) {
            out.push(Change::Set(Some("font"), "ligatures", (!f.ligatures).to_string()));
        }
        sep(ui, l);
        let found = if seen.nerd { "A Nerd Font is installed" } else { "No Nerd Font is installed: the marks are drawn" };
        if row(ui, l, "Nerd Font icons", &format!("The branch mark and others as the font's own icons. {found}"), |ui| switch(ui, l, a.nerd_icons)) {
            out.push(Change::Set(Some("appearance"), "nerd_icons", (!a.nerd_icons).to_string()));
        }
    });
    let w = &seen.settings.window;
    section(ui, l, "WINDOW", |ui| {
        let own = w.own_titlebar();
        let note = if cfg!(target_os = "macos") { "The band runs under the traffic lights (when the window opens next)" } else { "The band is the title bar, with its own buttons; off is the system's frame" };
        if row(ui, l, "tsumugi's own title bar", note, |ui| switch(ui, l, own)) {
            out.push(Change::Set(Some("window"), "titlebar", cfg::quote(if own { "system" } else { "tsumugi" })));
        }
        if cfg!(windows) || cfg!(target_os = "macos") {
            sep(ui, l);
            let (note, kinds): (&str, &[(&str, &str)]) = if cfg!(windows) {
                ("Mica and Acrylic on Windows 11 (when the window opens next)", &[("none", "None"), ("mica", "Mica"), ("acrylic", "Acrylic")])
            } else {
                ("The desktop shows through the band, sidebar and status bar (when the window opens next)", &[("none", "None"), ("vibrancy", "Vibrancy")])
            };
            if let Some(m) = row(ui, l, "Window material", note, |ui| select(ui, "material", w.material.as_str(), kinds)) {
                out.push(Change::Set(Some("window"), "material", cfg::quote(m)));
            }
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "Dim unfocused panes", "How much darker the panes without the keys get, in %", |ui| field(ui, &mut edit.drafts, "dim", &a.dim.to_string(), "35", 80.0)) {
            if let Ok(v) = t.trim().trim_end_matches('%').trim().parse::<u8>() {
                out.push(Change::Set(Some("appearance"), "dim", v.min(90).to_string()));
            }
        }
        sep(ui, l);
        let shapes = [
            ("block-blink", "Block, blinking"),
            ("block", "Block"),
            ("bar-blink", "Bar, blinking"),
            ("bar", "Bar"),
            ("underline-blink", "Underline, blinking"),
            ("underline", "Underline"),
        ];
        if let Some(s) = row(ui, l, "Cursor", "In the pane with the keys; the others show an outline", |ui| select(ui, "cursor", a.cursor.as_str(), &shapes)) {
            out.push(Change::Set(Some("appearance"), "cursor", cfg::quote(s)));
        }
        sep(ui, l);
        if row(ui, l, "Animations", "Breathing rings, the running line, tab moves", |ui| switch(ui, l, a.animations)) {
            out.push(Change::Set(Some("appearance"), "animations", (!a.animations).to_string()));
        }
    });
}

fn keys(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    use crate::keys::{label, Action, NAMED};
    let c = l.c;
    // A key being changed takes the next press: Esc leaves it as it was.
    if let Some(name) = edit.capturing {
        let pressed = ui.input_mut(|i| {
            // A modifier alone is not a key yet: wait for the one it holds.
            let found = i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if !crate::keys::is_modifier(*key) => Some((*key, *modifiers)),
                _ => None,
            });
            if let Some((k, m)) = found {
                i.consume_key(m, k);
            }
            found
        });
        match pressed {
            Some((egui::Key::Escape, m)) if !m.any() => {
                edit.capturing = None;
                edit.clash = None;
            }
            Some((k, m)) => {
                let chord = crate::keys::Chord::pressed(k, m);
                let action = NAMED.iter().find(|(_, n, ..)| *n == name).map(|(a, ..)| *a);
                match action.and_then(|a| crate::keys::clash(&chord, a)) {
                    Some(why) => edit.clash = Some((name, why, chord.label())),
                    None => {
                        out.push(Change::Set(Some("keys"), name, cfg::quote(&chord.label())));
                        edit.capturing = None;
                        edit.clash = None;
                    }
                }
            }
            None => {}
        }
    }
    let mac = crate::keys::mac();
    let preset = if mac { "macOS" } else { "Windows Terminal" };
    section(ui, l, "PRESET", |ui| {
        row(ui, l, "Preset", "The set of keys to start from", |ui| {
            select(ui, "preset", preset, &[(preset, preset)]);
        });
        sep(ui, l);
        let on = seen.settings.general.cmd_on_mac;
        let note = if cfg!(target_os = "macos") { "Ctrl+Shift+T becomes Cmd+T, and so on" } else { "Ctrl+Shift+T becomes Cmd+T, and so on (on a Mac)" };
        if row(ui, l, "Use Cmd on macOS", note, |ui| switch(ui, l, on)) {
            out.push(Change::Set(Some("general"), "cmd_on_mac", (!on).to_string()));
        }
    });
    let fixed = |win: &'static str, m: &'static str| if mac { m } else { win };
    let mut changeable = |ui: &mut egui::Ui, a: Action| {
        let Some((_, name, win, m)) = NAMED.iter().find(|(x, ..)| *x == a) else { return };
        let own = if mac { *m } else { *win };
        let now = label(a);
        let clash = edit.clash.clone().filter(|(n, ..)| n == name);
        let note = match &clash {
            Some((_, why, _)) => format!("{why}. Press another, or use it anyway"),
            None if now != own => format!("Its own: {own}"),
            None => String::new(),
        };
        row(ui, l, crate::keys::title(a), &note, |ui| {
            if let Some((_, _, chord)) = &clash {
                if ui.small_button("Use it anyway").clicked() {
                    out.push(Change::Set(Some("keys"), name, cfg::quote(chord)));
                    edit.capturing = None;
                    edit.clash = None;
                }
            } else if now != own && ui.small_button("Its own").clicked() {
                out.push(Change::Set(Some("keys"), name, cfg::quote(own)));
            }
            let waiting = edit.capturing == Some(*name);
            let text = if waiting { "Press a key… (Esc: leave it)".to_owned() } else { now };
            if keycap(ui, l, &text, waiting).on_hover_text("Click, then press the key to use (Esc: leave it)").clicked() {
                edit.capturing = Some(name);
                edit.clash = None;
            }
        });
    };
    let fixed_row = |ui: &mut egui::Ui, title: &str, key: &str| {
        row(ui, l, title, "", |ui| {
            ui.label(RichText::new(key).font(FontId::monospace(12.0)).color(c.dim));
        });
    };
    section(ui, l, "SESSIONS", |ui| {
        for (k, a) in [Action::NewTab, Action::CloseTab, Action::Rename, Action::Duplicate, Action::NextTab, Action::PrevTab, Action::NextWaiting, Action::Waiting, Action::Search, Action::Input, Action::Rail, Action::Settings].into_iter().enumerate() {
            if k > 0 {
                sep(ui, l);
            }
            changeable(ui, a);
        }
        sep(ui, l);
        fixed_row(ui, "The Nth tab", fixed("Ctrl+Alt+1 … 9", "Cmd+1 … 9"));
    });
    section(ui, l, "PANES", |ui| {
        changeable(ui, Action::SplitRight);
        sep(ui, l);
        changeable(ui, Action::SplitDown);
        sep(ui, l);
        fixed_row(ui, "Move between panes", fixed("Alt+Arrows", "Cmd+Option+Arrows"));
        sep(ui, l);
        fixed_row(ui, "Resize the pane", fixed("Alt+Shift+Arrows", "Cmd+Ctrl+Arrows"));
        sep(ui, l);
        changeable(ui, Action::Zoom);
    });
    ui.label(RichText::new("A key is written to [keys] in settings.toml; \"none\" there gives it back to the shell.").size(12.0).color(c.dim));
}

/// The sounds by what the list says.
const SOUND_NAMES: [(&str, &str); 4] = [("chime", "Soft chime"), ("low", "Low tone"), ("alert", "Alert"), ("default", "System default")];

fn notifications(ui: &mut egui::Ui, l: Look, seen: &Seen, _edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let n = &seen.settings.notify;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, l, "Send a test") {
                out.push(Change::TestNotification);
            }
            if button(ui, l, "Reset to defaults") {
                let d = cfg::Notify::default();
                for (key, list) in [("system", d.system), ("taskbar", d.taskbar), ("flash", d.flash), ("sound", d.sound)] {
                    out.push(Change::Set(Some("notify"), key, cfg::quote_list(&list)));
                }
            }
        });
    });
    ui.add_space(8.0);
    section(ui, l, "WHEN A SESSION…", |ui| {
        // The design's table: the way to tell, then a column per state, the
        // label's column 1.6 times as wide as each state's, a line between
        // rows.
        let states = [("waiting", "waits for you", c.wait), ("error", "fails", c.err), ("done", "finishes", c.done)];
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
            let text = ui.fonts_mut(|f| f.layout_no_wrap(words.to_string(), FontId::proportional(13.0), crate::chrome::ink(*color)));
            let left = r.center().x - (text.size().x + 14.0) / 2.0;
            ui.painter().circle_filled(egui::pos2(left + 4.0, r.center().y), 4.0, *color);
            ui.painter().galley(egui::pos2(left + 14.0, r.center().y - text.size().y / 2.0), text, *color);
        }
        for (key, label, note, list) in [
            ("system", "System notification", "The OS's own, which a click brings back here", &n.system),
            ("taskbar", "Taskbar count", "A number on the window's taskbar button", &n.taskbar),
            ("flash", "Taskbar flash", "The button lights up until you look", &n.flash),
            ("sound", "Sound", "Once, the sound picked below", &n.sound),
        ] {
            sep(ui, l);
            let (row, _) = ui.allocate_exact_size(egui::vec2(width, 56.0), egui::Sense::hover());
            let cols = columns(row);
            let words = egui::Rect::from_center_size(cols[0].center(), egui::vec2(cols[0].width(), 36.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(words), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(label).size(13.0).color(c.strong()));
                ui.add(egui::Label::new(RichText::new(note).size(12.0).color(c.dim)).truncate());
            });
            for ((word, _, _), r) in states.iter().zip(&cols[1..]) {
                let on = list.iter().any(|w| w == word);
                let at = egui::Rect::from_center_size(r.center(), egui::vec2(40.0, 22.0));
                let flipped = ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| switch(ui, l, on)).inner;
                if flipped {
                    let mut next: Vec<String> = list.iter().filter(|w| w != word).cloned().collect();
                    if !on {
                        next.push(word.to_string());
                    }
                    out.push(Change::Set(Some("notify"), key, cfg::quote_list(&next)));
                }
            }
        }
    });
    section(ui, l, "DETAILS", |ui| {
        let mut lengths: Vec<(u64, String)> = [(0, "Any length"), (30, "30 s or more"), (60, "1 min or more"), (300, "5 min or more"), (600, "10 min or more")].iter().map(|(v, t)| (*v, t.to_string())).collect();
        if !lengths.iter().any(|(v, _)| *v == n.long_run) {
            lengths.push((n.long_run, format!("{} s or more", n.long_run)));
        }
        let lengths: Vec<(u64, &str)> = lengths.iter().map(|(v, t)| (*v, t.as_str())).collect();
        if let Some(v) = row(ui, l, "Tell about a finish", "Only after a run this long, so a quick command says nothing", |ui| select(ui, "long-run", n.long_run, &lengths)) {
            out.push(Change::Set(Some("notify"), "long_run", v.to_string()));
        }
        for (key, label, now) in [("sound_waiting", "Sound for waits", &n.sound_waiting), ("sound_error", "Sound for fails", &n.sound_error)] {
            sep(ui, l);
            let picked = row(ui, l, label, "", |ui| {
                if ui.small_button("▶").on_hover_text("Play it").clicked() {
                    out.push(Change::PlaySound(now.clone()));
                }
                select(ui, key, now.as_str(), &SOUND_NAMES)
            });
            if let Some(s) = picked {
                out.push(Change::Set(Some("notify"), key, cfg::quote(s)));
            }
        }
        sep(ui, l);
        let note = if cfg!(windows) { "Nothing while focus mode, a presentation or a full-screen game is on; the bell still keeps them" } else { "Windows says when focus mode is on; this system does not tell tsumugi yet" };
        if row(ui, l, "Respect Windows focus mode", note, |ui| switch(ui, l, n.focus_mode)) {
            out.push(Change::Set(Some("notify"), "focus_mode", (!n.focus_mode).to_string()));
        }
    });
    section(ui, l, "QUIET TAGS", |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Sessions with these tags tell you in the bell only. Click one to quiet it.").size(12.0).color(c.dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.link("Edit tags…").clicked() {
                    out.push(Change::GoTo(Page::Tags));
                }
            });
        });
        ui.add_space(6.0);
        if seen.tags.is_empty() {
            ui.label(RichText::new("No session has a tag yet.").color(c.dim));
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
            let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::click());
            let chip = crate::chrome::tag_chip(ui.painter(), r.min, t, q);
            if q {
                ui.painter().line_segment([chip.left_center(), chip.right_center()], egui::Stroke::new(1.0, c.dim));
            }
            if resp.on_hover_text(if q { "Quiet: click to tell again" } else { "Click to quiet" }).clicked() {
                click(t.clone(), q);
            }
        }
    });
}

const STARTS: [(&str, &str); 3] = [("claude", "Claude Code"), ("resume", "Resume the last conversation"), ("shell", "Shell")];

fn profile_words(p: &Profile) -> String {
    let short = |s: &str| match s {
        "resume" => "Resume",
        "shell" => "Shell",
        _ => "Claude Code",
    };
    let mut kinds = vec![short(&p.start)];
    kinds.extend(p.panes.iter().map(|s| short(s)));
    let mut s = kinds.join(" + ");
    if !p.panes.is_empty() {
        s.push_str(", split");
    }
    s.push_str(&format!(" · {}", p.folder));
    if !p.tags.is_empty() {
        s.push_str(&format!(" · {}", p.tags.join(", ")));
    }
    s
}

fn sessions(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let s = &seen.settings.sessions;
    section(ui, l, "NEW SESSIONS", |ui| {
        if let Some(v) = row(ui, l, "Default program", "What the new-session dialog has picked first", |ui| select(ui, "start", s.start.as_str(), &STARTS)) {
            out.push(Change::Set(Some("sessions"), "start", cfg::quote(v)));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "Claude Code command", "The program, or a path to it", |ui| field(ui, &mut edit.drafts, "claude", &s.claude, "claude", 200.0)) {
            let t = t.trim();
            out.push(Change::Set(Some("sessions"), "claude", cfg::quote(if t.is_empty() { "claude" } else { t })));
        }
        sep(ui, l);
        let note = format!("Runs {} --resume in each restored session that ran Claude Code", s.claude);
        if row(ui, l, "Resume conversations after a restart", &note, |ui| switch(ui, l, s.resume)) {
            out.push(Change::Set(Some("sessions"), "resume", (!s.resume).to_string()));
        }
    });
    section(ui, l, "WAITING", |ui| {
        let now = s.quiet.map_or_else(String::new, |q| q.to_string());
        let quiet = row(ui, l, "Call a quiet session \u{201c}probably waiting\u{201d} after", "Seconds without output while a program runs; 0 turns it off; empty is 10", |ui| field(ui, &mut edit.drafts, "quiet", &now, "10", 80.0));
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
        if let Some(v) = row(ui, l, "Sort", "Also the button beside SESSIONS", |ui| select(ui, "sort", seen.sort, &sorts)) {
            out.push(Change::Sort(v));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "Offer compact rows above", "Sessions open at once", |ui| field(ui, &mut edit.drafts, "compact", &s.compact_after.to_string(), "12", 80.0)) {
            if let Ok(v) = t.trim().parse::<usize>() {
                out.push(Change::Set(Some("sessions"), "compact_after", v.to_string()));
            }
        }
    });
    section(ui, l, "PROFILES", |ui| {
        for p in seen.profiles {
            let (e, d) = row(ui, l, &p.name, &profile_words(p), |ui| (button(ui, l, "Delete"), button(ui, l, "Edit")));
            if d {
                edit.profile = Some(ProfileDraft::of(p));
            }
            if e {
                out.push(Change::DeleteProfile(p.name.clone()));
            }
            sep(ui, l);
        }
        if row(ui, l, "New profile", "Or tick \u{201c}Save as a profile\u{201d} in the new-session dialog", |ui| button(ui, l, "Add")) {
            let home = cfg::home().map(|h| h.display().to_string()).unwrap_or_default();
            edit.profile = Some(ProfileDraft { was: None, name: String::new(), folder: home, start: "claude".into(), tags: String::new(), panes: Vec::new() });
        }
        let mut done = None;
        if let Some(d) = &mut edit.profile {
            sep(ui, l);
            ui.add_space(10.0);
            let head = if d.was.is_some() { "Edit the profile" } else { "A new profile" };
            ui.label(RichText::new(head).size(13.0).strong().color(c.strong()));
            egui::Grid::new("profile-edit").num_columns(2).spacing(egui::vec2(14.0, 8.0)).show(ui, |ui| {
                ui.label(RichText::new("Name").color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                ui.end_row();
                ui.label(RichText::new("Folder").color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.folder).desired_width(360.0).font(FontId::monospace(12.5)));
                ui.end_row();
                ui.label(RichText::new("Starts").color(c.dim));
                if let Some(v) = select(ui, "profile-start", d.start.as_str(), &STARTS) {
                    d.start = v.to_owned();
                }
                ui.end_row();
                ui.label(RichText::new("Tags").color(c.dim));
                ui.add(egui::TextEdit::singleline(&mut d.tags).hint_text("comma between").desired_width(260.0));
                ui.end_row();
                let mut gone = None;
                for (k, pane) in d.panes.iter_mut().enumerate() {
                    ui.label(RichText::new(["Pane on the right", "Pane below it", "Pane below the first"][k.min(2)]).color(c.dim));
                    ui.horizontal(|ui| {
                        if let Some(v) = select(ui, &format!("profile-pane-{k}"), pane.as_str(), &STARTS) {
                            *pane = v.to_owned();
                        }
                        if ui.small_button("Remove").clicked() {
                            gone = Some(k);
                        }
                    });
                    ui.end_row();
                }
                if let Some(k) = gone {
                    d.panes.remove(k);
                }
            });
            if d.panes.len() < 3 && ui.small_button("Add a pane").clicked() {
                d.panes.push("shell".into());
            }
            ui.add_space(6.0);
            let p = d.profile();
            let taken = seen.profiles.iter().any(|o| o.name == p.name && Some(&o.name) != d.was.as_ref());
            ui.horizontal(|ui| {
                let ok = !p.name.is_empty() && !p.folder.is_empty() && !taken;
                if ui.add_enabled(ok, egui::Button::new("Save")).clicked() {
                    done = Some(Some(Change::SaveProfile(d.was.clone(), p)));
                }
                if ui.button("Cancel").clicked() {
                    done = Some(None);
                }
                if taken {
                    ui.label(RichText::new("Another profile has that name").size(12.0).color(c.err));
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
    section(ui, l, "OPEN WITH", |ui| {
        if let Some(t) = row(ui, l, "Editor command", "The tab menu's \u{201c}Open in the editor\u{201d}; {folder} is the session's folder", |ui| field(ui, &mut edit.drafts, "editor", &o.editor, "code {folder}", 220.0)) {
            out.push(Change::Set(Some("open"), "editor", cfg::quote(&t)));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "filer command", "The tab menu's \u{201c}Open the folder in filer\u{201d}", |ui| field(ui, &mut edit.drafts, "filer", &o.filer, "filer {folder}", 220.0)) {
            out.push(Change::Set(Some("open"), "filer", cfg::quote(&t)));
        }
    });
    tab_menu(ui, l, seen, edit, out);
}

/// The tab's right-click menu: which items show, their order, and items of
/// one's own.
fn tab_menu(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let m = &seen.settings.menu;
    let words = cfg::menu_words(&m.order);
    section(ui, l, "TAB MENU", |ui| {
        for (k, word) in words.iter().enumerate() {
            if k > 0 {
                sep(ui, l);
            }
            let shown = !m.hide.iter().any(|h| h == word);
            let (up, down, flip) = row(ui, l, cfg::menu_label(word), "", |ui| {
                let flip = switch(ui, l, shown);
                ui.add_space(8.0);
                let down = ui.add_enabled(k + 1 < words.len(), egui::Button::new("↓").small()).on_hover_text("Lower in the menu").clicked();
                let up = ui.add_enabled(k > 0, egui::Button::new("↑").small()).on_hover_text("Higher in the menu").clicked();
                (up, down, flip)
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
            if row(ui, l, &item.name, &item.command, |ui| button(ui, l, "Remove")) {
                let mut next = m.session.clone();
                next.remove(k);
                out.push(Change::MenuItems(next));
            }
        }
        sep(ui, l);
        ui.add_space(10.0);
        ui.label(RichText::new("An item of your own, before \u{201c}Close the session\u{201d}: {folder} and {session} go into the command, which the system's shell runs.").size(12.0).color(c.dim));
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut edit.item.0).hint_text("Its words").desired_width(160.0));
            ui.add(egui::TextEdit::singleline(&mut edit.item.1).hint_text("lazygit -p {folder}").desired_width(260.0).font(FontId::monospace(12.5)));
            let ok = !edit.item.0.trim().is_empty() && !edit.item.1.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new("Add")).clicked() {
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
    section(ui, l, "AUTOMATIC TAGS", |ui| {
        for (k, r) in t.rule.iter().enumerate() {
            let what = match (r.folder.is_empty(), r.branch.is_empty()) {
                (false, true) => format!("Folder {}", r.folder),
                (true, false) => format!("Branch {}", r.branch),
                _ => format!("Folder {} on branch {}", r.folder, r.branch),
            };
            let gone = row(ui, l, &what, "", |ui| {
                let gone = button(ui, l, "Remove");
                ui.add_space(8.0);
                let w = ui.fonts_mut(|f| f.layout_no_wrap(r.tag.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::hover());
                crate::chrome::tag_chip(ui.painter(), rect.min, &r.tag, false);
                gone
            });
            if gone {
                let mut next = t.rule.clone();
                next.remove(k);
                out.push(Change::Rules(next));
            }
            sep(ui, l);
        }
        row(ui, l, "New rule", "A folder (ending in * for each folder in it) or a branch pattern, then a tag ({name}: the folder's name)", |ui| {
            let (branch, pattern, tag) = &mut edit.rule;
            let ok = !pattern.trim().is_empty() && !tag.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new("Add")).clicked() {
                let mut next = t.rule.clone();
                let mut rule = TagRule { tag: tag.trim().to_owned(), ..TagRule::default() };
                if *branch {
                    rule.branch = pattern.trim().to_owned();
                } else {
                    rule.folder = pattern.trim().to_owned();
                }
                next.push(rule);
                out.push(Change::Rules(next));
                *pattern = String::new();
                *tag = String::new();
            }
            ui.add(egui::TextEdit::singleline(tag).hint_text("tag").desired_width(90.0));
            let hint = if *branch { "claude/*" } else { "~/dev/*" };
            ui.add(egui::TextEdit::singleline(pattern).hint_text(hint).desired_width(150.0).font(FontId::monospace(12.5)));
            if let Some(b) = select(ui, "rule-kind", *branch, &[(false, "Folder"), (true, "Branch")]) {
                *branch = b;
            }
        });
    });
    let mut known: Vec<String> = seen.tags.to_vec();
    for name in t.rule.iter().map(|r| &r.tag).chain(t.colors.keys()).filter(|n| !n.contains('{')) {
        if !known.contains(name) {
            known.push(name.clone());
        }
    }
    section(ui, l, "COLOURS", |ui| {
        row(ui, l, "Colour new tags", "Picked from the tag's name, so a tag is the same colour everywhere", |ui| {
            select(ui, "tag-colour-kind", "auto", &[("auto", "Automatic")]);
        });
        sep(ui, l);
        row(ui, l, "Tags per session", "The title bar and tabs show 3, then +N", |ui| status(ui, &format!("At most {}", tsumugi_mux::proto::MAX_TAGS), c.fg));
        sep(ui, l);
        let names: Vec<(&str, &str)> = known.iter().map(|n| (n.as_str(), n.as_str())).collect();
        let now = edit.tag.clone().unwrap_or_default();
        let picked = row(ui, l, "Edit a tag", "Rename, recolour, quiet", |ui| {
            if names.is_empty() {
                status(ui, "No tags yet", c.dim);
                None
            } else {
                let mut all = vec![("", "Choose…")];
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
                let (r, _) = ui.allocate_exact_size(egui::vec2(ui.fonts_mut(|f| f.layout_no_wrap(tag.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0, 16.0), egui::Sense::hover());
                crate::chrome::tag_chip(ui.painter(), r.min, &tag, false);
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut edit.rename).desired_width(140.0));
                let new = tsumugi_mux::proto::tag_name(&edit.rename);
                if ui.add_enabled(new.as_ref().is_some_and(|n| *n != tag), egui::Button::new("Rename")).clicked() {
                    if let Some(new) = new {
                        out.push(Change::RenameTag(tag.clone(), new.clone()));
                        edit.tag = Some(new);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Colour").color(c.dim));
                let own = t.colors.get(&tag);
                if ui.selectable_label(own.is_none(), "Automatic").clicked() && own.is_some() {
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
            if row(ui, l, "Quiet", "Its sessions tell you in the bell only", |ui| switch(ui, l, quiet)) {
                out.push(Change::MuteTag(tag.clone(), !quiet));
            }
        }
    });
}

fn shell(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let sh = &seen.settings.shell;
    section(ui, l, "SHELL", |ui| {
        let auto = format!("Automatic ({})", tsumugi_pane::shell_label(None));
        let mut shells: Vec<(&str, &str)> = vec![("", auto.as_str())];
        if let Some(f) = seen.facts {
            shells.extend(f.shells.iter().map(|(label, program)| (program.as_str(), label.as_str())));
        }
        if !sh.program.is_empty() && !shells.iter().any(|(p, _)| *p == sh.program) {
            shells.push((sh.program.as_str(), sh.program.as_str()));
        }
        if let Some(p) = row(ui, l, "Default shell", "What a session runs when it is not Claude Code; a program or a path", |ui| select(ui, "shell", sh.program.as_str(), &shells)) {
            out.push(Change::Set(Some("shell"), "program", cfg::quote(p)));
        }
        sep(ui, l);
        let args = sh.args.join(" ");
        if let Some(t) = row(ui, l, "Arguments", "Separated by spaces", |ui| field(ui, &mut edit.drafts, "shell-args", &args, "-NoLogo", 200.0)) {
            let list: Vec<String> = t.split_whitespace().map(str::to_owned).collect();
            out.push(Change::Set(Some("shell"), "args", cfg::quote_list(&list)));
        }
        sep(ui, l);
        let words = if sh.env.is_empty() { "Edit".to_owned() } else { format!("Edit ({})", sh.env.len()) };
        if row(ui, l, "Environment", "Variables every new session gets", |ui| button(ui, l, &words)) {
            edit.env = match edit.env {
                Some(_) => None,
                None => Some(sh.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            };
        }
        let mut done = false;
        if let Some(vars) = &mut edit.env {
            let mut gone = None;
            for (k, (name, value)) in vars.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(name).hint_text("NAME").desired_width(160.0).font(FontId::monospace(12.5)));
                    ui.label("=");
                    ui.add(egui::TextEdit::singleline(value).hint_text("value").desired_width(280.0).font(FontId::monospace(12.5)));
                    if ui.small_button("Remove").clicked() {
                        gone = Some(k);
                    }
                });
            }
            if let Some(k) = gone {
                vars.remove(k);
            }
            ui.horizontal(|ui| {
                if ui.small_button("Add a variable").clicked() {
                    vars.push((String::new(), String::new()));
                }
                let names: Vec<&str> = vars.iter().map(|(n, _)| n.trim()).filter(|n| !n.is_empty()).collect();
                let bad = names.iter().any(|n| n.contains(['=', ' ']));
                if ui.add_enabled(!bad, egui::Button::new("Save")).clicked() {
                    for old in sh.env.keys().filter(|k| !names.contains(&k.as_str())) {
                        out.push(Change::SetIn("shell.env".into(), toml_key(old), None));
                    }
                    for (name, value) in vars.iter().filter(|(n, _)| !n.trim().is_empty()) {
                        if sh.env.get(name.trim()) != Some(value) {
                            out.push(Change::SetIn("shell.env".into(), toml_key(name.trim()), Some(cfg::quote(value))));
                        }
                    }
                    done = true;
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
                if bad {
                    ui.label(RichText::new("A name has no = or space").size(12.0).color(c.err));
                }
            });
            ui.add_space(8.0);
        }
        if done {
            edit.env = None;
        }
    });
    section(ui, l, "HOOKS", |ui| {
        let facts = seen.facts;
        let hooks = facts.map(|f| f.hooks);
        let flip = row(ui, l, "Claude Code hooks", "Notification and Stop, in ~/.claude/settings.json: Claude Code says when it waits or is done, and which conversation to resume", |ui| {
            let flip = match hooks {
                Some(on) => button(ui, l, if on { "Remove" } else { "Add" }),
                None => false,
            };
            ui.add_space(8.0);
            match hooks {
                Some(true) => status(ui, "Installed", c.done),
                Some(false) => status(ui, "Not installed", c.dim),
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
        let note = format!("Prompt marks (OSC 133) and folder (OSC 7) for {}, in its profile", if shown.is_empty() { "the shell" } else { &shown });
        let hook = facts.map(|f| f.shell_hook);
        let flip = row(ui, l, "Shell integration", &note, |ui| {
            let flip = match hook {
                Some(Some(on)) => button(ui, l, if on { "Remove" } else { "Install" }),
                _ => false,
            };
            ui.add_space(8.0);
            match hook {
                Some(Some(true)) => status(ui, "Installed", c.done),
                Some(Some(false)) => status(ui, "Not installed", c.dim),
                Some(None) => status(ui, &format!("No hook for {shown}"), c.dim),
                None => status(ui, "…", c.dim),
            }
            flip
        });
        if flip {
            out.push(Change::ShellHook(name.clone(), !matches!(hook, Some(Some(true)))));
        }
        sep(ui, l);
        if row(ui, l, "The lines themselves", "To add them by hand, or somewhere else", |ui| button(ui, l, if edit.lines { "Hide" } else { "Show" })) {
            edit.lines = !edit.lines;
        }
        if edit.lines {
            let json = r#"{
  "hooks": {
    "Notification": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --stdin" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --state done" }] }]
  }
}"#;
            ui.label(RichText::new(json).font(FontId::monospace(11.5)).color(c.fg));
            if ui.small_button("Copy").clicked() {
                out.push(Change::Copy(json.to_owned()));
            }
            ui.add_space(6.0);
            let line = if cfg!(windows) { "tsumugi shell-hook pwsh >> $PROFILE" } else { "tsumugi shell-hook bash >> ~/.bashrc" };
            ui.label(RichText::new(line).font(FontId::monospace(12.0)).color(c.fg));
            if ui.small_button("Copy ").clicked() {
                out.push(Change::Copy(line.to_owned()));
            }
            ui.add_space(8.0);
        }
    });
    if cfg!(windows) {
        section(ui, l, "WINDOWS", |ui| {
            row(ui, l, "ConPTY", "The console host tsumugi talks to", |ui| match seen.facts.map(|f| f.conpty) {
                Some(true) => status(ui, "Bundled", c.done),
                Some(false) => status(ui, "The system's (older: lazygit and others can break)", c.wait),
                None => status(ui, "…", c.dim),
            });
        });
    }
}

fn advanced(ui: &mut egui::Ui, l: Look, seen: &Seen, edit: &mut Edit, out: &mut Vec<Change>) {
    let c = l.c;
    let a = &seen.settings.advanced;
    section(ui, l, "SERVER", |ui| {
        row(ui, l, "Mux server", &seen.address, |ui| status(ui, &format!("Running · {}", seen.server_up), c.done));
        sep(ui, l);
        if row(ui, l, "Restart the server", "Sessions are saved and come back around it", |ui| button(ui, l, "Restart")) {
            out.push(Change::RestartServer);
        }
    });
    section(ui, l, "DRAWING", |ui| {
        let mut backends = vec![("auto", "Auto"), ("gl", "GL"), ("vulkan", "Vulkan")];
        if cfg!(windows) {
            backends.push(("dx12", "DirectX 12"));
        }
        if cfg!(target_os = "macos") {
            backends.push(("metal", "Metal"));
        }
        if !backends.iter().any(|(b, _)| *b == a.backend) {
            backends.push((a.backend.as_str(), a.backend.as_str()));
        }
        let note = "Auto picks GL on Windows (AMD drivers spin a core under Vulkan); when the window opens next";
        if let Some(b) = row(ui, l, "Graphics backend", note, |ui| select(ui, "backend", a.backend.as_str(), &backends)) {
            out.push(Change::Set(Some("advanced"), "backend", cfg::quote(b)));
        }
        sep(ui, l);
        if let Some(t) = row(ui, l, "Scrollback", "Lines kept per pane, 100 to 1 000 000; for new sessions", |ui| field(ui, &mut edit.drafts, "scrollback", &a.scrollback.to_string(), "10000", 100.0)) {
            if let Ok(v) = t.trim().replace(['_', ',', ' '], "").parse::<usize>() {
                out.push(Change::Set(Some("advanced"), "scrollback", v.clamp(100, 1_000_000).to_string()));
            }
        }
    });
    section(ui, l, "FILES", |ui| {
        if row(ui, l, "Log what each pane sends and receives", "For bug reports: pane-logs beside the saved tabs, for new sessions. Off by default", |ui| switch(ui, l, a.pane_log)) {
            out.push(Change::Set(Some("advanced"), "pane_log", (!a.pane_log).to_string()));
        }
        sep(ui, l);
        if row(ui, l, "Settings folder", &seen.settings_path, |ui| button(ui, l, "Open")) {
            out.push(Change::OpenFolder);
        }
        sep(ui, l);
        row(ui, l, "Saved tabs", &seen.state_path, |_| ());
        sep(ui, l);
        if row(ui, l, "Export or import settings", "One file with the settings, themes and profiles, into Downloads", |ui| button(ui, l, "Export…")) {
            out.push(Change::Export);
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut edit.import).hint_text("The export to read back, a full path").desired_width(360.0).font(FontId::monospace(12.5)));
            if ui.add_enabled(!edit.import.trim().is_empty(), egui::Button::new("Import")).on_hover_text("The files there now are kept beside them as .bak").clicked() {
                out.push(Change::Import(edit.import.trim().to_owned()));
                edit.import.clear();
            }
        });
        ui.add_space(8.0);
        sep(ui, l);
        row(ui, l, "Usage data", "tsumugi sends nothing anywhere", |ui| status(ui, "None", c.fg));
    });
}

fn theme(ui: &mut egui::Ui, l: Look, seen: &Seen, out: &mut Vec<Change>) {
    let (pal, c) = (l.pal, &l.c);
    let s = seen.settings;
    let mode = match s.theme.as_str() {
        "system" => "system",
        "light" => "light",
        "dark" => "dark",
        name => {
            if seen.themes.iter().any(|t| t.name.eq_ignore_ascii_case(name) && t.colors.light) {
                "light"
            } else {
                "dark"
            }
        }
    };
    // The mode as one segmented control on the right (the design's Themes).
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        egui::Frame::NONE.stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(8.0).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (k, label) in [("dark", "Dark"), ("light", "Light"), ("system", "Follow OS")] {
                    let on = mode == k;
                    let b = egui::Button::new(RichText::new(label).size(12.5).color(if on { c.strong() } else { c.dim }))
                        .fill(if on { c.chosen() } else { Color32::TRANSPARENT })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(6.0)
                        .min_size(egui::vec2(84.0, 26.0));
                    let r = ui.add(b);
                    focus_ring(ui, &r);
                    if r.clicked() && !on {
                        // A mode keeps the theme picked for it.
                        let value = match k {
                            "system" => "system".to_owned(),
                            "light" => s.light_theme.clone(),
                            _ => s.dark_theme.clone(),
                        };
                        out.push(Change::Set(None, "theme", tsumugi_mux::settings::quote(&value)));
                    }
                }
            });
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Mode").size(12.0).color(c.dim));
    });
    ui.add_space(10.0);
    let picked = |t: &Theme| {
        let name = &t.name;
        match s.theme.as_str() {
            "system" => name == if t.colors.light { &s.light_theme } else { &s.dark_theme },
            "dark" => name == "tsumugi Dark",
            "light" => name == "tsumugi Light",
            other => name.eq_ignore_ascii_case(other),
        }
    };
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            let list: Vec<&Theme> = seen.themes.iter().filter(|t| mode == "system" || t.colors.light == (mode == "light")).collect();
            ui.label(RichText::new(format!("THEME · {}", list.len())).size(11.0).strong().color(c.dim));
            egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border)).corner_radius(10.0).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                for t in list {
                    let on = picked(t);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
                    let p = ui.painter();
                    if on {
                        p.rect_filled(r, 7.0, c.chosen());
                        p.rect_stroke(r, 7.0, egui::Stroke::new(1.0, c.run), egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        p.rect_filled(r, 7.0, c.hover());
                    }
                    // Its background and three of its states, side by side.
                    // A small chip of four: its ground and three of its states.
                    let chip = egui::Rect::from_min_size(egui::pos2(r.left() + 10.0, r.center().y - 9.0), egui::vec2(40.0, 18.0));
                    for (k, sw) in [t.colors.bg, t.colors.wait, t.colors.run, t.colors.err].iter().enumerate() {
                        let x = chip.left() + k as f32 * 10.0;
                        let corner = match k {
                            0 => egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 },
                            3 => egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 },
                            _ => egui::CornerRadius::ZERO,
                        };
                        p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, chip.top()), egui::vec2(10.0, 18.0)), corner, *sw);
                    }
                    p.rect_stroke(chip, 4.0, egui::Stroke::new(1.0, c.border_strong()), egui::StrokeKind::Outside);
                    p.text(egui::pos2(r.left() + 62.0, r.center().y), egui::Align2::LEFT_CENTER, &t.name, FontId::proportional(13.0), if on { c.strong() } else { c.fg });
                    let kind = if t.colors.light { "light" } else { "dark" };
                    p.text(egui::pos2(r.right() - 10.0, r.center().y), egui::Align2::RIGHT_CENTER, kind, FontId::proportional(11.0), c.faint());
                    if resp.clicked() {
                        // In Follow OS, a theme becomes the one for its kind.
                        let change = match mode {
                            "system" => Change::Set(None, if t.colors.light { "light_theme" } else { "dark_theme" }, tsumugi_mux::settings::quote(&t.name)),
                            _ => Change::Set(None, "theme", tsumugi_mux::settings::quote(&t.name)),
                        };
                        out.push(change);
                    }
                }
            });
            ui.label(RichText::new("Themes in themes/ beside the settings show here too.").size(12.0).color(c.dim));
        });
        ui.add_space(20.0);
        ui.vertical(|ui| {
            let name = seen.themes.iter().find(|t| t.colors == seen.current).map_or("", |t| t.name.as_str());
            ui.label(RichText::new(format!("PREVIEW · {name}")).size(11.0).strong().color(c.dim));
            preview(ui, pal);
            // What else the window looks like, a click away on its page.
            let f = &s.font;
            let family = if f.family.is_empty() { "Automatic" } else { f.family.as_str() };
            let material = match s.window.material.as_str() {
                "none" => "None",
                "mica" => "Mica",
                "acrylic" => "Acrylic",
                other => other,
            };
            let motion = if s.appearance.animations { "On" } else { "Off" };
            ui.add_space(6.0);
            let line = format!("Font {family} {} · Window {material} · Motion {motion}", f.size);
            if ui.add(egui::Label::new(RichText::new(line).size(12.0).color(c.dim)).sense(egui::Sense::click())).on_hover_text("On the Appearance page").clicked() {
                out.push(Change::GoTo(Page::Appearance));
            }
        });
    });
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
    card(12.0, c.wait, c.wait_bg(), "Approve the edit", "~/dev/filer", "Waiting for you · 2m", c.wait);
    let running = card(72.0, c.run.gamma_multiply(0.6), c.panel, "Split the pane", "~/dev/tsumugi", "Running · 4m", c.dim);
    p.line_segment([running.left_top() + egui::vec2(30.0, 1.0), running.left_top() + egui::vec2(80.0, 1.0)], egui::Stroke::new(2.0, c.run));
    card(132.0, c.err.gamma_multiply(0.7), c.panel, "Windows CI test", "~/dev/filer", "Error · Exited 101", c.err);
    card(192.0, c.done.gamma_multiply(0.5), c.panel, "Write the scope", "~/notes", "Done · 20m", c.dim);
    let pane = egui::Rect::from_min_max(egui::pos2(side.right() + 10.0, rect.top() + 10.0), rect.max - egui::vec2(10.0, 10.0));
    p.rect_filled(pane, 8.0, pal.bg);
    p.rect_stroke(pane, 8.0, egui::Stroke::new(1.0, c.wait), egui::StrokeKind::Inside);
    let mono = FontId::monospace(11.5);
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
    }

    impl Run {
        fn new() -> Self {
            let facts = Facts { shells: vec![("bash".into(), "bash".into())], hooks: false, shell: "bash".into(), shell_hook: Some(false), autostart: false, conpty: true };
            Self { ctx: egui::Context::default(), settings: Settings::default(), facts, themes: crate::theme::builtin() }
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
                state_path: "/home/u/.local/state/tsumugi/state.toml".into(),
                facts: Some(&self.facts),
            };
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(WIDE, 2600.0))), events, ..Default::default() };
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
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
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

    fn at_page(page: Page) -> Screen {
        Screen { page, ..Screen::default() }
    }

    /// Where a text was drawn: the first exact match, else the first that
    /// holds it.
    fn find<'a>(texts: &'a [(String, egui::Rect)], words: &str) -> Option<&'a egui::Rect> {
        texts.iter().find(|(t, _)| t == words).or_else(|| texts.iter().find(|(t, _)| t.contains(words))).map(|(_, r)| r)
    }

    #[test]
    fn every_page_draws_the_rows_its_search_finds() {
        let run = Run::new();
        for page in Page::ALL {
            let mut screen = at_page(page);
            run.frame(&mut screen, Vec::new());
            let (_, texts) = run.frame(&mut screen, Vec::new());
            assert!(find(&texts, page.title()).is_some(), "{page:?}: its title");
            for (_, words) in INDEX.iter().filter(|(p, _)| *p == page) {
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

        screen.query = "scrollback".into();
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
        assert_eq!(changes, vec![Change::Set(Some("sessions"), "claude", "\"claude-x\"".into())]);
        // Shown as typed until the file says it.
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "claude-x").is_some());
    }

    #[test]
    fn a_key_is_taken_unless_it_clashes() {
        let run = Run::new();
        let ctrl_shift = egui::Modifiers { ctrl: true, shift: true, ..Default::default() };
        let mut screen = at_page(Page::Keys);
        run.frame(&mut screen, Vec::new());
        screen.edit.capturing = Some("new_tab");
        let changes = run.key(&mut screen, egui::Key::K, ctrl_shift);
        assert_eq!(changes, vec![Change::Set(Some("keys"), "new_tab", "\"Ctrl+Shift+K\"".into())]);
        assert_eq!(screen.edit.capturing, None);

        // Another of the window's keys: named, and nothing written.
        screen.edit.capturing = Some("new_tab");
        let changes = run.key(&mut screen, egui::Key::W, ctrl_shift);
        assert!(changes.is_empty(), "{changes:?}");
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Ctrl+Shift+W is Close the session already").is_some());
        // One of Claude Code's.
        let changes = run.key(&mut screen, egui::Key::C, egui::Modifiers { ctrl: true, ..Default::default() });
        assert!(changes.is_empty());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Ctrl+C is Claude Code's interrupt").is_some());

        // Esc while a key is being changed leaves the key, not the screen.
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(changes.is_empty());
        assert_eq!(screen.edit.capturing, None);
        let changes = run.key(&mut screen, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(changes, vec![Change::Close]);
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
        run.settings.tags.rule = vec![TagRule { folder: "~/dev/*".into(), tag: "{name}".into(), ..TagRule::default() }, TagRule { branch: "claude/*".into(), tag: "claude".into(), ..TagRule::default() }];
        run.settings.menu.session = vec![MenuItem { name: "lazygit".into(), command: "lazygit -p {folder}".into() }];
        let mut screen = at_page(Page::Tags);
        run.frame(&mut screen, Vec::new());
        let (_, texts) = run.frame(&mut screen, Vec::new());
        assert!(find(&texts, "Folder ~/dev/*").is_some() && find(&texts, "Branch claude/*").is_some());
        let first = *find(&texts, "Folder ~/dev/*").unwrap();
        let remove = texts.iter().filter(|(t, _)| t == "Remove").map(|(_, r)| *r).find(|r| (r.center().y - first.top() - 16.0).abs() < 14.0).expect("its Remove");
        let changes = run.click(&mut screen, remove.center());
        assert_eq!(changes, vec![Change::Rules(vec![run.settings.tags.rule[1].clone()])]);

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
        assert_eq!(screen.page, Page::Advanced, "round from the first");
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
        assert_eq!(changes, vec![Change::Set(Some("notify"), "system", "[\"error\", \"done\"]".into())], "the first switch: system, waiting");
    }
}
