//! The settings screen (the design's 1m, `Ctrl+,`): nine pages down the
//! left, each a few cards of rows. It shows what `settings.toml` says and
//! hands back what to change; the window writes it (one line of the file at
//! a time, so the rest stays as written) and reads it again.
//!
//! Only what tsumugi does is here: the design's rows for what is not built
//! yet (fonts, window material, rebinding keys, sounds, ...) come with it.

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::settings::{Profile, Settings, TagRule, DATE_FORMATS};
use tsumugi_pane::Palette;

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
    const ALL: [Page; 9] =
        [Page::General, Page::Appearance, Page::Keys, Page::Notifications, Page::Sessions, Page::Tags, Page::Theme, Page::Shell, Page::Advanced];

    fn title(self) -> &'static str {
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
            Page::General => "Startup and the clock.",
            Page::Appearance => "How the panes look. Colours are on the Theme page.",
            Page::Keys => "The window's own keys; everything else goes to the shell.",
            Page::Notifications => "Only while you are not at a tsumugi window. Inside it, rings and tabs say it.",
            Page::Sessions => "How the sidebar orders sessions, and saved sets of choices for new ones.",
            Page::Tags => "Tags a session gets by itself from its folder.",
            Page::Theme => "Colours, applied as you pick.",
            Page::Shell => "How sessions tell tsumugi what they are doing.",
            Page::Advanced => "The server, and where tsumugi keeps its files.",
        }
    }
}

/// The screen while it is open.
#[derive(Default)]
pub struct Screen {
    pub page: Page,
    /// The key the next press goes to (`[keys]`'s name), while one is being
    /// changed.
    pub capturing: Option<&'static str>,
}


/// What the screen was asked to change.
pub enum Change {
    /// One key of `settings.toml`: its table (`None` at the top), its name
    /// and its value as TOML.
    Set(Option<&'static str>, &'static str, String),
    MuteTag(String, bool),
    TestNotification,
    DeleteProfile(String),
    Sort(Sort),
    AlwaysRestore(bool),
    /// Open with the system: the settings folder, or the file.
    OpenFolder,
    OpenFile,
    Copy(String),
    /// Put Claude Code's hooks in its settings.
    AddHooks,
    Close,
}

/// What the screen shows.
pub struct Seen<'a> {
    pub settings: &'a Settings,
    pub themes: &'a [Theme],
    /// The theme in force.
    pub current: Colors,
    pub profiles: &'a [tsumugi_mux::settings::Profile],
    pub tags: &'a [String],
    pub muted_tags: &'a [String],
    pub sort: Sort,
    pub always_restore: bool,
    pub nerd: bool,
    /// The monospace fonts installed, the file the panes are in, and which
    /// of bold, italic and bold italic were found beside it.
    pub font_names: &'a [String],
    pub font_file: Option<String>,
    pub faces: [bool; 3],
    pub server_up: String,
    pub settings_path: String,
    pub state_path: String,
}

pub fn show(ui: &mut egui::Ui, pal: &Palette, screen: &mut Screen, seen: &Seen) -> Vec<Change> {
    let c = seen.current;
    let mut out = Vec::new();
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        out.push(Change::Close);
    }
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, c.bg);
    // The nav down the left.
    let nav = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 220.0, rect.bottom()));
    ui.painter().rect_filled(nav, 0.0, c.side);
    ui.painter().line_segment([nav.right_top(), nav.right_bottom()], egui::Stroke::new(1.0, c.border));
    let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink2(egui::vec2(10.0, 16.0))));
    nav_ui.horizontal(|ui| {
        ui.label(RichText::new("Settings").size(16.0).strong().color(c.strong()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(RichText::new("Close").size(12.0)).frame(false)).on_hover_text("Esc").clicked() {
                out.push(Change::Close);
            }
        });
    });
    nav_ui.add_space(12.0);
    for p in Page::ALL {
        let on = screen.page == p;
        let (r, resp) = nav_ui.allocate_exact_size(egui::vec2(nav_ui.available_width(), 32.0), egui::Sense::click());
        if on {
            nav_ui.painter().rect_filled(r, 6.0, c.chosen());
        } else if resp.hovered() {
            nav_ui.painter().rect_filled(r, 6.0, c.hover());
        }
        let color = if on { c.strong() } else { c.dim };
        nav_ui.painter().text(egui::pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, p.title(), FontId::proportional(13.0), color);
        if resp.clicked() {
            screen.page = p;
        }
    }
    let foot = egui::Rect::from_min_max(egui::pos2(nav.left() + 10.0, nav.bottom() - 46.0), egui::pos2(nav.right() - 10.0, nav.bottom() - 14.0));
    if nav_ui.put(foot, egui::Button::new(RichText::new("Open settings.toml").size(12.5).color(c.dim))).clicked() {
        out.push(Change::OpenFile);
    }

    // The page.
    let body = egui::Rect::from_min_max(egui::pos2(nav.right() + 1.0, rect.top()), rect.max);
    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical().id_salt("prefs-page").auto_shrink([false, false]).show(&mut body_ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 26, bottom: 40 }).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(760.0));
            ui.label(RichText::new(screen.page.title()).size(22.0).strong().color(c.strong()));
            ui.label(RichText::new(screen.page.lead()).size(13.0).color(c.dim));
            ui.add_space(14.0);
            match screen.page {
                Page::General => general(ui, &c, seen, &mut out),
                Page::Appearance => appearance(ui, &c, seen, &mut out),
                Page::Keys => keys(ui, &c, screen, &mut out),
                Page::Notifications => notifications(ui, &c, seen, &mut out),
                Page::Sessions => sessions(ui, &c, seen, &mut out),
                Page::Tags => tags(ui, &c, &seen.settings.tags.rule, &mut out),
                Page::Theme => theme(ui, pal, &c, seen, &mut out),
                Page::Shell => shell(ui, &c, &mut out),
                Page::Advanced => advanced(ui, &c, seen, &mut out),
            }
        });
    });
    out
}

/// A card of rows under a small heading.
fn section(ui: &mut egui::Ui, c: &Colors, head: &str, rows: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    ui.label(RichText::new(head).size(11.0).strong().color(c.dim));
    ui.add_space(4.0);
    egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(18, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        rows(ui);
    });
    ui.add_space(14.0);
}

/// A row: its label and note on the left, its control on the right.
fn row<R>(ui: &mut egui::Ui, c: &Colors, label: &str, note: &str, control: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut r = None;
    ui.horizontal(|ui| {
        ui.set_min_height(46.0);
        // The control first, on the right; the words wrap in what is left,
        // never under it.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r = Some(control(ui));
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.add_space(6.0);
                    ui.label(RichText::new(label).size(13.0).color(c.strong()));
                    if !note.is_empty() {
                        ui.label(RichText::new(note).size(12.0).color(c.dim));
                    }
                    ui.add_space(4.0);
                });
            });
        });
    });
    r.expect("the control ran")
}

/// The design's switch: a pill with a knob, cyan when on.
fn switch(ui: &mut egui::Ui, c: &Colors, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 11.0, if on { c.run } else { c.border_strong() });
    let x = if on { rect.right() - 11.0 } else { rect.left() + 11.0 };
    p.circle_filled(egui::pos2(x, rect.center().y), 8.0, if on { c.on_accent() } else { c.dim });
    resp.clicked()
}

fn general(ui: &mut egui::Ui, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
    section(ui, c, "STARTUP", |ui| {
        let on = seen.always_restore;
        if row(ui, c, "Restore without asking", "After a restart, bring the last sessions back without the Welcome back screen", |ui| switch(ui, c, on)) {
            out.push(Change::AlwaysRestore(!on));
        }
    });
    let k = &seen.settings.clock;
    section(ui, c, "CLOCK", |ui| {
        let set = |key: &'static str, v: bool| Change::Set(Some("clock"), key, v.to_string());
        if row(ui, c, "Show the time in the status bar", "Visible when the window is maximized or full screen", |ui| switch(ui, c, k.show)) {
            out.push(set("show", !k.show));
        }
        ui.separator();
        let mut hour24 = k.hour24;
        row(ui, c, "Time format", "", |ui| {
            egui::ComboBox::from_id_salt("clock-hours").selected_text(if hour24 { "14:32 (24-hour)" } else { "2:32 PM (12-hour)" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut hour24, true, "14:32 (24-hour)");
                ui.selectable_value(&mut hour24, false, "2:32 PM (12-hour)");
            });
        });
        if hour24 != k.hour24 {
            out.push(set("hour24", hour24));
        }
        ui.separator();
        if row(ui, c, "Show the date", "Next to the time", |ui| switch(ui, c, k.date)) {
            out.push(set("date", !k.date));
        }
        ui.separator();
        let mut format = k.date_format.clone();
        row(ui, c, "Date format", "", |ui| {
            egui::ComboBox::from_id_salt("clock-date").selected_text(&format).show_ui(ui, |ui| {
                for f in DATE_FORMATS {
                    ui.selectable_value(&mut format, f.to_owned(), f);
                }
            });
        });
        if format != k.date_format {
            out.push(Change::Set(Some("clock"), "date_format", tsumugi_mux::settings::quote(&format)));
        }
        ui.separator();
        if row(ui, c, "Show the weekday", "", |ui| switch(ui, c, k.weekday)) {
            out.push(set("weekday", !k.weekday));
        }
    });
}

fn appearance(ui: &mut egui::Ui, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
    let w = &seen.settings.window;
    section(ui, c, "WINDOW", |ui| {
        let own = w.own_titlebar();
        let note = if cfg!(target_os = "macos") { "The band runs under the traffic lights (when the window opens next)" } else { "The band is the title bar, with its own buttons; off is the system's frame" };
        if row(ui, c, "tsumugi's own title bar", note, |ui| switch(ui, c, own)) {
            out.push(Change::Set(Some("window"), "titlebar", tsumugi_mux::settings::quote(if own { "system" } else { "tsumugi" })));
        }
        if cfg!(windows) || cfg!(target_os = "macos") {
            ui.separator();
            let mut m = w.material.clone();
            let (note, kinds): (&str, &[&str]) = if cfg!(windows) {
                ("Windows 11: the desktop shows through the band, sidebar and status bar (when the window opens next)", &["none", "mica", "acrylic"])
            } else {
                ("The desktop shows through the band, sidebar and status bar (when the window opens next)", &["none", "vibrancy"])
            };
            row(ui, c, "Material", note, |ui| {
                egui::ComboBox::from_id_salt("material").selected_text(&m).show_ui(ui, |ui| {
                    for k in kinds.iter().copied() {
                        ui.selectable_value(&mut m, k.to_owned(), k);
                    }
                });
            });
            if m != w.material {
                out.push(Change::Set(Some("window"), "material", tsumugi_mux::settings::quote(&m)));
            }
        }
    });
    let f = &seen.settings.font;
    section(ui, c, "FONT", |ui| {
        let mut family = f.family.clone();
        let shown = if family.is_empty() { "Automatic".to_owned() } else { family.clone() };
        row(ui, c, "Font", "Installed monospace fonts; Automatic is the Nerd Font found, else the built-in one", |ui| {
            egui::ComboBox::from_id_salt("font-family").selected_text(shown).width(220.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut family, String::new(), "Automatic");
                for n in seen.font_names {
                    ui.selectable_value(&mut family, n.clone(), n);
                }
            });
        });
        if family != f.family {
            out.push(Change::Set(Some("font"), "family", tsumugi_mux::settings::quote(&family)));
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
        ui.separator();
        let mut size = f.size;
        row(ui, c, "Size", "", |ui| {
            ui.add(egui::Slider::new(&mut size, 8.0..=32.0).step_by(1.0).suffix(" pt"));
        });
        if size != f.size {
            out.push(Change::Set(Some("font"), "size", format!("{size}")));
        }
        ui.separator();
        let mut line = f.line_height;
        row(ui, c, "Line height", "The rows' height for the font's own", |ui| {
            ui.add(egui::Slider::new(&mut line, 0.8..=2.0).step_by(0.05).fixed_decimals(2));
        });
        if line != f.line_height {
            out.push(Change::Set(Some("font"), "line_height", format!("{line:.2}")));
        }
        ui.separator();
        if row(ui, c, "Ligatures", "-> and != as one sign, in a font that has them (Fira Code, JetBrains Mono, Cascadia Code)", |ui| switch(ui, c, f.ligatures)) {
            out.push(Change::Set(Some("font"), "ligatures", (!f.ligatures).to_string()));
        }
    });
    section(ui, c, "PANES", |ui| {
        let mut dim = seen.settings.appearance.dim;
        row(ui, c, "Dim panes without the keys", "How much darker the other panes of a split get", |ui| {
            ui.add(egui::Slider::new(&mut dim, 0..=90).suffix(" %"));
        });
        if dim != seen.settings.appearance.dim {
            out.push(Change::Set(Some("appearance"), "dim", dim.to_string()));
        }
        ui.separator();
        let on = seen.settings.appearance.animations;
        if row(ui, c, "Animations", "Waiting tabs breathe in gold; running ones show a moving cyan line", |ui| switch(ui, c, on)) {
            out.push(Change::Set(Some("appearance"), "animations", (!on).to_string()));
        }
        ui.separator();
        let nerd = if seen.nerd { "Found" } else { "Not found: icons are drawn" };
        row(ui, c, "Nerd Font", "HackGen NF, or a FiraCode, Caskaydia Cove or JetBrains Mono Nerd Font", |ui| {
            ui.label(RichText::new(nerd).size(12.5).color(if seen.nerd { c.done } else { c.dim }));
        });
    });
}

fn keys(ui: &mut egui::Ui, c: &Colors, screen: &mut Screen, out: &mut Vec<Change>) {
    use crate::keys::{NAMED, label};
    // A key being changed takes the next press: Esc leaves it as it was.
    if let Some(name) = screen.capturing {
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
            Some((egui::Key::Escape, m)) if !m.any() => screen.capturing = None,
            Some((k, m)) => {
                out.push(Change::Set(Some("keys"), name, tsumugi_mux::settings::quote(&crate::keys::Chord::pressed(k, m).label())));
                screen.capturing = None;
            }
            None => {}
        }
    }
    let mac = cfg!(target_os = "macos");
    let title = crate::keys::title;
    section(ui, c, "CHANGEABLE", |ui| {
        for (k, (a, name, win, m)) in NAMED.iter().enumerate() {
            if k > 0 {
                ui.separator();
            }
            let own = if mac { *m } else { *win };
            let now = label(*a);
            let note = if now == own { String::new() } else { format!("Its own: {own}") };
            row(ui, c, title(*a), &note, |ui| {
                if now != own && ui.small_button("Its own").clicked() {
                    out.push(Change::Set(Some("keys"), name, tsumugi_mux::settings::quote(own)));
                }
                let waiting = screen.capturing == Some(*name);
                let text = if waiting { "Press a key… (Esc: leave it)".to_owned() } else { now };
                let stroke = if waiting { c.run } else { c.border_strong() };
                let key = egui::Button::new(RichText::new(text).font(FontId::monospace(12.0)).color(c.strong())).fill(c.side).stroke(egui::Stroke::new(1.0, stroke)).corner_radius(6.0);
                if ui.add(key).on_hover_text("Click, then press the key to use (Esc: leave it)").clicked() {
                    screen.capturing = Some(name);
                }
            });
        }
    });
    let k = |win: &'static str, m: &'static str| if mac { m } else { win };
    section(ui, c, "FIXED", |ui| {
        for (k2, (t, key)) in [("The Nth tab", k("Ctrl+Alt+1 … 9", "Cmd+1 … 9")), ("Move between panes", k("Alt+Arrows", "Cmd+Option+Arrows"))].iter().enumerate() {
            if k2 > 0 {
                ui.separator();
            }
            row(ui, c, t, "", |ui| {
                ui.label(RichText::new(*key).font(FontId::monospace(12.0)).color(c.dim));
            });
        }
    });
    ui.label(RichText::new("A key is written to [keys] in settings.toml; \"none\" there gives it back to the shell.").size(12.0).color(c.dim));
}

fn notifications(ui: &mut egui::Ui, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
    let n = &seen.settings.notify;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Send a test").clicked() {
                out.push(Change::TestNotification);
            }
            if ui.button("Reset to defaults").clicked() {
                let d = tsumugi_mux::settings::Notify::default();
                for (key, list) in [("system", d.system), ("taskbar", d.taskbar), ("flash", d.flash), ("sound", d.sound)] {
                    out.push(Change::Set(Some("notify"), key, tsumugi_mux::settings::quote_list(&list)));
                }
            }
        });
    });
    section(ui, c, "WHEN A SESSION…", |ui| {
        let states = [("waiting", "waits for you", c.wait), ("error", "fails", c.err), ("done", "finishes", c.done)];
        egui::Grid::new("notify-table").num_columns(4).min_col_width(120.0).spacing(egui::vec2(16.0, 14.0)).show(ui, |ui| {
            ui.label("");
            for (_, words, color) in states {
                ui.vertical_centered(|ui| ui.label(RichText::new(words).strong().color(color)));
            }
            ui.end_row();
            for (key, label, note, list) in [
                ("system", "System notification", "The OS's own, which a click brings back here", &n.system),
                ("taskbar", "Taskbar count", "A number on the window's taskbar button", &n.taskbar),
                ("flash", "Taskbar flash", "The button lights up until you look", &n.flash),
                ("sound", "Sound", "The system's own sound, once", &n.sound),
            ] {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).color(c.strong()));
                    ui.label(RichText::new(note).size(12.0).color(c.dim));
                });
                for (word, _, _) in states {
                    let on = list.iter().any(|w| w == word);
                    ui.vertical_centered(|ui| {
                        if switch(ui, c, on) {
                            let mut next: Vec<String> = list.iter().filter(|w| *w != word).cloned().collect();
                            if !on {
                                next.push(word.to_owned());
                            }
                            out.push(Change::Set(Some("notify"), key, tsumugi_mux::settings::quote_list(&next)));
                        }
                    });
                }
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("A finish only counts after a run of a minute or more. Sounds are not in this version.").size(12.0).color(c.dim));
    });
    section(ui, c, "QUIET TAGS", |ui| {
        ui.label(RichText::new("Sessions with these tags tell you in the bell only. Click one to quiet it.").size(12.0).color(c.dim));
        ui.add_space(6.0);
        if seen.tags.is_empty() {
            ui.label(RichText::new("No session has a tag yet.").color(c.dim));
        }
        ui.horizontal_wrapped(|ui| {
            for t in seen.tags {
                let quiet = seen.muted_tags.contains(t);
                let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0;
                let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::click());
                let chip = crate::chrome::tag_chip(ui.painter(), r.min, t, quiet);
                if quiet {
                    ui.painter().line_segment([chip.left_center(), chip.right_center()], egui::Stroke::new(1.0, c.dim));
                }
                if resp.on_hover_text(if quiet { "Quiet: click to tell again" } else { "Click to quiet" }).clicked() {
                    out.push(Change::MuteTag(t.clone(), !quiet));
                }
            }
        });
    });
}

fn sessions(ui: &mut egui::Ui, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
    section(ui, c, "SIDEBAR", |ui| {
        let mut sort = seen.sort;
        row(ui, c, "Sort", "Also the button beside SESSIONS", |ui| {
            egui::ComboBox::from_id_salt("prefs-sort").selected_text(sort.label()).show_ui(ui, |ui| {
                for s in Sort::ALL {
                    ui.selectable_value(&mut sort, s, s.label());
                }
            });
        });
        if sort != seen.sort {
            out.push(Change::Sort(sort));
        }
    });
    section(ui, c, "PROFILES", |ui| {
        if seen.profiles.is_empty() {
            ui.label(RichText::new("None yet: tick \"Save as a profile\" in the new-session dialog.").color(c.dim));
        }
        for (k, p) in seen.profiles.iter().enumerate() {
            if k > 0 {
                ui.separator();
            }
            if row(ui, c, &p.name, &profile_words(p), |ui| ui.button("Delete").clicked()) {
                out.push(Change::DeleteProfile(p.name.clone()));
            }
        }
    });
}

fn profile_words(p: &Profile) -> String {
    let start = match p.start.as_str() {
        "resume" => "Resume last",
        "shell" => "Shell",
        _ => "Claude Code",
    };
    let mut s = format!("{start} · {}", p.folder);
    if !p.tags.is_empty() {
        s.push_str(&format!(" · {}", p.tags.join(", ")));
    }
    s
}

fn tags(ui: &mut egui::Ui, c: &Colors, rules: &[TagRule], out: &mut Vec<Change>) {
    section(ui, c, "AUTOMATIC TAGS", |ui| {
        if rules.is_empty() {
            ui.label(RichText::new("No rules yet.").color(c.dim));
        }
        for (k, r) in rules.iter().enumerate() {
            if k > 0 {
                ui.separator();
            }
            row(ui, c, &format!("Folder {}", r.folder), "", |ui| {
                let tag = r.tag.clone();
                let w = ui.fonts_mut(|f| f.layout_no_wrap(tag.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0;
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::hover());
                crate::chrome::tag_chip(ui.painter(), rect.min, &tag, false);
            });
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Rules are [[tags.rule]] in settings.toml: a folder (ending in * for each folder in it) and a tag ({name} for that folder's name).").size(12.0).color(c.dim));
        if ui.button("Open settings.toml").clicked() {
            out.push(Change::OpenFile);
        }
    });
}

fn theme(ui: &mut egui::Ui, pal: &Palette, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
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
    ui.horizontal(|ui| {
        ui.label(RichText::new("Mode").size(12.0).color(c.dim));
        for (k, label) in [("dark", "Dark"), ("light", "Light"), ("system", "Follow OS")] {
            if ui.selectable_label(mode == k, label).clicked() && mode != k {
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
                    for (k, sw) in [t.colors.bg, t.colors.wait, t.colors.run, t.colors.err].iter().enumerate() {
                        let x = r.left() + 10.0 + k as f32 * 10.0;
                        p.rect_filled(egui::Rect::from_min_size(egui::pos2(x, r.center().y - 9.0), egui::vec2(10.0, 18.0)), 0.0, *sw);
                    }
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
            ui.label(RichText::new("PREVIEW").size(11.0).strong().color(c.dim));
            preview(ui, pal);
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
    let card = |y: f32, ring: Color32, fill: Color32, title: &str, words: &str, color: Color32| {
        let r = egui::Rect::from_min_size(egui::pos2(side.left() + 10.0, side.top() + y), egui::vec2(150.0, 52.0));
        p.rect_filled(r, 8.0, fill);
        p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, ring), egui::StrokeKind::Inside);
        p.text(r.left_top() + egui::vec2(10.0, 9.0), egui::Align2::LEFT_TOP, title, FontId::proportional(12.0), c.strong());
        p.text(r.left_top() + egui::vec2(10.0, 29.0), egui::Align2::LEFT_TOP, words, FontId::proportional(11.0), color);
    };
    card(12.0, c.wait, c.wait_bg(), "Approve the edit", "Waiting for you · 2m", c.wait);
    card(72.0, c.run.gamma_multiply(0.6), c.panel, "Split the pane", "Running · 4m", c.dim);
    card(132.0, c.err.gamma_multiply(0.7), c.panel, "Windows CI test", "Error · Exited 101", c.err);
    card(192.0, c.done.gamma_multiply(0.5), c.panel, "Write the scope", "Done · 20m", c.dim);
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

fn shell(ui: &mut egui::Ui, c: &Colors, out: &mut Vec<Change>) {
    let hooks = r#"{
  "hooks": {
    "Notification": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --stdin" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --state done" }] }]
  }
}"#;
    section(ui, c, "CLAUDE CODE HOOKS", |ui| {
        ui.label(RichText::new("In ~/.claude/settings.json, so Claude Code says when it waits or is done, and which conversation to resume after a restart.").size(12.0).color(c.dim));
        ui.add_space(6.0);
        ui.label(RichText::new(hooks).font(FontId::monospace(11.5)).color(c.fg));
        ui.horizontal(|ui| {
            if ui.button("Add them for me").on_hover_text("Into ~/.claude/settings.json, beside what is there; the old file is kept as settings.json.tsumugi-backup").clicked() {
                out.push(Change::AddHooks);
            }
            if ui.button("Copy").clicked() {
                out.push(Change::Copy(hooks.to_owned()));
            }
        });
    });
    let line = if cfg!(windows) { "tsumugi shell-hook pwsh >> $PROFILE" } else { "tsumugi shell-hook bash >> ~/.bashrc" };
    section(ui, c, "SHELL INTEGRATION", |ui| {
        ui.label(RichText::new("The prompt's marks (OSC 133) and its folder (OSC 7), for the state and for restoring a tab where it was.").size(12.0).color(c.dim));
        ui.add_space(6.0);
        ui.label(RichText::new(line).font(FontId::monospace(12.0)).color(c.fg));
        if ui.button("Copy").clicked() {
            out.push(Change::Copy(line.to_owned()));
        }
    });
}

fn advanced(ui: &mut egui::Ui, c: &Colors, seen: &Seen, out: &mut Vec<Change>) {
    section(ui, c, "SERVER", |ui| {
        row(ui, c, "Mux server", "Holds the sessions while no window is open", |ui| {
            ui.label(RichText::new(format!("Running · {}", seen.server_up)).size(12.5).color(c.done));
        });
    });
    section(ui, c, "FILES", |ui| {
        if row(ui, c, "Settings folder", &seen.settings_path, |ui| ui.button("Open").clicked()) {
            out.push(Change::OpenFolder);
        }
        ui.separator();
        row(ui, c, "Saved tabs", &seen.state_path, |ui| {
            ui.label(RichText::new("").size(12.0));
        });
        ui.separator();
        row(ui, c, "Usage data", "", |ui| {
            ui.label(RichText::new("None: tsumugi sends nothing anywhere").size(12.5).color(c.dim));
        });
    });
}
