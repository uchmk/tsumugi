//! What the window shows around the panes: the status bar, the bell and its
//! list, and the "Welcome back" screen after a restart (the design's 1d, 1h
//! and 8). Each draws and hands back what was asked, and changes nothing.

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::{Info, Notice, SessionId, State};
use tsumugi_pane::Palette;

pub const GOLD: Color32 = Color32::from_rgb(0xe8, 0xc8, 0x7a);
pub const CYAN: Color32 = Color32::from_rgb(0x6f, 0xd0, 0xd0);
pub const RED: Color32 = Color32::from_rgb(0xf0, 0x71, 0x78);
pub const GREEN: Color32 = Color32::from_rgb(0x8e, 0xd0, 0x8e);
pub const GREY: Color32 = Color32::from_rgb(0x7f, 0x87, 0x98);

/// The four state colours (docs/v1-scope.md 1k): waiting yellow, running
/// cyan, error red, done green -- filer's own yellow, cyan, red and green.
pub fn state_color(state: State) -> Color32 {
    match state {
        State::Waiting | State::MaybeWaiting => GOLD,
        State::Running => CYAN,
        State::Error => RED,
        State::Done => GREEN,
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A span of time as the design writes it: `14s`, `4m 12s`, `2m` past ten
/// minutes, `1h 04m`, `3d 2h`.
pub fn elapsed(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{s}s"),
        60..=599 => format!("{}m {:02}s", s / 60, s % 60),
        600..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
        _ => format!("{}d {}h", s / 86_400, s % 86_400 / 3600),
    }
}

/// The third line of a sidebar row, and a pane's heading: what the session
/// is doing, in words, and for how long (the design's sidebar, 3).
pub fn state_words(info: &Info, now: u64) -> String {
    let t = elapsed(now.saturating_sub(info.since_ms));
    match info.state {
        State::Waiting => format!("Waiting for you · {t}"),
        State::MaybeWaiting => format!("Quiet for {t} · probably waiting"),
        State::Running => format!("Running · {t}"),
        State::Error => format!("Error · {t}"),
        State::Done => format!("Done · {t}"),
    }
}

/// The git branch mark drawn, for a machine with no Nerd Font: `⎇` is in few
/// of the fonts there are (it came out as a box), so it is three rings and
/// two strokes, the way GitHub and VS Code draw it, in a square of `rect`.
pub fn branch_icon(p: &egui::Painter, rect: egui::Rect, color: Color32) {
    let s = rect.height().min(rect.width());
    let o = rect.center() - egui::vec2(s / 2.0, s / 2.0);
    let at = |x: f32, y: f32| o + egui::vec2(x * s, y * s);
    let stroke = egui::Stroke::new((s / 9.0).max(1.1), color);
    let r = s * 0.13;
    // The trunk, ring to ring, and the branch curving off it to its own ring.
    p.circle_stroke(at(0.3, 0.17), r, stroke);
    p.circle_stroke(at(0.3, 0.83), r, stroke);
    p.circle_stroke(at(0.75, 0.3), r, stroke);
    p.line_segment([at(0.3, 0.17 + 0.13), at(0.3, 0.83 - 0.13)], stroke);
    let curve = egui::epaint::CubicBezierShape::from_points_stroke(
        [at(0.75, 0.3 + 0.13), at(0.75, 0.62), at(0.3, 0.5), at(0.3, 0.7)],
        false,
        Color32::TRANSPARENT,
        stroke,
    );
    p.add(curve);
}

/// The branch mark: the Nerd Font's character when one is installed (as in
/// filer), else drawn.
pub fn branch_mark(p: &egui::Painter, rect: egui::Rect, color: Color32, nerd: bool) {
    if nerd {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, crate::fonts::BRANCH, FontId::monospace(rect.height() + 1.0), color);
    } else {
        branch_icon(p, rect, color);
    }
}

/// What the status bar was clicked for.
pub enum StatusClick {
    Bell,
}

/// The status bar along the bottom (1d): the server and how long it has been
/// up, how many sessions are in each state, the folder and branch of the pane
/// with the keys, its shell and size, and the clock (1n).
pub fn status_bar(
    ui: &mut egui::Ui,
    pal: &Palette,
    sessions: &[Info],
    focus: Option<&Info>,
    size: Option<(usize, usize)>,
    up_ms: u64,
    nerd: bool,
) -> Option<StatusClick> {
    let mut click = None;
    let small = |t: String, c: Color32| RichText::new(t).font(FontId::proportional(11.5)).color(c);
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        ui.label(small(format!("mux · up {}", elapsed(up_ms)), pal.fg_dim));
        ui.add_space(14.0);
        let count = |s: State| sessions.iter().filter(|i| i.state == s || (s == State::Waiting && i.state == State::MaybeWaiting)).count();
        for (state, word) in [(State::Waiting, "waiting"), (State::Running, "running"), (State::Error, "error")] {
            let n = count(state);
            if n == 0 {
                continue;
            }
            let r = ui.add(egui::Label::new(small(format!("● {n} {word}"), state_color(state))).sense(egui::Sense::click()));
            if r.on_hover_text("Open the notification list").clicked() {
                click = Some(StatusClick::Bell);
            }
            ui.add_space(6.0);
        }
        if let Some(i) = focus {
            ui.add_space(10.0);
            ui.label(small(crate::home_short(&i.cwd), pal.fg));
            if !i.branch.is_empty() {
                ui.add_space(4.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                branch_mark(ui.painter(), r, pal.fg_dim, nerd);
                ui.label(small(i.branch.clone(), pal.fg));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(small(chrono::Local::now().format("%Y/%m/%d (%a) %H:%M").to_string(), pal.fg));
            ui.add_space(12.0);
            ui.label(small("UTF-8".into(), pal.fg_dim));
            if let (Some(i), Some((cols, lines))) = (focus, size) {
                ui.add_space(12.0);
                ui.label(small(format!("{} · {cols}×{lines}", crate::program_name(&i.command)), pal.fg_dim));
            }
        });
    });
    click
}

/// The bell: a button with the number of unread notifications on it, gold,
/// or red while one of them is an error.
pub fn bell(ui: &mut egui::Ui, pal: &Palette, notices: &[Notice]) -> egui::Response {
    let unread: Vec<&Notice> = notices.iter().filter(|n| !n.read).collect();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 24.0), egui::Sense::click());
    let resp = resp.on_hover_text(format!("Notifications ({} unread)", unread.len()));
    let p = ui.painter_at(rect.expand(6.0));
    if resp.hovered() {
        p.rect_filled(rect, 6.0, pal.selection.gamma_multiply(0.5));
    }
    // A bell, drawn: the cup, its lip and the clapper.
    let c = rect.center() + egui::vec2(0.0, -1.0);
    let stroke = egui::Stroke::new(1.4, if unread.is_empty() { pal.fg_dim } else { pal.fg });
    let cup = vec![
        c + egui::vec2(-5.5, 4.0),
        c + egui::vec2(-4.5, -1.0),
        c + egui::vec2(-3.0, -4.5),
        c + egui::vec2(0.0, -5.5),
        c + egui::vec2(3.0, -4.5),
        c + egui::vec2(4.5, -1.0),
        c + egui::vec2(5.5, 4.0),
    ];
    p.add(egui::Shape::line(cup, stroke));
    p.line_segment([c + egui::vec2(-6.5, 4.0), c + egui::vec2(6.5, 4.0)], stroke);
    p.circle_filled(c + egui::vec2(0.0, 6.2), 1.4, stroke.color);
    if !unread.is_empty() {
        let color = if unread.iter().any(|n| n.state == State::Error) { RED } else { GOLD };
        let text = if unread.len() > 9 { "9+".to_owned() } else { unread.len().to_string() };
        let at = rect.right_top() + egui::vec2(-5.0, 5.0);
        p.circle_filled(at, 7.0, color);
        p.text(at, egui::Align2::CENTER_CENTER, text, FontId::proportional(10.0), Color32::from_rgb(0x1a, 0x16, 0x08));
    }
    resp
}

/// A tag's colours, fill and text: picked from its name, so a tag is the
/// same colour in every window and after a restart. The first three are the
/// design's (`claude`, `review`, `filer`).
pub fn tag_colors(tag: &str) -> (Color32, Color32) {
    type Rgb = (u8, u8, u8);
    const COLORS: [(Rgb, Rgb); 8] = [
        ((0x4a, 0x4f, 0x5c), (0xee, 0xf0, 0xf4)),
        ((0x5e, 0x4a, 0x86), (0xf3, 0xec, 0xff)),
        ((0x2f, 0x6b, 0x66), (0xe6, 0xff, 0xfb)),
        ((0x7a, 0x5a, 0x22), (0xff, 0xf3, 0xdc)),
        ((0x2f, 0x4f, 0x7f), (0xe6, 0xef, 0xff)),
        ((0x7a, 0x34, 0x3a), (0xff, 0xe8, 0xea)),
        ((0x3d, 0x6b, 0x3a), (0xea, 0xff, 0xe6)),
        ((0x7a, 0x3a, 0x5e), (0xff, 0xe8, 0xf4)),
    ];
    // FNV-1a: the same on every machine, unlike std's hasher.
    let h = tag.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let ((r, g, b), (r2, g2, b2)) = COLORS[h as usize % COLORS.len()];
    (Color32::from_rgb(r, g, b), Color32::from_rgb(r2, g2, b2))
}

/// A tag's chip at `at` (its left top), filled like the design's; its
/// rectangle. `faded`: drawn dim, for a muted tag.
pub fn tag_chip(p: &egui::Painter, at: egui::Pos2, tag: &str, faded: bool) -> egui::Rect {
    let (fill, text) = tag_colors(tag);
    let (fill, text) = if faded { (fill.gamma_multiply(0.45), text.gamma_multiply(0.6)) } else { (fill, text) };
    let galley = p.layout_no_wrap(tag.to_owned(), FontId::proportional(11.0), text);
    let rect = egui::Rect::from_min_size(at, egui::vec2(galley.size().x + 12.0, 16.0));
    p.rect_filled(rect, 4.0, fill);
    p.galley(egui::pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0), galley, text);
    rect
}

/// `+N` for the tags that did not fit.
pub fn more_chip(p: &egui::Painter, at: egui::Pos2, n: usize, color: Color32) -> egui::Rect {
    let galley = p.layout_no_wrap(format!("+{n}"), FontId::proportional(11.0), color);
    let rect = egui::Rect::from_min_size(at, egui::vec2(galley.size().x + 12.0, 16.0));
    p.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4b)), egui::StrokeKind::Inside);
    p.galley(egui::pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0), galley, color);
    rect
}

/// The band along the top of the window (the design's 1c): the name, the
/// search box in the middle, and the tags of the session with the keys on
/// the right, three and `+N`. When the window is narrow the tags give way
/// first, then the box shrinks to its magnifier (1o). Whether the box was
/// clicked.
pub fn top_band(ui: &mut egui::Ui, pal: &Palette, tags: &[String], muted_tags: &[String]) -> bool {
    let rect = ui.max_rect();
    let p = ui.painter().clone();
    p.line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, Color32::from_rgb(0x23, 0x26, 0x2e)));
    // The mark: two threads, cyan and gold (the design's 10 A).
    let o = egui::pos2(rect.left() + 16.0, rect.center().y);
    let wave = |amp: f32, color: Color32| {
        let pts: Vec<egui::Pos2> = (0..=16).map(|k| {
            let x = k as f32;
            o + egui::vec2(x, -amp * ((x / 16.0) * std::f32::consts::TAU).cos())
        }).collect();
        p.add(egui::Shape::line(pts, egui::Stroke::new(1.6, color)));
    };
    wave(4.0, CYAN);
    wave(2.0, GOLD);
    let name = p.layout_no_wrap("tsumugi".into(), FontId::proportional(13.0), Color32::from_rgb(0xe4, 0xe8, 0xf0));
    let name_right = o.x + 24.0 + name.size().x;
    p.galley(egui::pos2(o.x + 24.0, rect.center().y - name.size().y / 2.0), name, Color32::WHITE);

    // The tags, right to left, as many as fit with the box at its widest.
    let chip_w = |t: &str| p.layout_no_wrap(t.to_owned(), FontId::proportional(11.0), pal.fg).size().x + 12.0;
    let full: f32 = tags.iter().take(3).map(|t| chip_w(t) + 5.0).sum::<f32>() + if tags.len() > 3 { 34.0 } else { 0.0 };
    let room = rect.width() - (name_right - rect.left()) - 32.0;
    let box_w = 420.0f32.min(room - full - 16.0);
    let (show_tags, box_w) = if box_w >= 200.0 { (true, box_w) } else { (false, (room - 34.0).min(420.0)) };
    if show_tags && !tags.is_empty() {
        let mut x = rect.right() - 12.0 - full;
        for t in tags.iter().take(3) {
            let r = tag_chip(&p, egui::pos2(x, rect.center().y - 8.0), t, muted_tags.contains(t));
            x = r.right() + 5.0;
        }
        if tags.len() > 3 {
            let r = more_chip(&p, egui::pos2(x, rect.center().y - 8.0), tags.len() - 3, pal.fg_dim);
            ui.interact(r, ui.id().with("more-tags"), egui::Sense::hover()).on_hover_text(tags[3..].join(", "));
        }
    }

    // The box, or only its magnifier when there is no room.
    let compact = box_w < 160.0;
    let w = if compact { 30.0 } else { box_w };
    let center_x = (name_right + rect.right() - if show_tags { full } else { 0.0 }) / 2.0;
    let r = egui::Rect::from_center_size(egui::pos2(center_x.max(name_right + 12.0 + w / 2.0), rect.center().y), egui::vec2(w, 26.0));
    let resp = ui.interact(r, ui.id().with("search"), egui::Sense::click()).on_hover_text("Search sessions, folders and commands");
    let fill = if resp.hovered() { Color32::from_rgb(0x22, 0x26, 0x2e) } else { Color32::from_rgb(0x1b, 0x1e, 0x24) };
    p.rect_filled(r, 6.0, fill);
    p.rect_stroke(r, 6.0, egui::Stroke::new(1.0, Color32::from_rgb(0x2c, 0x30, 0x39)), egui::StrokeKind::Inside);
    let glass = egui::pos2(r.left() + 15.0, r.center().y - 1.0);
    let stroke = egui::Stroke::new(1.3, pal.fg_dim);
    p.circle_stroke(glass, 4.0, stroke);
    p.line_segment([glass + egui::vec2(3.0, 3.0), glass + egui::vec2(6.0, 6.0)], stroke);
    if !compact {
        let key = if cfg!(target_os = "macos") { "Cmd+Shift+P" } else { "Ctrl+Shift+P" };
        let key = p.layout_no_wrap(key.into(), FontId::monospace(11.0), GREY);
        let key_x = r.right() - 10.0 - key.size().x;
        let mut job = egui::text::LayoutJob::simple_singleline("Search sessions, folders and commands".into(), FontId::proportional(12.0), pal.fg_dim);
        job.wrap = egui::text::TextWrapping::truncate_at_width((key_x - r.left() - 40.0).max(0.0));
        let words = ui.fonts_mut(|f| f.layout_job(job));
        if key_x - r.left() > 140.0 {
            p.galley(egui::pos2(r.left() + 28.0, r.center().y - 7.0), words, pal.fg_dim);
        }
        p.galley(egui::pos2(key_x, r.center().y - key.size().y / 2.0), key, GREY);
    }
    resp.clicked()
}

/// The search box opened (`Ctrl+Shift+P`): a field, and the entries that
/// match it below, the arrows moving among them, `Enter` picking, `Esc` or a
/// click elsewhere closing.
pub fn search_box(ctx: &egui::Context, pal: &Palette, view: &mut crate::palette::View, entries: &[crate::palette::Entry]) -> Option<crate::palette::Answer> {
    use crate::palette::Answer;
    let found = crate::palette::search(&view.query, entries);
    let (up, down, enter, esc) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if esc {
        return Some(Answer::Close);
    }
    let shown = found.len().min(12);
    if down && shown > 0 {
        view.selected = (view.selected + 1) % shown;
    }
    if up && shown > 0 {
        view.selected = (view.selected + shown - 1) % shown;
    }
    view.selected = view.selected.min(shown.saturating_sub(1));
    if enter {
        return found.get(view.selected).map(|e| Answer::Pick(e.pick.clone()));
    }
    let mut answer = None;
    let area = egui::Area::new(egui::Id::new("search-box")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 46.0)).show(ctx, |ui| {
        egui::Frame::NONE
            .fill(Color32::from_rgb(0x20, 0x23, 0x2b))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4b)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(520.0);
                let before = view.query.clone();
                let field = egui::TextEdit::singleline(&mut view.query)
                    .id(egui::Id::new("search-field"))
                    .hint_text("Search sessions, folders and commands")
                    .desired_width(f32::INFINITY);
                ui.add(field).request_focus();
                if view.query != before {
                    view.selected = 0;
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(RichText::new("Nothing matches").color(pal.fg_dim).size(12.0));
                }
                for (k, e) in found.iter().take(12).enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                        view.selected = k;
                    }
                    let p = ui.painter();
                    if k == view.selected {
                        p.rect_filled(rect, 5.0, Color32::from_rgb(0x2a, 0x2e, 0x37));
                    }
                    let title = p.layout_no_wrap(e.title.clone(), FontId::proportional(12.5), Color32::from_rgb(0xe4, 0xe8, 0xf0));
                    let title_w = title.size().x;
                    p.galley(egui::pos2(rect.left() + 8.0, rect.center().y - title.size().y / 2.0), title, Color32::WHITE);
                    let mut job = egui::text::LayoutJob::simple_singleline(e.detail.clone(), FontId::proportional(11.5), pal.fg_dim);
                    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - title_w - 28.0).max(0.0));
                    let detail = ui.fonts_mut(|f| f.layout_job(job));
                    p.galley(egui::pos2(rect.right() - 8.0 - detail.size().x, rect.center().y - detail.size().y / 2.0), detail, pal.fg_dim);
                    if resp.clicked() {
                        answer = Some(Answer::Pick(e.pick.clone()));
                    }
                }
            });
    });
    if answer.is_none() && area.response.clicked_elsewhere() && !view.opening {
        answer = Some(Answer::Close);
    }
    view.opening = false;
    answer
}

/// The sidebar's order button: two arrows and the order's short name.
pub fn sort_button(ui: &mut egui::Ui, pal: &Palette, label: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(11.5), pal.fg_dim);
    let size = egui::vec2(galley.size().x + 30.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let resp = resp.on_hover_text("Sort sessions");
    let p = ui.painter();
    let border = if resp.hovered() { Color32::from_rgb(0x4a, 0x50, 0x60) } else { Color32::from_rgb(0x2c, 0x30, 0x39) };
    p.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    // Down on the left, up on the right, as in the design.
    let stroke = egui::Stroke::new(1.3, pal.fg_dim);
    let o = rect.left_center() + egui::vec2(9.0, 0.0);
    p.line_segment([o + egui::vec2(0.0, -5.0), o + egui::vec2(0.0, 5.0)], stroke);
    p.add(egui::Shape::line(vec![o + egui::vec2(-2.5, 2.5), o + egui::vec2(0.0, 5.0), o + egui::vec2(2.5, 2.5)], stroke));
    let o = o + egui::vec2(7.0, 0.0);
    p.line_segment([o + egui::vec2(0.0, -5.0), o + egui::vec2(0.0, 5.0)], stroke);
    p.add(egui::Shape::line(vec![o + egui::vec2(-2.5, -2.5), o + egui::vec2(0.0, -5.0), o + egui::vec2(2.5, -2.5)], stroke));
    p.galley(egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, pal.fg_dim);
    resp
}

/// A filter's toggle at the sidebar's foot: outlined, and filled while on;
/// `dot`, a state's colour before the words.
pub fn filter_button(ui: &mut egui::Ui, pal: &Palette, label: &str, dot: Option<Color32>, on: bool) -> egui::Response {
    let color = if on { Color32::from_rgb(0xe4, 0xe8, 0xf0) } else { Color32::from_rgb(0x9a, 0xa3, 0xb5) };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(11.5), color);
    let pad = if dot.is_some() { 21.0 } else { 9.0 };
    let size = egui::vec2(galley.size().x + pad + 9.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, 6.0, Color32::from_rgb(0x2a, 0x2e, 0x37));
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, pal.selection.gamma_multiply(0.4));
    }
    let border = if on { Color32::from_rgb(0x4a, 0x50, 0x60) } else { Color32::from_rgb(0x2c, 0x30, 0x39) };
    p.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    if let Some(c) = dot {
        p.circle_filled(rect.left_center() + egui::vec2(12.0, 0.0), 3.0, c);
    }
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Six dots to take a row by (the design's grip), centred on `c`.
pub fn grip(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    for dx in [-2.0, 2.0] {
        for dy in [-4.0, 0.0, 4.0] {
            p.circle_filled(c + egui::vec2(dx, dy), 1.1, color);
        }
    }
}

/// A small bell struck through: the tab's notifications are muted.
pub fn muted_mark(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let stroke = egui::Stroke::new(1.2, color);
    let cup = vec![
        c + egui::vec2(-4.0, 3.0),
        c + egui::vec2(-3.3, -0.8),
        c + egui::vec2(-2.2, -3.3),
        c + egui::vec2(0.0, -4.0),
        c + egui::vec2(2.2, -3.3),
        c + egui::vec2(3.3, -0.8),
        c + egui::vec2(4.0, 3.0),
    ];
    p.add(egui::Shape::line(cup, stroke));
    p.line_segment([c + egui::vec2(-5.0, 3.0), c + egui::vec2(5.0, 3.0)], stroke);
    p.circle_filled(c + egui::vec2(0.0, 4.6), 1.0, color);
    p.line_segment([c + egui::vec2(-5.5, -5.0), c + egui::vec2(5.5, 6.0)], stroke);
}

/// What the notification list was asked to do.
pub enum BellAction {
    /// Go to the session, and count it read.
    Open(SessionId, u64),
    ReadAll,
    Close,
}

/// The list the bell opens (1h, the user's ask of 2026-10-06): newest first,
/// unread ones bright, each opening its session; "Mark all read" on top.
pub fn bell_list(ctx: &egui::Context, pal: &Palette, at: egui::Pos2, notices: &[Notice]) -> Option<BellAction> {
    let mut action = None;
    let now = now_ms();
    let area = egui::Area::new(egui::Id::new("bell-list")).order(egui::Order::Foreground).fixed_pos(at);
    let resp = area.show(ctx, |ui| {
        egui::Frame::NONE
            .fill(Color32::from_rgb(0x1b, 0x1e, 0x24))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4b)))
            .corner_radius(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    let unread = notices.iter().filter(|n| !n.read).count();
                    ui.label(RichText::new(format!("Notifications · {unread} unread")).strong().color(pal.fg));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(unread > 0, egui::Button::new("Mark all read").small()).clicked() {
                            action = Some(BellAction::ReadAll);
                        }
                    });
                });
                ui.separator();
                if notices.is_empty() {
                    ui.label(RichText::new("Nothing yet. A session that waits for you, fails, or finishes a long run shows here.").color(pal.fg_dim));
                    return;
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for n in notices.iter().rev() {
                        let text_color = if n.read { pal.fg_dim } else { pal.fg };
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 46.0), egui::Sense::click());
                        let p = ui.painter_at(rect);
                        if resp.hovered() {
                            p.rect_filled(rect, 6.0, pal.selection.gamma_multiply(0.6));
                        }
                        let dot = rect.left_top() + egui::vec2(10.0, 14.0);
                        if n.read {
                            p.circle_stroke(dot, 4.0, egui::Stroke::new(1.2, state_color(n.state)));
                        } else {
                            p.circle_filled(dot, 4.0, state_color(n.state));
                        }
                        let word = match n.state {
                            State::Waiting => "waiting",
                            State::Error => "error",
                            _ => "finished",
                        };
                        let head = format!("{} · {word}", n.title);
                        p.text(rect.left_top() + egui::vec2(22.0, 6.0), egui::Align2::LEFT_TOP, head, FontId::proportional(13.0), text_color);
                        p.text(rect.right_top() + egui::vec2(-6.0, 6.0), egui::Align2::RIGHT_TOP, format!("{} ago", elapsed(now.saturating_sub(n.at_ms))), FontId::proportional(11.0), pal.fg_dim);
                        if !n.note.is_empty() {
                            p.text(rect.left_top() + egui::vec2(22.0, 25.0), egui::Align2::LEFT_TOP, &n.note, FontId::proportional(11.5), pal.fg_dim);
                        }
                        if resp.clicked() {
                            action = Some(BellAction::Open(n.session, n.id));
                        }
                    }
                });
            });
    });
    if resp.response.clicked_elsewhere() && action.is_none() {
        action = Some(BellAction::Close);
    }
    action
}

/// The "Welcome back" screen's state: which saved panes are ticked.
pub struct RestoreView {
    pub saved: tsumugi_mux::state::Saved,
    pub ticked: Vec<(SessionId, bool)>,
    pub always: bool,
}

impl RestoreView {
    /// Everything ticked but what had finished (the design's 8: "finished ·
    /// left closed").
    pub fn new(saved: tsumugi_mux::state::Saved) -> Self {
        let ticked = saved.workspaces.iter().flat_map(|w| &w.panes).map(|p| (p.id, p.state != State::Done)).collect();
        Self { saved, ticked, always: false }
    }
}

pub enum RestoreAnswer {
    Restore(Vec<SessionId>),
    Fresh,
}

/// After a restart (the design's 8): the sessions that were open, the ones
/// that wanted a person first and ringed in gold, each to restore or not.
pub fn restore_screen(ui: &mut egui::Ui, pal: &Palette, view: &mut RestoreView) -> Option<RestoreAnswer> {
    let mut answer = None;
    let rect = ui.max_rect();
    let width = 620.0_f32.min(rect.width() - 40.0);
    let panes: Vec<&tsumugi_mux::state::SavedPane> = view.saved.workspaces.iter().flat_map(|w| &w.panes).collect();
    let mut order: Vec<usize> = (0..panes.len()).collect();
    // What wanted a person comes first: it is what to look at first.
    order.sort_by_key(|&i| match panes[i].state {
        State::Waiting | State::MaybeWaiting => 0,
        State::Error => 1,
        State::Running => 2,
        State::Done => 3,
    });
    let when = chrono::DateTime::from_timestamp_millis(view.saved.at_ms as i64)
        .map(|t| t.with_timezone(&chrono::Local).format("%Y/%m/%d %H:%M").to_string())
        .unwrap_or_default();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(rect.center(), egui::vec2(width, rect.height().min(560.0)))));
    egui::Frame::NONE.fill(Color32::from_rgb(0x1b, 0x1e, 0x24)).corner_radius(12.0).stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4b))).inner_margin(20.0).show(&mut child, |ui| {
        ui.label(RichText::new("Welcome back").size(18.0).strong().color(pal.fg));
        ui.label(RichText::new(format!("{} sessions were open when tsumugi stopped, {when}.", panes.len())).color(pal.fg_dim));
        ui.add_space(10.0);
        for &i in &order {
            let p = panes[i];
            let Some(slot) = view.ticked.iter_mut().find(|(id, _)| *id == p.id) else { continue };
            let waiting = matches!(p.state, State::Waiting | State::MaybeWaiting);
            let frame = if waiting { egui::Frame::NONE.fill(Color32::from_rgb(0x1f, 0x1d, 0x18)).stroke(egui::Stroke::new(1.0, GOLD.gamma_multiply(0.6))) } else { egui::Frame::NONE };
            frame.corner_radius(7.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut slot.1, "");
                    let color = if p.claude.is_some() || p.state != State::Done { state_color(p.state) } else { GREY };
                    ui.label(RichText::new("●").color(color));
                    let name = if p.title.is_empty() { crate::home_short(&p.cwd) } else { p.title.clone() };
                    ui.label(RichText::new(name).color(if slot.1 { pal.fg } else { pal.fg_dim }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let what = match (&p.claude, p.state) {
                            (Some(_), _) => "resume conversation".to_owned(),
                            (None, State::Done) if !slot.1 => "finished · left closed".to_owned(),
                            (None, _) => format!("new shell in {}", crate::home_short(&p.cwd)),
                        };
                        ui.label(RichText::new(what).size(12.0).color(pal.fg_dim));
                    });
                });
            });
        }
        ui.add_space(10.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut view.always, RichText::new("Always restore without asking").size(12.5).color(pal.fg_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let picked: Vec<SessionId> = view.ticked.iter().filter(|(_, on)| *on).map(|(id, _)| *id).collect();
                let restore = egui::Button::new(RichText::new(format!("Restore {}", picked.len())).strong().color(Color32::from_rgb(0x0f, 0x1d, 0x1d))).fill(CYAN);
                if ui.add_enabled(!picked.is_empty(), restore).clicked() {
                    answer = Some(RestoreAnswer::Restore(picked));
                }
                if ui.button("Start fresh").clicked() {
                    answer = Some(RestoreAnswer::Fresh);
                }
            });
        });
    });
    answer
}

#[cfg(test)]
mod tests {
    use super::elapsed;

    #[test]
    fn time_is_said_the_way_the_design_says_it() {
        assert_eq!(elapsed(14_000), "14s");
        assert_eq!(elapsed(252_000), "4m 12s");
        assert_eq!(elapsed(720_000), "12m");
        assert_eq!(elapsed(3_840_000), "1h 04m");
        assert_eq!(elapsed(2 * 86_400_000 + 3_600_000), "2d 1h");
    }
}
