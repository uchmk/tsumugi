//! The new-session dialog (the design's 1g, `Ctrl+Shift+T`): the folder,
//! what to start in it, its tags, and where to put it. The folder of the
//! pane with the keys is filled in and Claude Code is the default, so the
//! commonest case -- one more Claude Code here -- is two keys: the shortcut
//! and `Enter`.

use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::settings::{Profile, TagRule};
use tsumugi_pane::Palette;

use crate::chrome;

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
            Start::Claude => "Claude Code",
            Start::Resume => "Resume last",
            Start::Shell => "Shell",
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

    /// The line typed into the new shell, if any.
    pub fn typed(self) -> Option<String> {
        match self {
            Start::Claude => Some("claude".into()),
            Start::Resume => Some("claude --continue".into()),
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
    /// The line of the folder list the arrows are on.
    selected: Option<usize>,
    opening: bool,
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
}

impl Dialog {
    pub fn new(folder: &std::path::Path) -> Self {
        Self {
            folder: crate::home_short(folder),
            start: Start::Claude,
            tags: Vec::new(),
            dropped: Vec::new(),
            tag_input: String::new(),
            save: false,
            name: String::new(),
            selected: None,
            opening: true,
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
        self.dropped.clear();
        self.selected = None;
    }

    fn create(&self, rules: &[TagRule], split: bool) -> Answer {
        let mut tags = self.rule_tags(rules);
        for t in &self.tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        let save_as = (self.save && !self.name.trim().is_empty()).then(|| self.name.trim().to_owned());
        Answer::Create(Create { folder: self.folder_path(), start: self.start, tags, dropped: self.dropped.clone(), split, save_as })
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
    let typing_tag = !d.tag_input.trim().is_empty();
    // Alt+Enter before Enter: egui's plain Enter also matches it.
    let (esc, alt_enter, enter, up, down, tab) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            i.consume_key(egui::Modifiers::ALT, egui::Key::Enter),
            !typing_tag && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
        )
    });
    if esc {
        return Some(Answer::Cancel);
    }
    // The folders that match what is typed; all of them before any typing.
    let typed = d.folder.trim().to_lowercase();
    let mut shown: Vec<&Recent> = recents.iter().filter(|r| crate::palette::score(&typed, &crate::home_short(&r.folder).to_lowercase()).is_some()).collect();
    if shown.iter().any(|r| crate::home_short(&r.folder).to_lowercase() == typed) || typed.is_empty() {
        shown = recents.iter().collect();
    }
    shown.truncate(5);
    if down && !shown.is_empty() {
        d.selected = Some(d.selected.map_or(0, |s| (s + 1) % shown.len()));
    }
    if up && !shown.is_empty() {
        d.selected = Some(d.selected.map_or(shown.len() - 1, |s| (s + shown.len() - 1) % shown.len()));
    }
    if tab {
        if let Some(r) = d.selected.and_then(|s| shown.get(s)).or(shown.first()) {
            d.folder = crate::home_short(&r.folder);
            d.selected = None;
        }
    }
    if (enter || alt_enter) && !d.opening {
        if let Some(r) = d.selected.and_then(|s| shown.get(s)) {
            d.folder = crate::home_short(&r.folder);
        }
        return Some(d.create(rules, alt_enter));
    }

    let mut answer = None;
    // Behind: dimmed, and a click there cancels.
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("new-session-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(140));
        if resp.clicked() && !d.opening {
            answer = Some(Answer::Cancel);
        }
    });
    let label = |t: &str| RichText::new(t).size(11.0).strong().color(Color32::from_rgb(0x9a, 0xa3, 0xb5));
    egui::Area::new(egui::Id::new("new-session")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(Color32::from_rgb(0x1b, 0x1e, 0x24))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4b)))
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.set_width(560.0);
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("New session").size(16.0).strong().color(Color32::from_rgb(0xe4, 0xe8, 0xf0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let key = if cfg!(target_os = "macos") { "Cmd+T" } else { "Ctrl+Shift+T" };
                        ui.label(RichText::new(key).font(FontId::monospace(11.5)).color(chrome::GREY));
                    });
                });
                ui.add_space(8.0);

                ui.label(label("FOLDER"));
                let field = egui::TextEdit::singleline(&mut d.folder).id(egui::Id::new("ns-folder")).font(FontId::monospace(13.0)).desired_width(f32::INFINITY);
                let f = ui.add(field);
                if d.opening {
                    f.request_focus();
                }
                if f.changed() {
                    d.selected = None;
                    d.dropped.clear();
                }
                for (k, r) in shown.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                    let p = ui.painter();
                    if d.selected == Some(k) || resp.hovered() {
                        p.rect_filled(rect, 6.0, Color32::from_rgb(0x2a, 0x2e, 0x37));
                    }
                    let name = p.layout_no_wrap(crate::home_short(&r.folder), FontId::monospace(12.5), Color32::from_rgb(0xe4, 0xe8, 0xf0));
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

                ui.label(label("START"));
                ui.horizontal(|ui| {
                    for s in Start::ALL {
                        if start_button(ui, s.label(), d.start == s).clicked() {
                            d.start = s;
                        }
                    }
                    let profile = start_button(ui, "Profile…", false);
                    egui::Popup::menu(&profile).show(|ui| {
                        if profiles.is_empty() {
                            ui.label(RichText::new("No profiles yet: tick \"Save as a profile\"").color(pal.fg_dim));
                        }
                        for p in profiles {
                            if ui.button(format!("{}   {}", p.name, p.folder)).clicked() {
                                d.apply(p);
                                ui.close();
                            }
                        }
                    });
                });
                ui.add_space(10.0);

                ui.label(label("TAGS"));
                let rule_tags = d.rule_tags(rules);
                ui.horizontal_wrapped(|ui| {
                    // The rules' tags dashed, as the design draws them; a
                    // click takes either kind off.
                    for (t, auto) in rule_tags.iter().map(|t| (t.clone(), true)).chain(d.tags.clone().into_iter().map(|t| (t, false))) {
                        let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(11.0), pal.fg).size().x) + 12.0;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::click());
                        let p = ui.painter();
                        chrome::tag_chip(p, rect.min, &t, false);
                        if auto {
                            chrome::dashed_outline(p, rect.expand(2.0), Color32::from_rgb(0x9f, 0xe0, 0xe0));
                        }
                        let hint = if auto { "From the folder's rule; click to leave it off" } else { "Click to take it off" };
                        if resp.on_hover_text(hint).clicked() {
                            if auto {
                                d.dropped.push(t.clone());
                            } else {
                                d.tags.retain(|x| *x != t);
                            }
                        }
                    }
                    let edit = ui.add(egui::TextEdit::singleline(&mut d.tag_input).id(egui::Id::new("ns-tag")).hint_text("Add a tag").desired_width(140.0));
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
                    ui.checkbox(&mut d.save, RichText::new("Save as a profile").color(Color32::from_rgb(0x9a, 0xa3, 0xb5)));
                    if d.save {
                        ui.add(egui::TextEdit::singleline(&mut d.name).id(egui::Id::new("ns-name")).hint_text("Its name").desired_width(200.0));
                    }
                });
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Alt+Enter").font(FontId::monospace(12.0)).color(Color32::from_rgb(0xc8, 0xcd, 0xd8)));
                    ui.label(RichText::new("split right").size(12.0).color(pal.fg_dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let create = egui::Button::new(RichText::new("Create").color(Color32::from_rgb(0x0f, 0x1d, 0x1d)).strong())
                            .fill(chrome::CYAN)
                            .min_size(egui::vec2(90.0, 30.0));
                        if ui.add(create).clicked() {
                            answer = Some(d.create(rules, false));
                        }
                        if ui.add(egui::Button::new("Cancel").min_size(egui::vec2(70.0, 30.0))).clicked() {
                            answer = Some(Answer::Cancel);
                        }
                    });
                });
            });
    });
    d.opening = false;
    answer
}

fn start_button(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let (fill, stroke, color) = if on {
        (Color32::from_rgba_unmultiplied(0x6f, 0xd0, 0xd0, 30), chrome::CYAN, Color32::from_rgb(0xe4, 0xe8, 0xf0))
    } else {
        (Color32::TRANSPARENT, Color32::from_rgb(0x2c, 0x30, 0x39), Color32::from_rgb(0xc8, 0xcd, 0xd8))
    };
    let b = egui::Button::new(RichText::new(text).size(12.5).color(color)).fill(fill).stroke(egui::Stroke::new(1.0, stroke)).min_size(egui::vec2(128.0, 32.0));
    ui.add(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_kept_by_its_word() {
        for s in Start::ALL {
            assert_eq!(Start::from_word(s.word()), Some(s));
        }
        assert_eq!(Start::Claude.typed().as_deref(), Some("claude"));
        assert_eq!(Start::Shell.typed(), None);
    }

    #[test]
    fn rule_tags_follow_the_folder_less_those_taken_off() {
        let rules = vec![TagRule { folder: "/srv/*".into(), tag: "{name}".into() }, TagRule { folder: "/srv/ci".into(), tag: "ci".into() }];
        let mut d = Dialog::new(std::path::Path::new("/srv/ci"));
        assert_eq!(d.rule_tags(&rules), ["ci"], "the same tag from two rules, once");
        d.dropped.push("ci".into());
        assert!(d.rule_tags(&rules).is_empty());
        d.tags.push("mine".into());
        let Answer::Create(c) = d.create(&rules, true) else { panic!("created") };
        assert_eq!((c.tags, c.dropped, c.split), (vec!["mine".to_owned()], vec!["ci".to_owned()], true));
    }
}
