//! The help (F1, as filer's): every key the window has and what it does, by
//! kind, in two or three columns on one screen. The changeable keys as the
//! settings have them now; the fixed ones from `keys::FIXED`.

use egui::{FontId, RichText};

use crate::keys::{self, Action};
use crate::theme::Colors;

/// A kind of key and its lines: (key, what it does).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The changeable keys, by kind, in the order they read.
const KINDS: [(&str, &[Action]); 4] = [
    ("Sessions and tabs", &[Action::NewTab, Action::CloseTab, Action::Rename, Action::Duplicate, Action::NextTab, Action::PrevTab, Action::NextWaiting]),
    ("Panes", &[Action::SplitRight, Action::SplitDown, Action::Zoom, Action::TypeAll, Action::CopyMode, Action::Find, Action::PrevPrompt, Action::NextPrompt]),
    ("Lists and boxes", &[Action::Search, Action::Waiting, Action::Overview, Action::Notices, Action::Input, Action::Settings, Action::Help]),
    ("The view", &[Action::Rail, Action::FontBigger, Action::FontSmaller, Action::FontReset]),
];

/// Every group, the keys as they are now (`label` is the settings' key for
/// an action, `mac` picks the fixed keys' Mac column).
pub fn groups(label: impl Fn(Action) -> String, mac: bool) -> Vec<Group> {
    let pick = |win: &str, m: &str| if mac { m.to_owned() } else { win.to_owned() };
    let mut out: Vec<Group> = KINDS.iter().map(|(title, actions)| Group { title, rows: actions.iter().map(|a| (label(*a), keys::title(*a).to_owned())).collect() }).collect();
    // The window's fixed keys, the four arrows on one line each.
    out[0].rows.push((pick("Ctrl+Alt+1 … 9", "Cmd+1 … 9"), "The Nth tab".into()));
    out[1].rows.push((pick("Alt+Arrows", "Cmd+Option+Arrows"), "The keys to the pane that way".into()));
    out[1].rows.push((pick("Alt+Shift+Arrows", "Cmd+Ctrl+Arrows"), "Move the divider nearest the pane that way".into()));
    for (section, win, m, what) in keys::FIXED {
        if matches!(*section, "The window, fixed" | "The help") {
            continue;
        }
        let row = (pick(win, m), (*what).to_owned());
        match out.iter_mut().find(|g| g.title == *section) {
            Some(g) => g.rows.push(row),
            None => out.push(Group { title: section, rows: vec![row] }),
        }
    }
    out
}

/// How many columns a width holds.
pub fn columns_for(width: f32) -> usize {
    if width >= 1100.0 {
        3
    } else if width >= 640.0 {
        2
    } else {
        1
    }
}

/// The groups dealt into `n` columns in their order, each about as tall as
/// the others (a title counts as two lines).
pub fn deal(groups: &[Group], n: usize) -> Vec<Vec<usize>> {
    let n = n.max(1);
    let tall = |g: &Group| g.rows.len() + 2;
    let total: usize = groups.iter().map(tall).sum();
    let mut out = vec![Vec::new(); n];
    let (mut col, mut filled) = (0, 0);
    for (i, g) in groups.iter().enumerate() {
        // On to the next column when this one is past its share, and the
        // group would sit better there (more than half of it over).
        let share = total * (col + 1) / n;
        if col + 1 < n && !out[col].is_empty() && filled + tall(g) / 2 > share {
            col += 1;
        }
        out[col].push(i);
        filled += tall(g);
    }
    out
}

/// The help while it is open.
pub struct View {
    /// Opened this frame: the click or key that opened it does not close it.
    pub opening: bool,
}

/// Draw it: false when it closes (Esc, its own key, × or a click outside).
pub fn show(ctx: &egui::Context, view: &mut View, c: &Colors) -> bool {
    let mut keep = true;
    let screen = ctx.content_rect();
    let dim = egui::Area::new(egui::Id::new("help-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
        resp
    });
    let closing_key = ctx.input_mut(|i| {
        let own = i.events.iter().find_map(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } if keys::action(*key, *modifiers) == Some(Action::Help) => Some((*key, *modifiers)),
            _ => None,
        });
        if let Some((k, m)) = own {
            i.consume_key(m, k);
        }
        own.is_some() || i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
    });
    if !view.opening && (dim.inner.clicked() || closing_key) {
        keep = false;
    }
    view.opening = false;
    let all = groups(keys::label, keys::mac());
    let width = (screen.width() - 48.0).clamp(280.0, 1240.0);
    let n = columns_for(width);
    let dealt = deal(&all, n);
    egui::Area::new(egui::Id::new("help")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 24.0)).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(20, 14)).show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Keys").size(15.0).strong().color(c.strong()));
                ui.label(RichText::new("Change them in Settings → Keys").size(12.0).color(c.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Close (Esc)").clicked() {
                        keep = false;
                    }
                });
            });
            ui.separator();
            // On a screen too short for it all, it scrolls rather than run off.
            egui::ScrollArea::vertical().max_height((screen.height() - 110.0).max(160.0)).auto_shrink([false, true]).show(ui, |ui| {
                ui.columns(n, |cols| {
                    for (col, ui) in dealt.iter().zip(cols.iter_mut()) {
                        for &g in col {
                            group(ui, &all[g], c, g);
                        }
                    }
                });
            });
        });
    });
    keep
}

fn group(ui: &mut egui::Ui, g: &Group, c: &Colors, id: usize) {
    ui.add_space(6.0);
    ui.label(RichText::new(g.title.to_uppercase()).size(11.0).strong().color(c.dim));
    ui.add_space(2.0);
    // The keys in a column as wide as the widest of the group's, the words
    // wrapping in what is left.
    let font = FontId::monospace(11.5);
    let key_w = g.rows.iter().map(|(k, _)| ui.fonts_mut(|f| f.layout_no_wrap(k.clone(), font.clone(), c.fg).size().x)).fold(0.0_f32, f32::max).min(ui.available_width() * 0.45);
    egui::Grid::new(("help-group", id)).num_columns(2).spacing(egui::vec2(10.0, 3.0)).show(ui, |ui| {
        for (k, what) in &g.rows {
            ui.allocate_ui_with_layout(egui::vec2(key_w, 16.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(key_w);
                ui.label(RichText::new(k).font(font.clone()).color(c.wait_text()));
            });
            ui.add(egui::Label::new(RichText::new(what).size(12.0).color(c.fg)).wrap());
            ui.end_row();
        }
    });
    ui.add_space(6.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Group> {
        groups(|a| keys::NAMED.iter().find(|(x, ..)| *x == a).map(|(.., win, _)| (*win).to_owned()).unwrap_or_default(), false)
    }

    #[test]
    fn every_key_is_in_the_help_once() {
        let all = all();
        let rows: Vec<&(String, String)> = all.iter().flat_map(|g| &g.rows).collect();
        for (a, ..) in keys::NAMED {
            assert_eq!(rows.iter().filter(|(_, w)| w == keys::title(a)).count(), 1, "{a:?}");
        }
        assert!(rows.iter().any(|(k, w)| k == "F1" && w.contains("help")));
        for (section, _, _, what) in keys::FIXED.iter().filter(|f| !matches!(f.0, "The window, fixed" | "The help")) {
            assert!(all.iter().any(|g| g.title == *section && g.rows.iter().any(|(_, w)| w == what)), "{section}: {what}");
        }
    }

    #[test]
    fn the_groups_are_dealt_in_order_and_about_even() {
        let all = all();
        for n in 1..=3 {
            let dealt = deal(&all, n);
            assert_eq!(dealt.len(), n);
            let order: Vec<usize> = dealt.iter().flatten().copied().collect();
            assert_eq!(order, (0..all.len()).collect::<Vec<_>>());
            let tall: Vec<usize> = dealt.iter().map(|col| col.iter().map(|&g| all[g].rows.len() + 2).sum()).collect();
            let total: usize = tall.iter().sum();
            assert!(tall.iter().all(|t| *t > 0 && *t <= total / n + 14), "{n}: {tall:?}");
        }
        assert_eq!((columns_for(1200.0), columns_for(900.0), columns_for(500.0)), (3, 2, 1));
    }

    /// On a 1280 × 800 window it is all there without scrolling, in three
    /// columns side by side.
    #[test]
    fn it_fits_one_screen() {
        let ctx = egui::Context::default();
        let mut view = View { opening: true };
        let mut texts = Vec::new();
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                assert!(show(ui.ctx(), &mut view, &crate::theme::colors()));
            });
            out.textures_delta.clear();
            texts.clear();
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
        }
        let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
        assert!(bottom < 800.0, "runs to {bottom}");
        let heads: Vec<f32> = all().iter().filter_map(|g| texts.iter().find(|(t, _)| *t == g.title.to_uppercase()).map(|(_, r)| r.left())).collect();
        let mut xs = heads.clone();
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        assert_eq!(xs.len(), 3, "{heads:?}");
    }
}
