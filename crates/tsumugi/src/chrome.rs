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

/// What the status bar was clicked for.
pub enum StatusClick {
    Bell,
}

/// The status bar along the bottom (1d): the server and how long it has been
/// up, how many sessions are in each state, the folder and branch of the pane
/// with the keys, its shell and size, and the clock (1n).
pub fn status_bar(ui: &mut egui::Ui, pal: &Palette, sessions: &[Info], focus: Option<&Info>, size: Option<(usize, usize)>, up_ms: u64) -> Option<StatusClick> {
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
            let branch = if i.branch.is_empty() { String::new() } else { format!(" · {}", i.branch) };
            ui.label(small(format!("{}{branch}", crate::home_short(&i.cwd)), pal.fg));
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
