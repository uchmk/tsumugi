//! The help (F1, as filer's): every key the window has and what it does, by
//! kind, in the middle of the window. It fits without scrolling: when it runs
//! over, it takes another column, then smaller letters (down to `SMALLEST`),
//! and only scrolls below that. The changeable keys as the settings have them
//! now; the fixed ones from `keys::FIXED`.

use egui::{FontId, RichText};

use crate::i18n::{tr, tr_desc};
use crate::keys::{self, Action};
use crate::theme::Colors;

/// A kind of key and its lines: (key, what it does). In English, as keys.rs
/// has them; translated (`[keydesc]`) when drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The changeable keys, by kind, in the order they read.
const KINDS: [(&str, &[Action]); 4] = [
    ("Sessions and tabs", &[Action::NewTab, Action::CloseTab, Action::Rename, Action::Duplicate, Action::NextTab, Action::PrevTab, Action::NextWaiting]),
    ("Panes", &[Action::SplitRight, Action::SplitDown, Action::Zoom, Action::TypeAll, Action::CopyMode, Action::QuickSelect, Action::Find, Action::PrevPrompt, Action::NextPrompt, Action::SwapPane, Action::Equalize, Action::PaneToTab, Action::Record, Action::WorkLog, Action::CopyOutput]),
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

/// The smallest the letters get before it scrolls instead.
pub const SMALLEST: f32 = 0.7;

/// How many columns a width can take at most when it has to (300 each).
pub fn most_columns(width: f32) -> usize {
    ((width / 300.0) as usize).clamp(1, 4)
}

/// The help while it is open.
pub struct View {
    /// Opened this frame: the click or key that opened it does not close it.
    pub opening: bool,
    /// The columns and the letters' scale it is drawn with.
    cols: usize,
    scale: f32,
    /// The window's size the fit was found for.
    screen: egui::Vec2,
    /// It fits (or can get no smaller): shown, and left as it is.
    settled: bool,
    /// Drawn once as it is now: the first frame after a change wraps in the
    /// widths of the one before, so only the second is measured.
    drawn: bool,
}

impl View {
    pub fn new() -> Self {
        Self { opening: true, cols: 0, scale: 1.0, screen: egui::Vec2::ZERO, settled: false, drawn: false }
    }

    /// The next try after the content came out `tall` for `room`.
    fn refit(&mut self, tall: f32, room: f32, most: usize) {
        if tall <= room + 0.5 {
            self.settled = true;
        } else if self.cols < most {
            self.cols += 1;
        } else if self.scale > SMALLEST {
            // About as much smaller as it is over, at least a step.
            self.scale = (self.scale * room / tall).min(self.scale - 0.02).max(SMALLEST);
        } else {
            self.settled = true;
        }
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
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
    let width = (screen.width() - 48.0).clamp(280.0, 1600.0);
    if view.screen != screen.size() {
        // A new window size: start again from the columns it holds as is.
        view.screen = screen.size();
        view.cols = columns_for(width);
        view.scale = 1.0;
        view.settled = false;
        view.drawn = false;
    }
    let (n, s) = (view.cols, view.scale);
    let dealt = deal(&all, n);
    let room = (screen.height() - 110.0).max(160.0);
    let mut tall = 0.0;
    egui::Area::new(egui::Id::new("help")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
        // Not shown until it fits, so it does not jump about while it settles.
        if !view.settled {
            ui.set_invisible();
        }
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(20, 14)).show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("help.title")).size(15.0).strong().color(c.strong()));
                ui.label(RichText::new(tr("help.lead")).size(12.0).color(c.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text(tr("help.close")).clicked() {
                        keep = false;
                    }
                });
            });
            ui.separator();
            // Even at the smallest letters too short a screen scrolls rather
            // than run off. As tall as it needs: the area alone would keep the
            // height of its first frame, and scroll under it.
            egui::ScrollArea::vertical().max_height(room).min_scrolled_height(room).auto_shrink([false, true]).show(ui, |ui| {
                tall = ui
                    .scope(|ui| {
                        ui.columns(n, |cols| {
                            for (col, ui) in dealt.iter().zip(cols.iter_mut()) {
                                for &g in col {
                                    group(ui, &all[g], c, g, s);
                                }
                            }
                        });
                    })
                    .response
                    .rect
                    .height();
            });
        });
    });
    if !view.settled {
        if view.drawn {
            view.refit(tall, room, most_columns(width));
        }
        view.drawn = !view.drawn;
        ctx.request_repaint();
    }
    keep
}

/// One group, its letters and spaces times `s`.
fn group(ui: &mut egui::Ui, g: &Group, c: &Colors, id: usize, s: f32) {
    ui.add_space(6.0 * s);
    ui.label(RichText::new(tr_desc(g.title).to_uppercase()).size(11.0 * s).strong().color(c.dim));
    ui.add_space(2.0 * s);
    // The keys in a column as wide as the widest of the group's, the words
    // wrapping in what is left.
    let font = FontId::monospace(11.5 * s);
    let key_w = g.rows.iter().map(|(k, _)| ui.fonts_mut(|f| f.layout_no_wrap(k.clone(), font.clone(), c.fg).size().x)).fold(0.0_f32, f32::max).min(ui.available_width() * 0.45);
    // The rows as low as the letters: egui's 18 pixels would keep them as
    // tall at the smallest letters, and it would scroll anyway.
    egui::Grid::new(("help-group", id)).num_columns(2).min_row_height(16.0 * s).spacing(egui::vec2(10.0, 3.0) * s).show(ui, |ui| {
        for (k, what) in &g.rows {
            ui.allocate_ui_with_layout(egui::vec2(key_w, 16.0 * s), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(key_w);
                ui.label(RichText::new(k).font(font.clone()).color(c.wait_text()));
            });
            ui.add(egui::Label::new(RichText::new(tr_desc(what)).size(12.0 * s).color(c.fg)).wrap());
            ui.end_row();
        }
    });
    ui.add_space(6.0 * s);
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

    /// Every title and line the help draws, and whose key a clash names on
    /// the settings screen, has a `[keydesc]` translation.
    #[test]
    fn every_line_is_translated() {
        let all = all();
        let mut descs: Vec<&str> = all.iter().flat_map(|g| std::iter::once(g.title).chain(g.rows.iter().map(|(_, w)| w.as_str()))).collect();
        descs.extend(keys::OTHERS.iter().map(|(_, whose)| *whose));
        let bad = ito_i18n::check::key_descriptions(crate::i18n::AVAILABLE, &descs);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn the_groups_are_dealt_in_order_and_about_even() {
        let all = all();
        for n in 1..=4 {
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

    /// Draws it on a `w` × `h` window until it settles: the texts and where.
    fn draw(w: f32, h: f32) -> (View, Vec<(String, egui::Rect)>) {
        let ctx = egui::Context::default();
        let mut view = View::new();
        let mut texts = Vec::new();
        for _ in 0..30 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))), ..Default::default() };
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
        (view, texts)
    }

    /// On a 1280 × 800 window it is all there without scrolling, in three
    /// columns or more side by side, in the middle.
    #[test]
    fn it_fits_one_screen() {
        let (view, texts) = draw(1280.0, 800.0);
        assert!(view.settled && view.scale == 1.0, "cols {}, scale {}", view.cols, view.scale);
        let top = texts.iter().map(|(_, r)| r.top()).fold(f32::MAX, f32::min);
        let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
        assert!(bottom < 800.0, "runs to {bottom}");
        assert!(((800.0 - bottom) - top).abs() < 60.0, "not in the middle: {top} to {bottom}");
        let heads: Vec<f32> = all().iter().filter_map(|g| texts.iter().find(|(t, _)| *t == g.title.to_uppercase()).map(|(_, r)| r.left())).collect();
        let mut xs = heads.clone();
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        assert!(xs.len() >= 3, "{heads:?}");
    }

    /// A shorter or narrower window takes more columns or smaller letters
    /// instead of scrolling: it settles before the smallest letters, so it
    /// fits (the real machine found a scrollbar at 1000 × 700 with all of
    /// them at the smallest, 5.10: the rows kept egui's 18-pixel height, and
    /// the first frame after a change was measured in the last one's widths).
    #[test]
    fn a_small_window_still_holds_it_all() {
        for (w, h) in [(1280.0, 600.0), (1000.0, 700.0), (1366.0, 640.0)] {
            let (view, texts) = draw(w, h);
            assert!(view.settled && view.scale > SMALLEST + 0.05, "{w}x{h}: cols {}, scale {}", view.cols, view.scale);
            let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
            assert!(bottom < h, "{w}x{h}: runs to {bottom} (cols {}, scale {})", view.cols, view.scale);
            for g in all() {
                assert!(texts.iter().any(|(t, _)| *t == g.title.to_uppercase()), "{w}x{h}: {} is not drawn (cols {}, scale {})", g.title, view.cols, view.scale);
            }
        }
    }

    #[test]
    fn it_takes_columns_before_smaller_letters() {
        let mut v = View::new();
        v.cols = 2;
        v.refit(900.0, 600.0, 3);
        assert_eq!((v.cols, v.scale, v.settled), (3, 1.0, false));
        v.refit(900.0, 600.0, 3);
        assert!(v.scale < 0.7 + 0.01 && !v.settled);
        v.refit(650.0, 600.0, 3);
        assert!(v.settled);
        let mut v = View::new();
        v.cols = 3;
        v.refit(610.0, 600.0, 3);
        assert!(v.scale < 1.0 && v.scale > 0.95);
    }
}
