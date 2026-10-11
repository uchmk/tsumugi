//! The new-session dialog (the design's 1g, `Ctrl+Shift+T`): the folder,
//! what to start in it, its tags, and where to put it. The folder of the
//! pane with the keys is filled in and Claude Code is the default, so the
//! commonest case -- one more Claude Code here -- is two keys: the shortcut
//! and `Enter`.

use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::settings::{Profile, TagRule};
use ito_pane::Palette;

use crate::chrome;
use crate::i18n::tr;
use crate::remote::Where;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Claude,
    /// The last conversation in the folder (`claude --continue`).
    Resume,
    Shell,
}

impl Start {
    const ALL: [Start; 3] = [Start::Claude, Start::Resume, Start::Shell];

    fn label(self) -> &'static str {
        match self {
            Start::Claude => tr("newsession.claude"),
            Start::Resume => tr("newsession.resume"),
            Start::Shell => tr("newsession.shell"),
        }
    }

    /// As a profile keeps it.
    pub fn word(self) -> &'static str {
        match self {
            Start::Claude => "claude",
            Start::Resume => "resume",
            Start::Shell => "shell",
        }
    }

    pub fn from_word(w: &str) -> Option<Self> {
        Start::ALL.into_iter().find(|s| s.word() == w)
    }

    /// The line typed into the new shell, if any: `claude` is Claude
    /// Code's command (`[sessions] claude`).
    pub fn typed(self, claude: &str) -> Option<String> {
        let claude = if claude.trim().is_empty() { "claude" } else { claude.trim() };
        match self {
            Start::Claude => Some(claude.to_owned()),
            Start::Resume => Some(format!("{claude} --continue")),
            Start::Shell => None,
        }
    }
}

/// A folder the list offers, with what to say beside it.
pub struct Recent {
    pub folder: PathBuf,
    pub note: String,
}

/// The dialog while it is open.
pub struct Dialog {
    folder: String,
    start: Start,
    /// Tags put on by hand.
    tags: Vec<String>,
    /// Tags the folder's rules give, taken off by hand.
    dropped: Vec<String>,
    tag_input: String,
    save: bool,
    name: String,
    /// In a git worktree of its own, on this branch (empty: one named for
    /// the time).
    worktree: bool,
    branch: String,
    /// More panes in the same tab, beside the first (a profile's layout).
    more: Vec<Start>,
    /// The line of the folder list the arrows are on.
    selected: Option<usize>,
    opening: bool,
    /// The start button the arrows moved to, to give the keys next frame.
    start_focus: Option<usize>,
    /// The ways-to-start buttons as last drawn, for Enter on one of them;
    /// Profile… after them; the first thing in the Beside it row.
    start_ids: Vec<egui::Id>,
    profile_id: Option<egui::Id>,
    beside_id: Option<egui::Id>,
    /// Where it runs, and the places found to offer (none: the row is not
    /// shown).
    pub on: Where,
    pub places: Vec<Where>,
}

/// What the dialog was asked to do.
pub enum Answer {
    Create(Create),
    Cancel,
}

pub struct Create {
    pub folder: PathBuf,
    pub start: Start,
    /// Tags to put on, the rules' among them.
    pub tags: Vec<String>,
    /// The rules' tags taken off.
    pub dropped: Vec<String>,
    /// Beside the pane with the keys (`Alt+Enter`), not in a tab of its own.
    pub split: bool,
    /// Keep these choices as a profile of this name.
    pub save_as: Option<String>,
    /// Start it in a new git worktree on this branch.
    pub worktree: Option<String>,
    /// More panes beside it, in the same folder.
    pub more: Vec<Start>,
    /// This machine, a WSL distribution or an SSH host.
    pub on: Where,
}

/// Where the `k`th more pane goes (0 the first of them), given the first
/// pane and those already made: right of the first, below that, below the
/// first -- two, three or four panes in a square.
pub fn more_place(k: usize, first: u64, made: &[u64]) -> (u64, ito_layout::Dir) {
    use ito_layout::Dir;
    match k {
        0 => (first, Dir::Right),
        1 => (made.first().copied().unwrap_or(first), Dir::Down),
        _ => (first, Dir::Down),
    }
}

/// How many more panes a tab can start with.
pub const MORE_MOST: usize = 3;

impl Dialog {
    #[cfg(test)]
    pub fn new(folder: &std::path::Path) -> Self {
        Self::starting(folder, Start::Claude)
    }

    /// The dialog with `start` chosen first (`[sessions] start`).
    pub fn starting(folder: &std::path::Path, start: Start) -> Self {
        Self {
            folder: crate::home_short(folder),
            start,
            tags: Vec::new(),
            dropped: Vec::new(),
            tag_input: String::new(),
            save: false,
            name: String::new(),
            worktree: false,
            branch: String::new(),
            more: Vec::new(),
            selected: None,
            opening: true,
            start_focus: None,
            start_ids: Vec::new(),
            profile_id: None,
            beside_id: None,
            on: Where::Here,
            places: Vec::new(),
        }
    }

    fn folder_path(&self) -> PathBuf {
        expand(&self.folder)
    }

    /// The tags the folder's rules give, less those taken off.
    fn rule_tags(&self, rules: &[TagRule]) -> Vec<String> {
        let home = tsumugi_mux::settings::home();
        let path = self.folder_path();
        let mut out: Vec<String> = Vec::new();
        for t in rules.iter().filter_map(|r| r.tag_for(&path, home.as_deref())) {
            if !out.contains(&t) && !self.dropped.contains(&t) {
                out.push(t);
            }
        }
        out
    }

    fn apply(&mut self, p: &Profile) {
        self.folder.clone_from(&p.folder);
        self.start = Start::from_word(&p.start).unwrap_or(Start::Claude);
        self.tags.clone_from(&p.tags);
        self.more = p.panes.iter().filter_map(|w| Start::from_word(w)).take(MORE_MOST).collect();
        self.on = Where::from_word(&p.place);
        self.dropped.clear();
        self.selected = None;
    }

    /// A session started straight away, as the first-run screen does: in
    /// `folder`, started as `start`, nothing else chosen.
    pub fn quick(folder: &std::path::Path, start: Start, rules: &[TagRule]) -> Create {
        match Self::starting(folder, start).create(rules, false) {
            Answer::Create(c) => c,
            Answer::Cancel => unreachable!("create creates"),
        }
    }

    fn create(&self, rules: &[TagRule], split: bool) -> Answer {
        let mut tags = self.rule_tags(rules);
        for t in &self.tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        let save_as = (self.save && !self.name.trim().is_empty()).then(|| self.name.trim().to_owned());
        let worktree = self.worktree.then(|| {
            let b = self.branch.trim();
            if b.is_empty() { crate::worktree::default_branch(chrono::Local::now()) } else { b.to_owned() }
        });
        Answer::Create(Create { folder: self.folder_path(), start: self.start, tags, dropped: self.dropped.clone(), split, save_as, worktree, more: self.more.clone(), on: self.on.clone() })
    }
}

/// `~` said as the home folder.
pub fn expand(text: &str) -> PathBuf {
    let text = text.trim();
    match text.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => match tsumugi_mux::settings::home() {
            Some(home) => home.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(text),
        },
        _ => PathBuf::from(text),
    }
}

/// The dialog over the window, everything behind it dimmed.
pub fn show(ctx: &egui::Context, pal: &Palette, d: &mut Dialog, recents: &[Recent], profiles: &[Profile], rules: &[TagRule]) -> Option<Answer> {
    // The folders that match what is typed; all of them before any typing.
    let typed = d.folder.trim().to_lowercase();
    let exact = recents.iter().any(|r| crate::home_short(&r.folder).to_lowercase() == typed);
    let mut shown: Vec<&Recent> = recents.iter().filter(|r| crate::palette::score(&typed, &crate::home_short(&r.folder).to_lowercase()).is_some()).collect();
    if exact || typed.is_empty() {
        shown = recents.iter().collect();
    }
    shown.truncate(5);

    // The keys. Tab and Shift+Tab walk the fields, as in any dialog: from
    // the folder, Tab first completes it when the list has a folder that
    // differs from what is typed (the line the arrows are on, else the best
    // match), and moves on once it is complete. The arrows walk the list
    // only from the folder. Enter creates from a field or from nowhere; on a
    // button it presses the button, as Space does.
    let focused = ctx.memory(|m| m.focused());
    let in_field = |name: &str| focused == Some(egui::Id::new(name));
    let in_folder = in_field("ns-folder");
    let typing_tag = in_field("ns-tag") && !d.tag_input.trim().is_empty();
    let mut completion = match d.selected {
        Some(k) => shown.get(k).map(|r| crate::home_short(&r.folder)),
        None if exact => None,
        None => shown.first().map(|r| crate::home_short(&r.folder)),
    }
    .filter(|f| *f != d.folder.trim());
    // Enter on a way to start (Claude Code, Resume last, Shell) picks it and
    // creates, as the dialog's foot says; on another button it presses it.
    let on_start = Start::ALL.into_iter().zip(&d.start_ids).find(|(_, id)| focused == Some(**id)).map(|(s, _)| s);
    // The START row is one stop for Tab, as a row of radio buttons is: Tab
    // comes in on the way picked and leaves for Beside it; Left and Right
    // walk the row, Profile… included.
    let on_profile = focused.is_some() && focused == d.profile_id;
    let on_row = on_start.is_some() || on_profile;
    let at = |s: Start| Start::ALL.iter().position(|x| *x == s).unwrap_or(0);
    let picked_id = d.start_ids.get(at(d.start)).copied();
    // Shift+Tab before Tab: egui's plain Tab also matches it.
    let (row_back, row_tab, row_left, row_right, beside_back) = ctx.input_mut(|i| {
        (
            on_row && i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
            on_row && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
            on_row && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
            on_row && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
            focused.is_some() && focused == d.beside_id && i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
        )
    });
    // egui has already read Tab and the arrows as moves of its own, from
    // the raw keys; those are called off where the row moves instead.
    let focus = |id: Option<egui::Id>| {
        if let Some(id) = id {
            ctx.memory_mut(|m| {
                m.move_focus(egui::FocusDirection::None);
                m.request_focus(id);
            });
        }
    };
    if row_tab {
        focus(d.beside_id);
    }
    if row_back {
        focus(Some(egui::Id::new("ns-folder")));
    }
    if beside_back {
        focus(picked_id);
    }
    if row_left || row_right {
        let n = Start::ALL.len() + 1;
        let here = if on_profile { Start::ALL.len() } else { on_start.map_or(0, at) };
        let next = if row_right { (here + 1) % n } else { (here + n - 1) % n };
        match Start::ALL.get(next) {
            Some(s) => {
                d.start = *s;
                focus(d.start_ids.get(next).copied());
            }
            None => focus(d.profile_id),
        }
    }
    let enter_creates = focused.is_none() || on_start.is_some() || ["ns-folder", "ns-branch", "ns-name", "ns-tag"].iter().any(|n| in_field(n));
    // Alt+Enter before Enter: egui's plain Enter also matches it.
    let (esc, alt_enter, enter, up, down, back, tab) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            i.consume_key(egui::Modifiers::ALT, egui::Key::Enter),
            enter_creates && !typing_tag && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            in_folder && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            in_folder && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            in_folder && completion.is_some() && i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
            in_folder && !i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. } if modifiers.shift)) && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
        )
    });
    // The folder holds on to Tab while it has something to complete (its
    // event filter, below), so Shift+Tab is moved on by hand then.
    if back {
        ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::Previous));
    }
    if esc {
        return Some(Answer::Cancel);
    }
    if down && !shown.is_empty() {
        d.selected = Some(d.selected.map_or(0, |s| (s + 1) % shown.len()));
    }
    if up && !shown.is_empty() {
        d.selected = Some(d.selected.map_or(shown.len() - 1, |s| (s + shown.len() - 1) % shown.len()));
    }
    // Tab from a folder that is complete goes into the START row, on the
    // way picked.
    if tab && completion.is_none() {
        focus(picked_id);
    } else if tab {
        if let Some(f) = completion.take() {
            d.folder = f;
            d.selected = None;
        }
        // The cursor to the end of what was put in, for more typing.
        let id = egui::Id::new("ns-folder");
        if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
            let end = egui::text::CCursor::new(d.folder.chars().count());
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ctx, id);
        }
    }
    if (enter || alt_enter) && !d.opening {
        if let Some(s) = on_start {
            d.start = s;
        }
        if let Some(r) = d.selected.and_then(|s| shown.get(s)) {
            d.folder = crate::home_short(&r.folder);
        }
        return Some(d.create(rules, alt_enter));
    }

    let mut answer = None;
    // Behind: dimmed, and a click there cancels.
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("new-session-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        // Mouse only: Tab never stops on the backdrop.
        let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::CLICK);
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(140));
        if resp.clicked() && !d.opening {
            answer = Some(Answer::Cancel);
        }
    });
    let label = |t: &str| RichText::new(t).size(11.0).strong().color(crate::theme::colors().dim);
    egui::Area::new(egui::Id::new("new-session")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(crate::theme::colors().panel)
            .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.set_width(560.0);
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("newsession.title")).size(16.0).strong().color(crate::theme::colors().strong()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let key = crate::keys::label(crate::keys::Action::NewTab);
                        ui.label(RichText::new(key).font(FontId::monospace(11.5)).color(chrome::grey()));
                    });
                });
                ui.add_space(8.0);

                ui.label(label(tr("newsession.folder")));
                // Tab stays in the field while there is a folder to complete
                // it to (egui moves the keys on Tab before the dialog reads
                // it otherwise); the arrows always stay, for the list.
                let filter = egui::EventFilter { tab: completion.is_some(), horizontal_arrows: true, vertical_arrows: true, escape: false };
                let field = egui::TextEdit::singleline(&mut d.folder).id(egui::Id::new("ns-folder")).font(FontId::monospace(13.0)).desired_width(f32::INFINITY).event_filter(filter);
                let f = ui.add(field);
                if d.opening {
                    f.request_focus();
                }
                if f.changed() {
                    d.selected = None;
                    d.dropped.clear();
                }
                for (k, r) in shown.iter().enumerate() {
                    // The arrows walk these from the folder; Tab passes them by.
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::CLICK);
                    let p = ui.painter();
                    if d.selected == Some(k) || resp.hovered() {
                        p.rect_filled(rect, 6.0, crate::theme::colors().chosen());
                    }
                    let name = p.layout_no_wrap(crate::home_short(&r.folder), FontId::monospace(12.5), crate::theme::colors().strong());
                    let w = name.size().x;
                    p.galley(egui::pos2(rect.left() + 10.0, rect.center().y - name.size().y / 2.0), name, Color32::WHITE);
                    let note = p.layout_no_wrap(r.note.clone(), FontId::proportional(12.0), pal.fg_dim);
                    p.galley(egui::pos2(rect.left() + 20.0 + w, rect.center().y - note.size().y / 2.0), note, pal.fg_dim);
                    if resp.clicked() {
                        d.folder = crate::home_short(&r.folder);
                        d.selected = None;
                        d.dropped.clear();
                    }
                }
                ui.add_space(10.0);

                ui.label(label(tr("newsession.start")));
                ui.horizontal(|ui| {
                    d.start_ids.clear();
                    for (k, s) in Start::ALL.into_iter().enumerate() {
                        // While Profile… has the keys, nothing else in the
                        // row is lit: one place lit in the row at a time
                        // (seen on Windows, 2026-10-07). The way picked
                        // comes back lit when the keys leave Profile….
                        let profile_has_keys = d.profile_id.is_some_and(|id| ui.memory(|m| m.has_focus(id)));
                        let b = start_button(ui, s.label(), d.start == s && !profile_has_keys);
                        d.start_ids.push(b.id);
                        if d.start_focus == Some(k) {
                            b.request_focus();
                            d.start_focus = None;
                        }
                        if b.clicked() {
                            d.start = s;
                        }
                    }
                    // Lit with the ring while it has the keys, as the others
                    // are when picked.
                    let profile_has_keys = d.profile_id.is_some_and(|id| ui.memory(|m| m.has_focus(id)));
                    let profile = start_button(ui, tr("newsession.profile"), profile_has_keys);
                    d.profile_id = Some(profile.id);
                    egui::Popup::menu(&profile).show(|ui| {
                        if profiles.is_empty() {
                            ui.label(RichText::new(tr("newsession.no_profiles")).color(pal.fg_dim));
                        }
                        for p in profiles {
                            if ui.button(format!("{}   {}", p.name, p.folder)).clicked() {
                                d.apply(p);
                                ui.close();
                            }
                        }
                    });
                });
                // More panes in the tab, beside the first (a profile keeps them).
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("newsession.beside")).size(12.0).color(pal.fg_dim));
                    let mut gone = None;
                    d.beside_id = None;
                    for (k, s) in d.more.iter().enumerate() {
                        let chip = ui.add(egui::Button::new(RichText::new(format!("{}  ×", s.label())).size(12.0)));
                        d.beside_id = d.beside_id.or(Some(chip.id));
                        if ring(ui, chip).on_hover_text(tr("newsession.take_off")).clicked() {
                            gone = Some(k);
                        }
                    }
                    if let Some(k) = gone {
                        d.more.remove(k);
                    }
                    if d.more.len() < MORE_MOST {
                        let add = ui.add(egui::Button::new(RichText::new(tr("newsession.add_pane")).size(12.0)));
                        d.beside_id = d.beside_id.or(Some(add.id));
                        let add = ring(ui, add);
                        egui::Popup::menu(&add).show(|ui| {
                            for s in Start::ALL {
                                if ui.button(s.label()).clicked() {
                                    d.more.push(s);
                                    ui.close();
                                }
                            }
                        });
                    }
                    if d.more.is_empty() {
                        ui.label(RichText::new(tr("newsession.one_pane")).size(12.0).color(chrome::grey()));
                    }
                });
                // Where it runs: shown once WSL or SSH has somewhere to offer
                // (or a profile asked for one).
                if !d.places.is_empty() || d.on != Where::Here {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("newsession.runs_on")).size(12.0).color(pal.fg_dim));
                        let mut places = Where::with_kept(&d.places);
                        if !places.contains(&d.on) {
                            places.push(d.on.clone());
                        }
                        let combo = egui::ComboBox::from_id_salt("ns-on").selected_text(d.on.label()).show_ui(ui, |ui| {
                            for p in &places {
                                ui.selectable_value(&mut d.on, p.clone(), p.label());
                            }
                        });
                        ring(ui, combo.response);
                        match d.on {
                            Where::Ssh(_) => {
                                ui.label(RichText::new(tr("newsession.ssh_note")).size(12.0).color(chrome::grey()));
                            }
                            Where::Mux(_) => {
                                ui.label(RichText::new(tr("newsession.mux_note")).size(12.0).color(chrome::grey()));
                            }
                            _ => {}
                        }
                    });
                }
                ui.add_space(10.0);

                ui.label(label(tr("newsession.tags")));
                let rule_tags = d.rule_tags(rules);
                ui.horizontal_wrapped(|ui| {
                    // The rules' tags dashed, as the design draws them; a
                    // click takes either kind off.
                    for (t, auto) in rule_tags.iter().map(|t| (t.clone(), true)).chain(d.tags.clone().into_iter().map(|t| (t, false))) {
                        let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(11.0), pal.fg).size().x) + 12.0;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::CLICK);
                        let p = ui.painter();
                        chrome::tag_chip(p, rect.min, &t, false);
                        if auto {
                            chrome::dashed_outline(p, rect.expand(2.0), crate::theme::colors().run);
                        }
                        let hint = if auto { tr("newsession.rule_tag") } else { tr("newsession.take_off") };
                        if resp.on_hover_text(hint).clicked() {
                            if auto {
                                d.dropped.push(t.clone());
                            } else {
                                d.tags.retain(|x| *x != t);
                            }
                        }
                    }
                    let edit = ui.add(egui::TextEdit::singleline(&mut d.tag_input).id(egui::Id::new("ns-tag")).hint_text(tr("newsession.add_tag")).desired_width(140.0));
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(t) = tsumugi_mux::proto::tag_name(&d.tag_input) {
                            if !d.tags.contains(&t) && d.tags.len() + rule_tags.len() < tsumugi_mux::proto::MAX_TAGS {
                                d.tags.push(t);
                            }
                        }
                        d.tag_input.clear();
                        edit.request_focus();
                    }
                });
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    let c = ui.checkbox(&mut d.worktree, RichText::new(tr("newsession.worktree")).color(crate::theme::colors().dim));
                    ring(ui, c).on_hover_text(tr("newsession.worktree_hover"));
                    if d.worktree {
                        let hint = crate::worktree::default_branch(chrono::Local::now());
                        ui.add(egui::TextEdit::singleline(&mut d.branch).id(egui::Id::new("ns-branch")).hint_text(hint).font(FontId::monospace(12.0)).desired_width(220.0));
                    }
                });
                ui.horizontal(|ui| {
                    let c = ui.checkbox(&mut d.save, RichText::new(tr("newsession.save")).color(crate::theme::colors().dim));
                    ring(ui, c);
                    if d.save {
                        ui.add(egui::TextEdit::singleline(&mut d.name).id(egui::Id::new("ns-name")).hint_text(tr("newsession.name")).desired_width(200.0));
                    }
                });
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    for (key, what) in [("Tab", tr("newsession.next")), ("Enter", tr("newsession.create_key")), ("Alt+Enter", tr("newsession.split"))] {
                        ui.label(RichText::new(key).font(FontId::monospace(12.0)).color(crate::theme::colors().fg));
                        ui.label(RichText::new(what).size(12.0).color(pal.fg_dim));
                        ui.add_space(6.0);
                    }
                    let create = egui::Button::new(RichText::new(tr("newsession.create")).color(crate::theme::colors().on_accent()).strong()).fill(chrome::cyan());
                    let (create, cancel) = chrome::foot(ui, create, true, tr("dialog.cancel"));
                    if ring(ui, cancel).clicked() {
                        answer = Some(Answer::Cancel);
                    }
                    if ring(ui, create).clicked() {
                        answer = Some(d.create(rules, false));
                    }
                });
            });
    });
    d.opening = false;
    answer
}

fn start_button(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    // The way picked is filled, its edge as the others': cyan edges are the
    // ring's alone, so the picked one does not read as having the keys
    // when they have moved on (to Profile…, seen on Windows, 2026-10-07).
    let (fill, stroke, color) = if on {
        (crate::theme::colors().run.gamma_multiply(0.28), crate::theme::colors().border_strong(), crate::theme::colors().strong())
    } else {
        (Color32::TRANSPARENT, crate::theme::colors().border, crate::theme::colors().fg)
    };
    let b = egui::Button::new(RichText::new(text).size(12.5).color(color)).fill(fill).stroke(egui::Stroke::new(1.0, stroke)).min_size(egui::vec2(128.0, 32.0));
    let r = ui.add(b);
    ring(ui, r)
}

/// A ring round the control that has the keys, so Tab shows where it went.
fn ring(ui: &egui::Ui, r: egui::Response) -> egui::Response {
    if r.has_focus() {
        ui.painter().rect_stroke(r.rect.expand(3.0), 8.0, egui::Stroke::new(1.5, chrome::cyan()), egui::StrokeKind::Outside);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_kept_by_its_word() {
        for s in Start::ALL {
            assert_eq!(Start::from_word(s.word()), Some(s));
        }
        assert_eq!(Start::Claude.typed("").as_deref(), Some("claude"));
        assert_eq!(Start::Resume.typed("C:\\tools\\claude.exe").as_deref(), Some("C:\\tools\\claude.exe --continue"));
        assert_eq!(Start::Shell.typed("claude"), None);
    }

    #[test]
    fn more_panes_make_a_square() {
        use ito_layout::Node;
        let mut layout = Node::Leaf(1u64);
        let mut made = Vec::new();
        for k in 0..3 {
            let (beside, dir) = more_place(k, 1, &made);
            let id = 2 + k as u64;
            assert!(layout.split(&beside, dir, id));
            made.push(id);
        }
        // 1 | 2 over 3, with 4 below 1: two by two.
        let r = layout.layout(ito_layout::Rect::new(0.0, 0.0, 100.0, 100.0), 0.0);
        let at = |id: u64| r.iter().find(|(x, _)| *x == id).unwrap().1;
        assert_eq!((at(1).x, at(1).y, at(4).x, at(4).y), (0.0, 0.0, 0.0, 50.0));
        assert_eq!((at(2).x, at(2).y, at(3).x, at(3).y), (50.0, 0.0, 50.0, 50.0));
    }

    #[test]
    fn rule_tags_follow_the_folder_less_those_taken_off() {
        let rules = vec![TagRule { folder: "/srv/*".into(), tag: "{name}".into(), ..TagRule::default() }, TagRule { folder: "/srv/ci".into(), tag: "ci".into(), ..TagRule::default() }];
        let mut d = Dialog::new(std::path::Path::new("/srv/ci"));
        assert_eq!(d.rule_tags(&rules), ["ci"], "the same tag from two rules, once");
        d.dropped.push("ci".into());
        assert!(d.rule_tags(&rules).is_empty());
        d.tags.push("mine".into());
        let Answer::Create(c) = d.create(&rules, true) else { panic!("created") };
        assert_eq!((c.tags, c.dropped, c.split), (vec!["mine".to_owned()], vec!["ci".to_owned()], true));
    }

    /// The dialog drawn without a window, keys pressed into it.
    struct Run {
        ctx: egui::Context,
        recents: Vec<Recent>,
    }

    impl Run {
        fn new() -> Self {
            let recents = ["/srv/app", "/srv/api"].iter().map(|f| Recent { folder: PathBuf::from(f), note: String::new() }).collect();
            Self { ctx: egui::Context::default(), recents }
        }

        fn frame(&self, d: &mut Dialog, events: Vec<egui::Event>) -> Option<Answer> {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 800.0))), events, ..Default::default() };
            let mut answer = None;
            let mut out = self.ctx.run_ui(input, |ui| answer = show(ui.ctx(), &Palette::default(), d, &self.recents, &[], &[]));
            out.textures_delta.clear();
            answer
        }

        /// A key pressed and let go, then a frame for the focus to settle.
        fn key(&self, d: &mut Dialog, key: egui::Key, modifiers: egui::Modifiers) -> Option<Answer> {
            let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
            let answer = self.frame(d, vec![ev(true), ev(false)]);
            answer.or_else(|| self.frame(d, Vec::new()))
        }

        #[allow(dead_code)]
        fn focused(&self) -> String {
            let id = self.ctx.memory(|m| m.focused());
            let rect = id.and_then(|id| self.ctx.read_response(id)).map(|r| r.rect);
            format!("{id:?} {rect:?}")
        }

        /// The texts drawn in a frame, with where.
        fn texts(&self, d: &mut Dialog) -> Vec<(String, egui::Rect)> {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 800.0))), ..Default::default() };
            let mut out = self.ctx.run_ui(input, |ui| {
                show(ui.ctx(), &Palette::default(), d, &self.recents, &[], &[]);
            });
            out.textures_delta.clear();
            fn walk(s: &egui::Shape, v: &mut Vec<(String, egui::Rect)>) {
                match s {
                    egui::Shape::Text(t) => v.push((t.galley.text().to_owned(), t.galley.rect.translate(t.pos.to_vec2()))),
                    egui::Shape::Vec(list) => list.iter().for_each(|s| walk(s, v)),
                    _ => {}
                }
            }
            let mut v = Vec::new();
            for c in &out.shapes {
                walk(&c.shape, &mut v);
            }
            v
        }

        fn in_folder(&self) -> bool {
            self.ctx.memory(|m| m.has_focus(egui::Id::new("ns-folder")))
        }
    }

    #[test]
    fn tab_completes_the_folder_then_walks_the_fields() {
        let run = Run::new();
        let mut d = Dialog::new(std::path::Path::new("/srv/ap"));
        run.frame(&mut d, Vec::new());
        run.frame(&mut d, Vec::new());
        assert!(run.in_folder(), "the folder has the keys when it opens");
        // Not a folder of the list yet: Tab completes it and stays.
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(d.folder, "/srv/app");
        assert!(run.in_folder());
        // Complete: Tab goes on, to the first way to start.
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::NONE);
        assert!(!run.in_folder(), "Tab left the folder");
        // The arrows pick the way to start there.
        run.key(&mut d, egui::Key::ArrowRight, egui::Modifiers::NONE);
        assert_eq!(d.start, Start::ALL[1]);
        run.key(&mut d, egui::Key::ArrowLeft, egui::Modifiers::NONE);
        // Round from the first, past Profile… (the way picked stays).
        run.key(&mut d, egui::Key::ArrowLeft, egui::Modifiers::NONE);
        assert_eq!(d.start, Start::ALL[0], "on Profile…, the way picked is kept: {}", run.focused());
        run.key(&mut d, egui::Key::ArrowLeft, egui::Modifiers::NONE);
        assert_eq!(d.start, Start::ALL[Start::ALL.len() - 1], "round from the first");
        // The row is one stop: Shift+Tab goes back to the folder at once.
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::SHIFT);
        assert!(run.in_folder(), "Shift+Tab back to the folder: {}", run.focused());
        // Enter from the folder creates.
        let answer = run.key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
        let Some(Answer::Create(c)) = answer else { panic!("created") };
        assert_eq!(c.folder, PathBuf::from("/srv/app"));
    }

    /// The dialog's buttons sit as every dialog's do (`chrome::foot`): at
    /// the right, Cancel then Create, on one line.
    #[test]
    fn the_buttons_sit_as_in_every_dialog() {
        let run = Run::new();
        let mut d = Dialog::new(std::path::Path::new("/srv/app"));
        run.frame(&mut d, Vec::new());
        run.frame(&mut d, Vec::new());
        let texts = run.texts(&mut d);
        let at = |w: &str| texts.iter().find(|(t, _)| t == w).map(|(_, r)| *r).unwrap_or_else(|| panic!("{w}"));
        let (cancel, create, folder) = (at("Cancel"), at("Create"), at("FOLDER"));
        assert!(cancel.right() < create.left(), "Cancel {cancel:?} before Create {create:?}");
        assert!((cancel.center().y - create.center().y).abs() < 2.0, "on one line");
        assert!(create.left() > folder.left() + 300.0, "at the right: {create:?}");
    }

    #[test]
    fn enter_on_a_button_presses_it() {
        let run = Run::new();
        let mut d = Dialog::new(std::path::Path::new("/srv/app"));
        run.frame(&mut d, Vec::new());
        run.frame(&mut d, Vec::new());
        // Shift+Tab from the folder wraps round to the last of the dialog,
        // Create (Cancel reads, and is tabbed to, before it); once more,
        // Cancel.
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::SHIFT);
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::SHIFT);
        let answer = run.key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(answer, Some(Answer::Cancel)), "Enter on Cancel cancels: {}", run.focused());
        // Tab leaves the START row for Beside it (2026-10-07, the owner's
        // ask), and Shift+Tab from there comes back on the way picked.
        let mut d = Dialog::new(std::path::Path::new("/srv/app"));
        run.frame(&mut d, Vec::new());
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::NONE);
        run.key(&mut d, egui::Key::ArrowRight, egui::Modifiers::NONE);
        run.key(&mut d, egui::Key::ArrowRight, egui::Modifiers::NONE);
        assert_eq!(d.start, Start::ALL[2]);
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::NONE);
        let focused = run.ctx.memory(|m| m.focused());
        assert!(focused.is_some() && focused == d.beside_id, "Tab went to Beside it: {}", run.focused());
        run.key(&mut d, egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(run.ctx.memory(|m| m.focused()), d.start_ids.get(2).copied(), "back on Shell: {}", run.focused());
        // Enter on a way to start picks it and creates (seen on Windows,
        // 2026-10-07: Enter on Shell did nothing, Alt+Enter worked).
        let answer = run.key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(answer, Some(Answer::Create(_))), "Enter on Shell creates: {}", run.focused());
    }
}
