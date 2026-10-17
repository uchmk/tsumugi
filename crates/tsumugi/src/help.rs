//! The help (F1, as filer's): every key the window has and what it does, by

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
    (
        "Sessions and tabs",
        &[Action::NewTab, Action::CloseTab, Action::Rename, Action::Duplicate, Action::NextTab, Action::PrevTab, Action::NextWaiting],
    ),
    (
        "Panes",
        &[
            Action::SplitRight,
            Action::SplitDown,
            Action::Zoom,
            Action::TypeAll,
            Action::CopyMode,
            Action::QuickSelect,
            Action::Find,
            Action::PrevPrompt,
            Action::NextPrompt,
            Action::SwapPane,
            Action::Equalize,
            Action::PaneToTab,
            Action::JoinTab,
            Action::Record,
            Action::WorkLog,
            Action::CopyOutput,
            Action::SaveOutput,
            Action::Changes,
            Action::Browser,
            Action::ReadOnly,
            Action::SaveLayout,
        ],
    ),
    (
        "Lists and boxes",
        &[
            Action::Search,
            Action::Waiting,
            Action::Overview,
            Action::Notices,
            Action::Input,
            Action::Settings,
            Action::Help,
            Action::BugReport,
            Action::ClosedList,
        ],
    ),
    ("The view", &[Action::Rail, Action::FontBigger, Action::FontSmaller, Action::FontReset, Action::RestartServer]),
];

/// Every group, the keys as they are now (`label` is the settings' key for
/// an action, `mac` picks the fixed keys' Mac column).
pub fn groups(label: impl Fn(Action) -> String, mac: bool) -> Vec<Group> {
    let pick = |win: &str, m: &str| if mac { m.to_owned() } else { win.to_owned() };
    // An action with no key stays out: the palette and the settings have it.
    let mut out: Vec<Group> = KINDS
        .iter()
        .map(|(title, actions)| Group {
            title,
            rows: actions.iter().map(|a| (label(*a), keys::title(*a).to_owned())).filter(|(k, _)| !k.is_empty() && k != "none").collect(),
        })
        .collect();
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

/// The smallest the letters get before it scrolls instead.
pub const SMALLEST: f32 = 0.7;

/// The help while it is open.
pub struct View {
    /// Opened this frame: the click or key that opened it does not close it.
    pub opening: bool,
    /// The first line shown, when it is too tall even at the smallest letters.
    scroll: usize,
}

impl View {
    pub fn new() -> Self {
        Self { opening: true, scroll: 0 }
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

/// The groups as ito-help lays them out: titles in capitals, everything translated.
fn translated(all: &[Group]) -> Vec<ito_help::Group> {
    all.iter()
        .map(|g| ito_help::Group {
            title: tr_desc(g.title).to_uppercase(),
            rows: g.rows.iter().map(|(k, w)| (k.clone(), tr_desc(w).to_owned())).collect(),
        })
        .collect()
}

/// Draw it: false when it closes (Esc, its own key, × or a click outside).
/// The help's letters and rows follow the panes' (`ito_theme::set_text_zoom`,
/// the font keys), and what no longer fits scrolls.
pub fn show(ctx: &egui::Context, view: &mut View, c: &Colors) -> bool {
    let zoom = ito_theme::text_zoom(ctx);
    let row_h = ROW_H * zoom;
    let mut keep = true;
    let screen = ctx.content_rect();
    let dim = egui::Area::new(egui::Id::new("help-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
        resp
    });
    let closing_key = ctx.input_mut(|i| {
        let own = i.events.iter().find_map(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } if keys::action(*key, *modifiers) == Some(Action::Help) => {
                Some((*key, *modifiers))
            }
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
    let all = translated(&groups(keys::label, keys::mac()));
    let width = (screen.width() - 48.0).clamp(280.0, 1600.0);
    let room = (screen.height() - 110.0).max(160.0);
    let style = ito_help::Style {
        font: FontId::proportional(12.0 * zoom),
        row_h,
        head: c.dim,
        keys: c.wait_text(),
        desc: c.fg,
        warn: c.wait_text(),
        dim: c.dim,
        bar: c.dim,
    };
    let plan = ito_help::plan(&all, room, row_h, ((width / (300.0 * zoom)) as usize).clamp(1, 4), SMALLEST);
    let tall = plan.columns.iter().map(Vec::len).max().unwrap_or(0) as f32 * row_h * plan.scale;
    egui::Area::new(egui::Id::new("help")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(
        ctx,
        |ui| {
            egui::Frame::NONE
                .fill(c.panel)
                .stroke(egui::Stroke::new(1.0, c.border_strong()))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(20, 14))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("help.title")).size(15.0 * zoom).strong().color(c.strong()));
                        ui.label(RichText::new(tr("help.lead")).size(12.0 * zoom).color(c.dim));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("×").on_hover_text(tr("help.close")).clicked() {
                                keep = false;
                            }
                        });
                    });
                    ui.separator();
                    let (body, resp) = ui.allocate_exact_size(egui::vec2(width, tall.min(room)), egui::Sense::hover());
                    if plan.scrolls && resp.hovered() {
                        let dy = ctx.input(|i| i.smooth_scroll_delta.y);
                        if dy != 0.0 {
                            let lines = (dy.abs() / (row_h * plan.scale)).ceil() as usize;
                            view.scroll = if dy > 0.0 { view.scroll.saturating_sub(lines) } else { view.scroll + lines };
                        }
                    }
                    let drawn = ito_help::draw_plan(ui.painter(), body, &plan, view.scroll, &style);
                    view.scroll = drawn.scroll;
                });
        },
    );
    keep
}

/// A row's height at full size.
const ROW_H: f32 = 18.0;

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
        for (a, ..) in keys::NAMED.into_iter().filter(|n| !n.2.is_empty()) {
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
        let mut descs: Vec<&str> =
            all.iter().flat_map(|g| std::iter::once(g.title).chain(g.rows.iter().map(|(_, w)| w.as_str()))).collect();
        descs.extend(keys::OTHERS.iter().map(|(_, whose)| *whose));
        let bad = ito_i18n::check::key_descriptions(crate::i18n::AVAILABLE, &descs);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Draws it on a `w` × `h` window until it settles: the texts and where.
    fn draw(w: f32, h: f32) -> (View, Vec<(String, egui::Rect)>) {
        draw_in(egui::Context::default(), w, h, 1.0)
    }

    fn draw_in(ctx: egui::Context, w: f32, h: f32, zoom: f32) -> (View, Vec<(String, egui::Rect)>) {
        let mut view = View::new();
        let mut texts = Vec::new();
        ito_theme::set_text_zoom(&ctx, zoom);
        for _ in 0..3 {
            let input =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))), ..Default::default() };
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
        assert_eq!(view.scroll, 0);
        let top = texts.iter().map(|(_, r)| r.top()).fold(f32::MAX, f32::min);
        let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
        assert!(bottom < 800.0, "runs to {bottom}");
        assert!(((800.0 - bottom) - top).abs() < 60.0, "not in the middle: {top} to {bottom}");
        let heads: Vec<f32> =
            all().iter().filter_map(|g| texts.iter().find(|(t, _)| *t == g.title.to_uppercase()).map(|(_, r)| r.left())).collect();
        let mut xs = heads.clone();
        xs.sort_by(f32::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        assert!(xs.len() >= 3, "{heads:?}");
    }

    /// A shorter or narrower window takes more columns or smaller letters
    /// instead of scrolling (5.10).
    #[test]
    fn a_small_window_still_holds_it_all() {
        for (w, h) in [(1280.0, 600.0), (1000.0, 700.0), (1366.0, 640.0)] {
            let (view, texts) = draw(w, h);
            assert_eq!(view.scroll, 0, "{w}x{h}");
            let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
            assert!(bottom < h, "{w}x{h}: runs to {bottom}");
            for g in all() {
                assert!(texts.iter().any(|(t, _)| *t == g.title.to_uppercase()), "{w}x{h}: {} is not drawn", g.title);
            }
        }
    }

    /// With the letters made bigger (the font keys) the help's follow, and
    /// what no longer fits scrolls instead of running off the window.
    #[test]
    fn it_follows_the_font_keys() {
        let height = |zoom: f32| {
            let (_, texts) = draw_in(egui::Context::default(), 1280.0, 800.0, zoom);
            let t = texts.iter().find(|(t, _)| t.starts_with("SESSIONS")).expect("a title").1;
            let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
            assert!(bottom < 800.0, "zoom {zoom}: runs to {bottom}");
            t.height()
        };
        // Bigger letters are shrunk back by the plan until `SMALLEST`, then it scrolls.
        assert!(height(2.0) > height(1.0) * 1.2);
        assert!(height(0.5) < height(1.0));
    }

    /// In Japanese, with the fonts tsumugi draws it in, it fits too: the
    /// real machine found the last column running off the bottom at
    /// 1280 × 800 (5.10), its lines wrapping taller frames after it was
    /// measured.
    #[test]
    fn it_fits_in_japanese() {
        crate::i18n::set_language_here("ja");
        let fonts = crate::fonts::load("", Default::default()).defs;
        for (w, h) in [(1280.0, 800.0), (1280.0, 600.0), (1000.0, 700.0), (1366.0, 640.0)] {
            let ctx = egui::Context::default();
            ctx.set_fonts(fonts.clone());
            let (view, texts) = draw_in(ctx, w, h, 1.0);
            assert_eq!(view.scroll, 0, "{w}x{h}");
            let bottom = texts.iter().map(|(_, r)| r.bottom()).fold(0.0_f32, f32::max);
            assert!(bottom < h, "{w}x{h}: runs to {bottom}");
            for g in all() {
                let title = crate::i18n::tr_desc(g.title).to_uppercase();
                assert!(texts.iter().any(|(t, _)| *t == title), "{w}x{h}: {title} is not drawn");
            }
        }
    }
}
