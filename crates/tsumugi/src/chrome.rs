//! What the window shows around the panes: the status bar, the bell and its
//! list, and the "Welcome back" screen after a restart (the design's 1d, 1h
//! and 8). Each draws and hands back what was asked, and changes nothing.

use eframe::egui::{self, Color32, FontId, RichText};
use tsumugi_mux::{Info, Notice, SessionId, State};
use ito_pane::Palette;

use crate::i18n::{tr, trf};

/// The four state colours and the faintest text, from the theme in force
/// (1k): waiting gold, running cyan, error red, done green.
pub fn gold() -> Color32 {
    crate::theme::colors().wait
}
pub fn cyan() -> Color32 {
    crate::theme::colors().run
}
pub fn red() -> Color32 {
    crate::theme::colors().err
}
pub fn green() -> Color32 {
    crate::theme::colors().done
}
pub fn grey() -> Color32 {
    crate::theme::colors().faint()
}

/// The four state colours (docs/v1-scope.md 1k): waiting yellow, running
/// cyan, error red, done green -- filer's own yellow, cyan, red and green.
pub fn state_color(state: State) -> Color32 {
    match state {
        State::Waiting | State::MaybeWaiting => gold(),
        State::Running => cyan(),
        State::Error => red(),
        State::Done => green(),
    }
}

/// A state's colour as text: as it is where it reads, else moved toward
/// black on a light theme (white on a dark one) until it reaches WCAG's 4.5
/// against every ground it is written on. tsumugi Light's gold on white was
/// 3.6.
pub fn ink(c: Color32) -> Color32 {
    let t = crate::theme::colors();
    let grounds = [t.panel, t.side, t.wait_bg(), t.bg];
    let toward = if t.light { Color32::BLACK } else { Color32::WHITE };
    let mut out = c;
    for step in 1..=20 {
        if grounds.iter().all(|g| crate::theme::contrast(out, *g) >= 4.5) {
            break;
        }
        out = crate::theme::mix(c, toward, step as f32 * 0.05);
    }
    out
}

/// A state's colour, for its words.
pub fn state_ink(state: State) -> Color32 {
    ink(state_color(state))
}

/// How long a waiting ring takes to brighten and fade, in seconds.
pub const BREATHE: f64 = 2.4;
/// How long the running line takes to cross a card, in seconds.
pub const SWEEP: f64 = 1.8;

/// Where a waiting ring is in its breath at `time` seconds: 0 faint, 1
/// brightest, easing in and out.
pub fn breathe(time: f64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::TAU * time / BREATHE).cos()) as f32
}

/// The ring round something waiting for a person, breathing at `time`
/// (`None` holds it still at its brightest). A soft glow spreads outside it
/// as it brightens; on a light theme the line is thicker, the glow less.
pub fn wait_ring(painter: &egui::Painter, rect: egui::Rect, radius: f32, time: Option<f64>) {
    let light = crate::theme::colors().light;
    let b = time.map_or(1.0, breathe);
    let gold = gold();
    let width = if light { 1.5 } else { 1.0 };
    painter.rect_stroke(rect, radius, egui::Stroke::new(width, gold.gamma_multiply(0.45 + 0.5 * b)), egui::StrokeKind::Inside);
    if time.is_some() {
        let glow = if light { 0.18 } else { 0.28 };
        for (k, spread) in [1.5_f32, 3.5].into_iter().enumerate() {
            let alpha = glow * b / (k as f32 + 1.0);
            painter.rect_stroke(rect.expand(spread - 1.0), radius + spread, egui::Stroke::new(2.0, gold.gamma_multiply(alpha)), egui::StrokeKind::Outside);
        }
    }
}

/// Where the running line's left end is at `time`, as a share of the width:
/// from -0.3 (just out of sight on the left) to 1.0 (gone on the right).
pub fn sweep_at(time: f64) -> f32 {
    ((time % SWEEP) / SWEEP) as f32 * 1.3 - 0.3
}

/// The thin cyan line that runs along the top of something working, a
/// third of its width, left to right and round again.
pub fn run_line(painter: &egui::Painter, rect: egui::Rect, radius: f32, time: f64) {
    let w = rect.width();
    let x0 = rect.left() + sweep_at(time) * w;
    let left = x0.max(rect.left() + radius);
    let right = (x0 + 0.3 * w).min(rect.right() - radius);
    if right <= left {
        return;
    }
    let y = rect.top() + 1.0;
    painter.line_segment([egui::pos2(left, y), egui::pos2(right, y)], egui::Stroke::new(2.0, cyan()));
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

/// How a session is drawn (the design's States): by its state, or as a
/// plain shell -- no agent in it, at its prompt or running a command --
/// which has a grey ring and no mark or words, since "running" and "done"
/// say nothing about a shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    State(State),
    Shell,
}

pub fn look(info: &Info) -> Look {
    if crate::sort::is_shell(info) { Look::Shell } else { Look::State(info.state) }
}

pub fn look_color(l: Look) -> Color32 {
    match l {
        Look::State(s) => state_color(s),
        Look::Shell => grey(),
    }
}

/// The ring round a card or a pane, by how it looks: gold and breathing for
/// waiting, a thin gold one for probably waiting, cyan with the running
/// line, red for an error, green on a sunken ground for done, grey for a
/// shell. `time` moves what moves; `None` holds it still.
pub fn state_ring(painter: &egui::Painter, rect: egui::Rect, radius: f32, l: Look, time: Option<f64>) {
    let c = crate::theme::colors();
    let width = if c.light { 1.5 } else { 1.0 };
    let ring = |color: Color32| {
        painter.rect_stroke(rect, radius, egui::Stroke::new(width, color), egui::StrokeKind::Inside);
    };
    match l {
        Look::State(State::Waiting) => wait_ring(painter, rect, radius, time),
        Look::State(State::MaybeWaiting) => ring(gold().gamma_multiply(0.35)),
        Look::State(State::Running) => {
            ring(cyan().gamma_multiply(0.45));
            if let Some(t) = time {
                run_line(painter, rect, radius, t);
            }
        }
        Look::State(State::Error) => ring(red().gamma_multiply(0.6)),
        Look::State(State::Done) => ring(green().gamma_multiply(0.35)),
        Look::Shell => ring(grey().gamma_multiply(0.6)),
    }
}

/// The selected card's ring, over its state's: the state's colour (cyan
/// for a shell) at full strength, and a soft glow outside it. The ground
/// under it is left as it is.
pub fn selected_ring(painter: &egui::Painter, rect: egui::Rect, radius: f32, l: Look) {
    let color = match l {
        Look::Shell => cyan(),
        _ => look_color(l),
    };
    let light = crate::theme::colors().light;
    painter.rect_stroke(rect, radius, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
    let glow = if light { 0.16 } else { 0.24 };
    for (k, spread) in [1.5_f32, 3.5].into_iter().enumerate() {
        let alpha = glow / (k as f32 + 1.0);
        painter.rect_stroke(rect.expand(spread - 1.0), radius + spread, egui::Stroke::new(2.0, color.gamma_multiply(alpha)), egui::StrokeKind::Outside);
    }
    painter.rect_filled(rect, radius, color.gamma_multiply(0.04));
}

/// The zoom's mark at the right of the pane's heading, ending at `right`:
/// `ZOOM` and a small drawing of the split it hides; its rectangle.
pub fn zoom_mark(p: &egui::Painter, right: egui::Pos2, color: Color32) -> egui::Rect {
    let icon = egui::Rect::from_min_size(egui::pos2(right.x - 14.0, right.y - 5.0), egui::vec2(14.0, 10.0));
    let stroke = egui::Stroke::new(1.0, color);
    p.rect_stroke(icon, 1.5, stroke, egui::StrokeKind::Inside);
    p.vline(icon.center().x, icon.y_range(), stroke);
    p.hline(icon.center().x..=icon.right(), icon.center().y, stroke);
    let word = p.text(egui::pos2(icon.left() - 5.0, right.y), egui::Align2::RIGHT_CENTER, "ZOOM", FontId::proportional(10.0), color);
    word.union(icon)
}

/// A finished card's sunken ground, under its ring.
pub fn done_ground(painter: &egui::Painter, rect: egui::Rect, radius: f32) {
    let c = crate::theme::colors();
    painter.rect_filled(rect, radius, crate::theme::mix(c.panel, c.bg, 0.6));
}

/// The state's mark at `at`, about 10 points across, so the state reads
/// without its colour: a clock for waiting, a turning arc for running, a
/// dotted circle for probably waiting, a triangle for an error, a tick for
/// done, a small grey dot for a shell.
pub fn state_mark(painter: &egui::Painter, at: egui::Pos2, l: Look, time: Option<f64>) {
    let color = look_color(l);
    let stroke = egui::Stroke::new(1.4, color);
    match l {
        Look::State(State::Waiting) => {
            painter.circle_stroke(at, 4.5, stroke);
            painter.line_segment([at, at + egui::vec2(0.0, -3.0)], stroke);
            painter.line_segment([at, at + egui::vec2(2.2, 0.0)], stroke);
        }
        Look::State(State::Running) => {
            let start = time.map_or(0.0, |t| (t * std::f64::consts::TAU / 1.2) as f32);
            let pts: Vec<egui::Pos2> = (0..=18).map(|k| {
                let a = start + k as f32 / 18.0 * std::f32::consts::TAU * 0.72;
                at + egui::vec2(a.cos(), a.sin()) * 4.5
            }).collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        Look::State(State::MaybeWaiting) => {
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                painter.circle_filled(at + egui::vec2(a.cos(), a.sin()) * 4.5, 0.9, color);
            }
        }
        Look::State(State::Error) => {
            let pts = vec![at + egui::vec2(0.0, -5.0), at + egui::vec2(5.0, 4.0), at + egui::vec2(-5.0, 4.0)];
            painter.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
        }
        Look::State(State::Done) => {
            let pts = vec![at + egui::vec2(-4.0, 0.0), at + egui::vec2(-1.2, 3.0), at + egui::vec2(4.5, -3.5)];
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.8, color)));
        }
        Look::Shell => {
            painter.circle_filled(at, 2.5, color);
        }
    }
}

/// A pane's heading on the right: short, as the design has it (`Waiting ·
/// 2m`, `Running · 4m 12s`); nothing for a shell.
pub fn short_words(info: &Info, now: u64) -> String {
    let t = elapsed(now.saturating_sub(info.since_ms));
    match look(info) {
        Look::Shell => String::new(),
        Look::State(State::Waiting) => trf("state.waiting", &[&t]),
        Look::State(State::MaybeWaiting) => trf("state.quiet", &[&t]),
        Look::State(State::Running) => trf("state.running", &[&t]),
        Look::State(State::Error) => trf("state.error", &[&t]),
        Look::State(State::Done) => trf("state.done", &[&t]),
    }
}

/// The third line of a sidebar row: what the session is doing, in words,
/// and for how long (the design's sidebar, 3). A shell has none: its
/// folder and branch say it.
pub fn card_words(info: &Info, now: u64) -> String {
    if look(info) == Look::Shell {
        return String::new();
    }
    state_words(info, now)
}

/// What the session is doing, in words, and for how long.
pub fn state_words(info: &Info, now: u64) -> String {
    let t = elapsed(now.saturating_sub(info.since_ms));
    match info.state {
        State::Waiting => trf("state.waiting_long", &[&t]),
        State::MaybeWaiting => trf("state.quiet_long", &[&t]),
        State::Running => trf("state.running", &[&t]),
        State::Error => trf("state.error", &[&t]),
        State::Done => trf("state.done", &[&t]),
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
    /// The pull request's page.
    Open(String),
    /// Read and write the focused pane in this character set.
    Charset(String),
}

/// The status bar along the bottom (1d): the server and how long it has been
/// up, how many sessions are in each state, the folder and branch of the pane
/// with the keys, its shell and size, and the clock (1n).
///
/// `extra`: the server's uptime, whether a Nerd Font is there, and the
/// clock's `chrono` format (`None`: no clock).
pub fn status_bar(
    ui: &mut egui::Ui,
    pal: &Palette,
    sessions: &[Info],
    focus: Option<&Info>,
    size: Option<(usize, usize)>,
    extra: &StatusExtra,
) -> Option<StatusClick> {
    let StatusExtra { up_ms, nerd, clock, git, tokens, block, cost } = extra;
    let (up_ms, nerd) = (*up_ms, *nerd);
    let mut click = None;
    let small = |t: String, c: Color32| RichText::new(t).font(FontId::proportional(11.5)).color(c);
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        ui.label(small(trf("status.up", &[&elapsed(up_ms)]), pal.fg_dim));
        ui.add_space(14.0);
        // Shells are none of these: they are not running anything of note.
        let count = |s: State| sessions.iter().filter(|i| !crate::sort::is_shell(i) && (i.state == s || (s == State::Waiting && i.state == State::MaybeWaiting))).count();
        for state in [State::Waiting, State::Running, State::Error] {
            let n = count(state);
            if n == 0 {
                continue;
            }
            let n = n.to_string();
            let words = match state {
                State::Waiting => trf("status.waiting", &[&n]),
                State::Running => trf("status.running", &[&n]),
                _ => trf("status.error", &[&n]),
            };
            let r = ui.add(egui::Label::new(small(words, state_ink(state))).sense(egui::Sense::click()));
            if r.on_hover_text(tr("status.bell_hover")).clicked() {
                click = Some(StatusClick::Bell);
            }
            ui.add_space(6.0);
        }
        // The design's two lines: after the counts, and before the right.
        let divider = |ui: &mut egui::Ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(1.0, 14.0), egui::Sense::hover());
            ui.painter().vline(r.center().x, r.y_range(), egui::Stroke::new(1.0, crate::theme::colors().border_strong()));
        };
        if let Some(i) = focus {
            ui.add_space(4.0);
            divider(ui);
            ui.add_space(10.0);
            ui.label(RichText::new(crate::home_short(&i.cwd)).font(FontId::monospace(11.0)).color(pal.fg));
            if !i.branch.is_empty() {
                ui.add_space(4.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                branch_mark(ui.painter(), r, pal.fg_dim, nerd);
                ui.label(small(i.branch.clone(), pal.fg));
                if let Some(g) = git {
                    git_marks(ui, pal, g, &mut click);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            if let Some(format) = clock {
                // The design's: in the terminal's font and the strongest
                // colour; the whole date on the pointer.
                let now = chrono::Local::now();
                ui.label(RichText::new(now.format(format).to_string()).font(FontId::monospace(11.5)).color(crate::theme::colors().strong())).on_hover_text(now.format("%A, %-d %B %Y").to_string());
                ui.add_space(12.0);
            }
            // The pane's character set; a menu of others off Windows, where
            // ConPTY hands over UTF-8 whatever the program wrote.
            let charset = focus.map_or("UTF-8", |i| if i.charset.is_empty() { "UTF-8" } else { i.charset.as_str() });
            if cfg!(windows) || focus.is_none() {
                ui.label(small(charset.into(), pal.fg_dim)).on_hover_text(tr("charset.windows_hint"));
            } else {
                let color = if charset == "UTF-8" { pal.fg_dim } else { ink(gold()) };
                ui.menu_button(small(charset.into(), color), |ui| {
                    ui.label(RichText::new(tr("charset.title")).size(11.5).color(pal.fg_dim));
                    for name in ito_pane::CHARSETS {
                        if ui.selectable_label(name == charset, name).clicked() {
                            click = Some(StatusClick::Charset(name.to_owned()));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tr("charset.hint"));
            }
            if let Some((conversation, today)) = tokens {
                use crate::usage::short;
                ui.add_space(12.0);
                use crate::price::dollars;
                let money = |c: f64| if c > 0.0 { format!(" ≈{}", dollars(c)) } else { String::new() };
                let said = match conversation {
                    Some(c) => trf("status.tokens", &[&short(c.total()), &money(cost.0.unwrap_or(0.0)), &short(today.total()), &money(cost.1)]),
                    None => trf("status.tokens_today", &[&short(today.total()), &money(cost.1)]),
                };
                let detail = |t: &crate::usage::Tokens| trf("status.tokens_detail", &[&short(t.input), &short(t.cache_write), &short(t.cache_read), &short(t.output)]);
                let mut hover = trf("status.tokens_hover_today", &[&detail(today)]);
                if let Some(c) = conversation {
                    hover = trf("status.tokens_hover_conversation", &[&detail(c), &hover]);
                }
                ui.label(small(said, pal.fg_dim)).on_hover_text(format!("{hover}\n{}", tr("status.tokens_note")));
            }
            if let Some(b) = block {
                use crate::usage::short;
                use chrono::TimeZone;
                let end = chrono::Local.timestamp_millis_opt(b.end_ms).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default();
                let start = chrono::Local.timestamp_millis_opt(b.start_ms).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default();
                let left = (b.end_ms - now_ms() as i64).max(0) as u64;
                ui.add_space(12.0);
                let money = if b.cost > 0.0 { format!(" ≈{}", crate::price::dollars(b.cost)) } else { String::new() };
                let tokens = short(b.tokens.total());
                ui.label(small(trf("status.block", &[&tokens, &money, &end]), pal.fg_dim)).on_hover_text(trf("status.block_hover", &[&start, &end, &elapsed(left), &tokens]));
            }
            if let (Some(i), Some((cols, lines))) = (focus, size) {
                ui.add_space(12.0);
                ui.label(small(format!("{} · {cols}×{lines}", crate::program_name(&i.command)), pal.fg_dim)).on_hover_text(tr("status.program_hover"));
            }
            ui.add_space(10.0);
            divider(ui);
        });
    });
    click
}

/// After the branch: the commits not pushed (and not pulled), then the pull
/// request, coloured by its checks, a click away from its page.
fn git_marks(ui: &mut egui::Ui, pal: &Palette, g: &crate::gitinfo::Git, click: &mut Option<StatusClick>) {
    use crate::gitinfo::Checks;
    let small = |t: String, c: Color32| RichText::new(t).font(FontId::proportional(11.5)).color(c);
    if let Some((ahead, behind)) = g.ahead {
        if ahead > 0 {
            ui.label(small(format!("↑{ahead}"), gold())).on_hover_text(format!("{ahead} commit(s) not pushed"));
        }
        if behind > 0 {
            ui.label(small(format!("↓{behind}"), pal.fg_dim)).on_hover_text(format!("{behind} commit(s) to pull"));
        }
    }
    if let Some(pr) = &g.pr {
        ui.add_space(8.0);
        let (mark, color, words) = match pr.checks {
            Checks::Pass => ("✓", green(), "checks passed"),
            Checks::Fail => ("✗", red(), "checks failed"),
            Checks::Running => ("●", cyan(), "checks running"),
            Checks::None => ("", pal.fg_dim, "no checks"),
        };
        let state = match pr.state.as_str() {
            "MERGED" => " merged",
            "CLOSED" => " closed",
            _ => "",
        };
        let text = format!("PR #{}{state} {mark}", pr.number);
        let r = ui.add(egui::Label::new(small(text.trim_end().to_owned(), color)).sense(egui::Sense::click()));
        if r.on_hover_text(format!("{words} · click to open {}", pr.url)).clicked() {
            *click = Some(StatusClick::Open(pr.url.clone()));
        }
    }
}

/// A card's pull request line (the design's Sidebar, 4): `PR #273 ·
/// checks passing`, coloured by its checks.
pub fn pr_line(pr: &crate::gitinfo::Pr) -> (String, Color32) {
    use crate::gitinfo::Checks;
    let (words, color) = match pr.checks {
        Checks::Pass => ("checks passing", green()),
        Checks::Fail => ("checks failing", red()),
        Checks::Running => ("checks running", cyan()),
        Checks::None => ("no checks", grey()),
    };
    let state = match pr.state.as_str() {
        "MERGED" => " merged",
        "CLOSED" => " closed",
        _ => "",
    };
    (format!("PR #{}{state} · {words}", pr.number), ink(color))
}

/// A session's last lines, as a card's preview shows them: small, in the
/// terminal's font, cut at the width rather than wrapped.
pub fn peek(ui: &mut egui::Ui, pal: &Palette, lines: &[String]) {
    if lines.is_empty() {
        ui.label(RichText::new(tr("sidebar.nothing_yet")).size(11.5).color(pal.fg_dim));
        return;
    }
    let font = FontId::monospace(11.0);
    let width = 520.0;
    let shown: Vec<_> = lines
        .iter()
        .map(|l| {
            let mut job = egui::text::LayoutJob::simple_singleline(l.clone(), font.clone(), pal.fg);
            job.wrap = egui::text::TextWrapping::truncate_at_width(width);
            ui.fonts_mut(|f| f.layout_job(job))
        })
        .collect();
    let h: f32 = shown.iter().map(|g| g.size().y).sum();
    let w = shown.iter().map(|g| g.size().x).fold(0.0f32, f32::max);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w + 16.0, h + 12.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 6.0, pal.bg);
    let mut y = rect.top() + 6.0;
    for g in shown {
        let gh = g.size().y;
        p.galley(egui::pos2(rect.left() + 8.0, y), g, pal.fg);
        y += gh;
    }
}

/// The bell: a button with the number of unread notifications on it, gold,
/// or red while one of them is an error.
/// The "+" that opens the new-session dialog (the same as `Ctrl+Shift+T`).
pub fn plus_button(ui: &mut egui::Ui, pal: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 24.0), egui::Sense::click());
    let resp = resp.on_hover_text(trf("sidebar.new_session", &[&crate::keys::label(crate::keys::Action::NewTab)]));
    let p = ui.painter();
    let border = if resp.hovered() { crate::theme::colors().border_strong() } else { crate::theme::colors().border };
    if resp.hovered() {
        p.rect_filled(rect, 6.0, pal.selection.gamma_multiply(0.5));
    }
    p.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5, if resp.hovered() { pal.fg } else { pal.fg_dim });
    p.line_segment([c - egui::vec2(5.0, 0.0), c + egui::vec2(5.0, 0.0)], stroke);
    p.line_segment([c - egui::vec2(0.0, 5.0), c + egui::vec2(0.0, 5.0)], stroke);
    resp
}

/// The bell in `rect` (28 by 24), with the unread count on it.
pub fn bell(ui: &mut egui::Ui, rect: egui::Rect, pal: &Palette, notices: &[Notice]) -> egui::Response {
    let unread: Vec<&Notice> = notices.iter().filter(|n| !n.read).collect();
    let resp = ui.interact(rect, ui.id().with("bell"), egui::Sense::click());
    let resp = resp.on_hover_text(trf("sidebar.notices", &[&unread.len().to_string(), &crate::keys::label(crate::keys::Action::Notices)]));
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
        let color = if unread.iter().any(|n| n.state == State::Error) { red() } else { gold() };
        let text = if unread.len() > 9 { "9+".to_owned() } else { unread.len().to_string() };
        let at = rect.right_top() + egui::vec2(-5.0, 5.0);
        p.circle_filled(at, 7.0, color);
        p.text(at, egui::Align2::CENTER_CENTER, text, FontId::proportional(10.0), crate::theme::colors().on_accent());
    }
    resp
}

/// `[tags.colors]`: tags given a colour of their own.
static TAG_COLORS: std::sync::RwLock<std::collections::BTreeMap<String, Color32>> = std::sync::RwLock::new(std::collections::BTreeMap::new());

/// Take `[tags.colors]` (`"#rrggbb"` each; others are left out).
pub fn set_tag_colors(map: &std::collections::BTreeMap<String, String>) {
    let parsed = map
        .iter()
        .filter_map(|(k, v)| {
            let h = v.trim().strip_prefix('#')?;
            let n = u32::from_str_radix(h.get(..6)?, 16).ok()?;
            Some((k.clone(), Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8)))
        })
        .collect();
    if let Ok(mut m) = TAG_COLORS.write() {
        *m = parsed;
    }
}

/// The design's eight tag colours, as `"#rrggbb"`, for the settings' picker.
pub const TAG_PALETTE: [&str; 8] = ["#4a4f5c", "#5e4a86", "#2f6b66", "#7a5a22", "#2f4f7f", "#7a343a", "#3d6b3a", "#7a3a5e"];

/// A tag's colours, fill and text: picked from its name, so a tag is the
/// same colour in every window and after a restart. The first three are the
/// design's (`claude`, `review`, `filer`).
pub fn tag_colors(tag: &str) -> (Color32, Color32) {
    // A colour of its own from `[tags.colors]`, its text white or black.
    if let Some(fill) = TAG_COLORS.read().ok().and_then(|m| m.get(tag).copied()) {
        let light = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b()) > 150.0;
        return (fill, if light { Color32::from_rgb(0x14, 0x14, 0x13) } else { Color32::WHITE });
    }
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
    p.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, crate::theme::colors().border_strong()), egui::StrokeKind::Inside);
    p.galley(egui::pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0), galley, color);
    rect
}

/// How many of the tags (their chips' widths) to show in `room`, at most
/// `shown`, with `+N` for the rest; and the width they take, gaps and all.
/// When not even `+N` fits, none.
pub fn tags_that_fit(widths: &[f32], shown: usize, room: f32, more_w: impl Fn(usize) -> f32) -> (usize, f32) {
    let total = widths.len();
    for n in (0..=shown.min(total)).rev() {
        let chips: f32 = widths[..n].iter().map(|w| w + 5.0).sum();
        let used = if n < total { chips + more_w(total - n) } else { chips - 5.0 };
        if used <= room {
            return (n, used.max(0.0));
        }
    }
    (0, 0.0)
}

/// The band along the top of the window (the design's 1c): the name, the
/// search box and the bell in the middle of the window, and the tags of the
/// session with the keys on the right, `[tags] shown` and `+N`. When the
/// window is narrow the tags give way to `+N` first, then the box shrinks
/// to its magnifier (1o); the box never moves for them.
pub fn top_band(ui: &mut egui::Ui, pal: &Palette, tags: &[String], shown: usize, muted_tags: &[String], notices: &[Notice], frame: &BandFrame) -> BandOut {
    let whole = ui.max_rect();
    let p = ui.painter().clone();
    p.line_segment([whole.left_bottom(), whole.right_bottom()], egui::Stroke::new(1.0, crate::theme::colors().border));
    let mut out = BandOut::default();
    // As the title bar: the band's empty parts move the window and a double
    // click maximizes it. Taken first, so what is drawn on it comes first.
    if frame.own {
        // The band is the mouse's: none of it is a stop for Tab, which goes
        // to what the window shows under it.
        let bg = ui.interact(whole, ui.id().with("band-drag"), egui::Sense::CLICK | egui::Sense::DRAG);
        if bg.double_clicked() {
            out.window = Some(WindowOp::ToggleMax);
        } else if bg.drag_started() {
            out.window = Some(WindowOp::Drag);
        }
    }
    let buttons = if frame.own && !cfg!(target_os = "macos") { 3.0 * CAPTION_W } else { 0.0 };
    if buttons > 0.0 {
        if let Some(op) = caption_buttons(ui, egui::Rect::from_min_max(egui::pos2(whole.right() - buttons, whole.top()), whole.right_bottom()), pal, frame.maximized) {
            out.window = Some(op);
        }
    }
    let mut rect = whole;
    rect.min.x += frame.left;
    rect.max.x -= buttons;
    // The mark: two threads, cyan and gold (the design's 10 A).
    let o = egui::pos2(rect.left() + 16.0, rect.center().y);
    let wave = |amp: f32, color: Color32| {
        let pts: Vec<egui::Pos2> = (0..=16).map(|k| {
            let x = k as f32;
            o + egui::vec2(x, -amp * ((x / 16.0) * std::f32::consts::TAU).cos())
        }).collect();
        p.add(egui::Shape::line(pts, egui::Stroke::new(1.6, color)));
    };
    wave(4.0, cyan());
    wave(2.0, gold());
    let name = p.layout_no_wrap("tsumugi".into(), FontId::proportional(13.0), crate::theme::colors().strong());
    let name_right = o.x + 24.0 + name.size().x;
    p.galley(egui::pos2(o.x + 24.0, rect.center().y - name.size().y / 2.0), name, Color32::WHITE);
    if frame.settings {
        p.text(whole.center(), egui::Align2::CENTER_CENTER, tr("title.settings"), FontId::proportional(13.0), pal.fg_dim);
        let r = egui::Rect::from_min_max(egui::pos2(rect.right() - 44.0, whole.top() + 4.0), egui::pos2(rect.right(), whole.bottom() - 4.0));
        let resp = ui.interact(r, ui.id().with("close-settings"), egui::Sense::CLICK).on_hover_text(tr("title.close_settings"));
        if resp.hovered() {
            p.rect_filled(r, 4.0, crate::theme::colors().hover());
        }
        let c = r.center();
        let stroke = egui::Stroke::new(1.2, if resp.hovered() { crate::theme::colors().strong() } else { pal.fg_dim });
        p.line_segment([c + egui::vec2(-4.5, -4.5), c + egui::vec2(4.5, 4.5)], stroke);
        p.line_segment([c + egui::vec2(4.5, -4.5), c + egui::vec2(-4.5, 4.5)], stroke);
        out.close_settings = resp.clicked();
        return out;
    }

    // The box and the bell beside it, in the middle of the window whatever
    // the tags (the user's ask of 2026-10-09): as wide as there is room for
    // on both sides of the middle, then only the magnifier.
    const BELL: f32 = 34.0;
    let middle = whole.center().x;
    let half = (middle - name_right - 12.0).min(rect.right() - 12.0 - middle);
    let box_w = 420.0f32.min(2.0 * half - BELL);
    let compact = box_w < 160.0;
    let w = if compact { 30.0 } else { box_w };
    let group_left = (middle - (w + BELL) / 2.0).max(name_right + 12.0);
    let r = egui::Rect::from_min_size(egui::pos2(group_left, rect.center().y - 13.0), egui::vec2(w, 26.0));
    // The tags at the right, in what the box and the bell leave: up to
    // `[tags] shown`, then +N; fewer, and +N, when they do not fit.
    let room = (rect.right() - 12.0) - (r.right() + BELL + 12.0);
    let chip_w = |t: &str| p.layout_no_wrap(t.to_owned(), FontId::proportional(11.0), pal.fg).size().x + 12.0;
    let more_w = |n: usize| p.layout_no_wrap(format!("+{n}"), FontId::proportional(11.0), pal.fg).size().x + 12.0;
    let (n, used) = tags_that_fit(&tags.iter().map(|t| chip_w(t)).collect::<Vec<_>>(), shown, room, more_w);
    if used > 0.0 {
        let mut x = rect.right() - 12.0 - used;
        for t in &tags[..n] {
            x = tag_chip(&p, egui::pos2(x, rect.center().y - 8.0), t, muted_tags.contains(t)).right() + 5.0;
        }
        if n < tags.len() {
            let r = more_chip(&p, egui::pos2(x, rect.center().y - 8.0), tags.len() - n, pal.fg_dim);
            ui.interact(r, ui.id().with("more-tags"), egui::Sense::hover()).on_hover_text(tags[n..].join(", "));
        }
    }

    let resp = ui.interact(r, ui.id().with("search"), egui::Sense::CLICK).on_hover_text(tr("title.search"));
    let fill = if resp.hovered() { crate::theme::colors().hover() } else { crate::theme::colors().panel };
    p.rect_filled(r, 6.0, fill);
    p.rect_stroke(r, 6.0, egui::Stroke::new(1.0, crate::theme::colors().border), egui::StrokeKind::Inside);
    let glass = egui::pos2(r.left() + 15.0, r.center().y - 1.0);
    let stroke = egui::Stroke::new(1.3, pal.fg_dim);
    p.circle_stroke(glass, 4.0, stroke);
    p.line_segment([glass + egui::vec2(3.0, 3.0), glass + egui::vec2(6.0, 6.0)], stroke);
    if !compact {
        let key = p.layout_no_wrap(crate::keys::label(crate::keys::Action::Search), FontId::monospace(11.0), grey());
        let key_x = r.right() - 10.0 - key.size().x;
        let mut job = egui::text::LayoutJob::simple_singleline(tr("title.search").into(), FontId::proportional(12.0), pal.fg_dim);
        job.wrap = egui::text::TextWrapping::truncate_at_width((key_x - r.left() - 40.0).max(0.0));
        let words = ui.fonts_mut(|f| f.layout_job(job));
        if key_x - r.left() > 140.0 {
            p.galley(egui::pos2(r.left() + 28.0, r.center().y - 7.0), words, pal.fg_dim);
        }
        p.galley(egui::pos2(key_x, r.center().y - key.size().y / 2.0), key, grey());
    }
    out.search = resp.clicked();
    // The bell just right of the box (the notices, the box's neighbour).
    let bell_rect = egui::Rect::from_min_size(egui::pos2(r.right() + 6.0, rect.center().y - 12.0), egui::vec2(28.0, 24.0));
    out.bell = bell(ui, bell_rect, pal, notices).clicked();
    out.bell_anchor = Some(egui::pos2(bell_rect.right() - 380.0, bell_rect.bottom() + 6.0));
    out
}

/// How the band is to be a title bar.
pub struct BandFrame {
    /// The window has no system title bar: the band is it.
    pub own: bool,
    /// Room kept on the left (macOS's traffic lights over the band).
    pub left: f32,
    pub maximized: bool,
    /// The settings are open: the band says so in the middle, with their
    /// close button, in place of the search box and the tags.
    pub settings: bool,
}

#[derive(Default)]
pub struct BandOut {
    /// The search box was clicked.
    pub search: bool,
    /// The settings' close button was clicked.
    pub close_settings: bool,
    /// The bell was clicked.
    pub bell: bool,
    /// Where its list goes, under it.
    pub bell_anchor: Option<egui::Pos2>,
    pub window: Option<WindowOp>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowOp {
    Drag,
    ToggleMax,
    Minimize,
    Close,
}

/// A caption button's width, as Windows 11 has them.
const CAPTION_W: f32 = 46.0;

/// Minimize, maximize (restore when maximized) and close, in `rect`, drawn
/// as Windows draws them: thin lines, close red under the pointer.
fn caption_buttons(ui: &mut egui::Ui, rect: egui::Rect, pal: &Palette, maximized: bool) -> Option<WindowOp> {
    let mut out = None;
    let ops = [(WindowOp::Minimize, tr("title.minimize")), (WindowOp::ToggleMax, if maximized { tr("title.restore") } else { tr("title.maximize") }), (WindowOp::Close, tr("title.close"))];
    for (k, (op, words)) in ops.into_iter().enumerate() {
        let r = egui::Rect::from_min_size(rect.min + egui::vec2(k as f32 * CAPTION_W, 0.0), egui::vec2(CAPTION_W, rect.height()));
        let resp = ui.interact(r, ui.id().with(("caption", k)), egui::Sense::CLICK).on_hover_text(words);
        let p = ui.painter();
        let hot = resp.hovered();
        let close = op == WindowOp::Close;
        if hot {
            p.rect_filled(r, 0.0, if close { Color32::from_rgb(0xc4, 0x2b, 0x1c) } else { crate::theme::colors().hover() });
        }
        let color = if hot && close { Color32::WHITE } else { pal.fg };
        let stroke = egui::Stroke::new(1.0, color);
        let c = r.center();
        match op {
            WindowOp::Minimize => {
                p.line_segment([c + egui::vec2(-5.0, 0.0), c + egui::vec2(5.0, 0.0)], stroke);
            }
            WindowOp::ToggleMax if maximized => {
                // Two windows, one behind the other.
                p.rect_stroke(egui::Rect::from_center_size(c + egui::vec2(-1.0, 1.0), egui::vec2(8.0, 8.0)), 1.0, stroke, egui::StrokeKind::Inside);
                p.line_segment([c + egui::vec2(-3.0, -5.0), c + egui::vec2(5.0, -5.0)], stroke);
                p.line_segment([c + egui::vec2(5.0, -5.0), c + egui::vec2(5.0, 3.0)], stroke);
            }
            WindowOp::ToggleMax => {
                p.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)), 1.0, stroke, egui::StrokeKind::Inside);
            }
            WindowOp::Close | WindowOp::Drag => {
                p.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], stroke);
                p.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], stroke);
            }
        }
        if resp.clicked() {
            out = Some(op);
        }
    }
    out
}

/// A window without the system's frame is resized by its edges: a 6px
/// strip along each, and the corners, set the pointer and start the
/// system's resizing on a press (on the press, not once a drag is seen: by
/// then the pointer has left the window, outward). Called last in a frame,
/// so its pointer is the one shown. Not while maximized.
pub fn resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon as C, ResizeDirection as D};
    let r = ctx.content_rect();
    let (pos, pressed) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_pressed()));
    let Some(p) = pos else { return };
    let Some(dir) = edge_at(r, p) else { return };
    ctx.set_cursor_icon(match dir {
        D::North | D::South => C::ResizeVertical,
        D::East | D::West => C::ResizeHorizontal,
        D::NorthWest | D::SouthEast => C::ResizeNwSe,
        D::NorthEast | D::SouthWest => C::ResizeNeSw,
    });
    if pressed {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
    }
}

/// Which edge or corner of `r` the point `p` is on, if any. The top edge is
/// thinner: the band under it is the title bar to drag by.
fn edge_at(r: egui::Rect, p: egui::Pos2) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection as D;
    const EDGE: f32 = 6.0;
    const CORNER: f32 = 12.0;
    if !r.contains(p) {
        return None;
    }
    let (left, right) = (p.x - r.left() < EDGE, r.right() - p.x < EDGE);
    let (top, bottom) = (p.y - r.top() < EDGE / 2.0, r.bottom() - p.y < EDGE);
    let (near_left, near_right) = (p.x - r.left() < CORNER, r.right() - p.x < CORNER);
    let (near_top, near_bottom) = (p.y - r.top() < CORNER, r.bottom() - p.y < CORNER);
    let dir = match () {
        _ if (top && near_left) || (left && near_top) => D::NorthWest,
        _ if (top && near_right) || (right && near_top) => D::NorthEast,
        _ if (bottom && near_left) || (left && near_bottom) => D::SouthWest,
        _ if (bottom && near_right) || (right && near_bottom) => D::SouthEast,
        _ if top => D::North,
        _ if bottom => D::South,
        _ if left => D::West,
        _ if right => D::East,
        _ => return None,
    };
    Some(dir)
}

/// The command palette opened (`Ctrl+Shift+P`): a field, and the entries
/// that match it below, the arrows moving among them, `Enter` picking, `Esc`
/// or a click elsewhere closing.
pub fn search_box(ctx: &egui::Context, pal: &Palette, view: &mut crate::palette::View, entries: &[crate::palette::Entry]) -> Option<crate::palette::Answer> {
    use crate::palette::Answer;
    let found: Vec<crate::palette::Entry> = crate::palette::search(&view.query, entries).into_iter().take(12).collect();
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
    let shown = found.len();
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
            .fill(crate::theme::colors().raised())
            .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(520.0);
                let before = view.query.clone();
                let field = egui::TextEdit::singleline(&mut view.query)
                    .id(egui::Id::new("search-field"))
                    .hint_text(trf("title.search_hint", &[&crate::keys::label(crate::keys::Action::Overview)]))
                    .desired_width(f32::INFINITY);
                crate::keep_focus(&ui.add(field));
                if view.query != before {
                    view.selected = 0;
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(RichText::new(tr("title.nothing_matches")).color(pal.fg_dim).size(12.0));
                }
                for (k, e) in found.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                        view.selected = k;
                    }
                    let p = ui.painter();
                    if k == view.selected {
                        p.rect_filled(rect, 5.0, crate::theme::colors().chosen());
                    }
                    // The detail at most 40% of the line, the title the rest;
                    // both cut short, so nothing runs out of the box.
                    let mut job = egui::text::LayoutJob::simple_singleline(e.detail.clone(), FontId::proportional(11.5), pal.fg_dim);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() * 0.4);
                    let detail = ui.fonts_mut(|f| f.layout_job(job));
                    let mut job = egui::text::LayoutJob::simple_singleline(e.title.clone(), FontId::proportional(12.5), crate::theme::colors().strong());
                    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - detail.size().x - 28.0).max(20.0));
                    let title = ui.fonts_mut(|f| f.layout_job(job));
                    let p = ui.painter();
                    p.galley(egui::pos2(rect.left() + 8.0, rect.center().y - title.size().y / 2.0), title, Color32::WHITE);
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
    let resp = resp.on_hover_text(crate::i18n::tr("sort.hover"));
    let p = ui.painter();
    let border = if resp.hovered() { crate::theme::colors().border_strong() } else { crate::theme::colors().border };
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
    let color = if on { crate::theme::colors().strong() } else { crate::theme::colors().dim };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(11.5), color);
    let pad = if dot.is_some() { 21.0 } else { 9.0 };
    let size = egui::vec2(galley.size().x + pad + 9.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, 6.0, crate::theme::colors().chosen());
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, pal.selection.gamma_multiply(0.4));
    }
    let border = if on { crate::theme::colors().border_strong() } else { crate::theme::colors().border };
    p.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    if let Some(c) = dot {
        p.circle_filled(rect.left_center() + egui::vec2(12.0, 0.0), 3.0, c);
    }
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// A group's triangle: pointing down when open, right when closed.
pub fn triangle(p: &egui::Painter, c: egui::Pos2, open: bool, color: Color32) {
    let pts = if open {
        vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)]
    } else {
        vec![c + egui::vec2(-2.0, -4.0), c + egui::vec2(3.0, 0.0), c + egui::vec2(-2.0, 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
}

/// A pin: the tab is kept at the top.
pub fn pin_mark(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let stroke = egui::Stroke::new(1.3, color);
    p.circle_filled(c + egui::vec2(1.5, -2.5), 3.0, color);
    p.line_segment([c + egui::vec2(0.5, -1.5), c + egui::vec2(-4.0, 4.5)], stroke);
}

/// A dashed rectangle around `rect`: a tag the folder's rule gives.
pub fn dashed_outline(p: &egui::Painter, rect: egui::Rect, color: Color32) {
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    p.extend(egui::Shape::dashed_line(&corners, egui::Stroke::new(1.0, color), 3.0, 2.0));
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

/// The status bar's other inputs.
pub struct StatusExtra {
    pub up_ms: u64,
    pub nerd: bool,
    pub clock: Option<String>,
    pub git: Option<crate::gitinfo::Git>,
    /// Claude Code's tokens: the focused session's conversation, if it is
    /// one, and today's in all.
    pub tokens: Option<(Option<crate::usage::Tokens>, crate::usage::Tokens)>,
    /// Claude Code's five-hour window, when one is open.
    pub block: Option<crate::usage::Block>,
    /// What the conversation's and today's tokens would cost on the API.
    pub cost: (Option<f64>, f64),
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
            .fill(crate::theme::colors().panel)
            .stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong()))
            .corner_radius(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    let unread = notices.iter().filter(|n| !n.read).count();
                    ui.label(RichText::new(trf("notices.title", &[&unread.to_string()])).strong().color(pal.fg));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(unread > 0, egui::Button::new(tr("notices.mark_read")).small()).clicked() {
                            action = Some(BellAction::ReadAll);
                        }
                    });
                });
                ui.separator();
                if notices.is_empty() {
                    ui.label(RichText::new(tr("notices.empty")).color(pal.fg_dim));
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
                            _ if n.trigger => "trigger",
                            State::Waiting => "waiting",
                            State::Error => "error",
                            _ => "finished",
                        };
                        let head = format!("{} · {word}", crate::sort::without_spinner(&n.title));
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

/// The "Select all" box: every row ticked, else ticks them all; all
/// ticked, it clears them. Finished ones too: ticked, they open a new shell
/// in their folder.
fn tick_all(ticked: &mut [(SessionId, bool)]) {
    let on = !ticked.iter().all(|(_, on)| *on);
    for (_, t) in ticked {
        *t = on;
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
    let when = chrono::DateTime::from_timestamp_millis(view.saved.at_ms as i64).map(|t| when_words(t.with_timezone(&chrono::Local).naive_local(), chrono::Local::now().naive_local())).unwrap_or_default();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(rect.center(), egui::vec2(width, rect.height().min(560.0)))));
    egui::Frame::NONE.fill(crate::theme::colors().panel).corner_radius(12.0).stroke(egui::Stroke::new(1.0, crate::theme::colors().border_strong())).inner_margin(20.0).show(&mut child, |ui| {
        ui.label(RichText::new(tr("restore.title")).size(18.0).strong().color(pal.fg));
        let lead = if panes.len() == 1 { trf("restore.lead_one", &[&when]) } else { trf("restore.lead", &[&panes.len().to_string(), &when]) };
        ui.label(RichText::new(lead).color(pal.fg_dim));
        ui.add_space(10.0);
        // The rows scroll, so Restore and Start fresh stay in sight however
        // many there were.
        if view.ticked.len() > 1 {
            let count = view.ticked.iter().filter(|(_, on)| *on).count();
            let mut all = count == view.ticked.len();
            let some = count > 0 && !all;
            let ticked = &mut view.ticked;
            egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                let label = RichText::new(tr("restore.select_all")).size(12.5).color(pal.fg_dim);
                if ui.add(egui::Checkbox::new(&mut all, label).indeterminate(some)).clicked() {
                    tick_all(ticked);
                }
            });
        }
        let room = (ui.available_height() - 60.0).max(80.0);
        egui::ScrollArea::vertical().id_salt("restore-rows").max_height(room).auto_shrink([false, true]).show(ui, |ui| {
            for &i in &order {
                let p = panes[i];
                let Some(slot) = view.ticked.iter_mut().find(|(id, _)| *id == p.id) else { continue };
                let waiting = matches!(p.state, State::Waiting | State::MaybeWaiting);
                let frame = if waiting { egui::Frame::NONE.fill(crate::theme::colors().wait_bg()).stroke(egui::Stroke::new(1.0, gold().gamma_multiply(0.6))) } else { egui::Frame::NONE };
                frame.corner_radius(7.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut slot.1, "");
                        let color = if p.claude.is_some() || p.state != State::Done { state_color(p.state) } else { grey() };
                        ui.label(RichText::new("●").color(color));
                        let command = p.shell.as_ref().map_or_else(ito_pane::default_program, |(c, _)| c.clone());
                        let name = if p.title.is_empty() { crate::home_short(&p.cwd) } else { crate::sort::display_title(&p.title, &command) };
                        // What it will do on the right first; the name gets the
                        // room left and is cut short in it, never over it.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let what = match (&p.claude, p.state) {
                                (Some(_), _) => tr("restore.resume").to_owned(),
                                (None, State::Done) if !slot.1 => tr("restore.left_closed").to_owned(),
                                (None, _) => trf("restore.new_shell", &[&crate::home_short(&p.cwd)]),
                            };
                            // At most under half the row, so the name always shows.
                            let room = ui.available_width() * 0.45;
                            ui.scope(|ui| {
                                ui.set_max_width(room);
                                ui.add(egui::Label::new(RichText::new(what).size(12.0).color(pal.fg_dim)).truncate());
                            });
                            ui.add_space(12.0);
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(RichText::new(name).color(if slot.1 { pal.fg } else { pal.fg_dim })).truncate());
                            });
                        });
                    });
                });
            }
        });
        ui.add_space(10.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut view.always, RichText::new(tr("restore.always")).size(12.5).color(pal.fg_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let picked: Vec<SessionId> = view.ticked.iter().filter(|(_, on)| *on).map(|(id, _)| *id).collect();
                let restore = egui::Button::new(RichText::new(trf("restore.restore", &[&picked.len().to_string()])).strong().color(crate::theme::colors().on_accent())).fill(cyan());
                if ui.add_enabled(!picked.is_empty(), restore).clicked() {
                    answer = Some(RestoreAnswer::Restore(picked));
                }
                if ui.button(tr("restore.fresh")).clicked() {
                    answer = Some(RestoreAnswer::Fresh);
                }
            });
        });
    });
    answer
}

/// When the tabs were saved, as a person says it: `today at 09:12`,
/// `yesterday at 23:41`, else the date.
fn when_words(at: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> String {
    let days = (now.date() - at.date()).num_days();
    match days {
        0 => trf("restore.today", &[&at.format("%H:%M").to_string()]),
        1 => trf("restore.yesterday", &[&at.format("%H:%M").to_string()]),
        _ => at.format("%Y/%m/%d %H:%M").to_string(),
    }
}

/// Several controls at the right of a row, laid out left to right so Tab
/// goes through them in the order they read (a row places its control right
/// to left, which would send Tab from the right). The width is the one they
/// took last frame; the first frame they sit at the left, then settle.
pub fn in_order<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // `unique_id`: rows built alike share `id`, and would share the width.
    let key = ui.unique_id().with("in-order");
    // At first all the room there is, so nothing is squeezed while it is
    // measured.
    let w = ui.data(|d| d.get_temp::<f32>(key)).unwrap_or(ui.available_width());
    let h = ui.available_height();
    let shown = ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::left_to_right(egui::Align::Center), add);
    let now = shown.response.rect.width();
    if (now - w).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(key, now));
        ui.ctx().request_repaint();
    }
    shown.inner
}

/// A dialog's buttons, the same everywhere (the new-session dialog's, the
/// design's): at the right, Cancel then the main one, in the order they read
/// and Tab goes. Whether each was clicked, and their responses (for a ring).
pub fn foot(ui: &mut egui::Ui, main: egui::Button<'_>, enabled: bool, cancel: &str) -> (egui::Response, egui::Response) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        in_order(ui, |ui| {
            let c = ui.add(egui::Button::new(cancel).min_size(egui::vec2(80.0, 30.0)));
            let m = ui.add_enabled(enabled, main.min_size(egui::vec2(90.0, 30.0)));
            (m, c)
        })
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::{BREATHE, SWEEP, breathe, edge_at, elapsed, sweep_at, when_words};

    /// "Select all" ticks every row, finished ones too, unless all were
    /// ticked; then it clears them.
    #[test]
    fn select_all_ticks_every_row_then_none() {
        let mut ticked = vec![(1, true), (2, false), (3, false)];
        super::tick_all(&mut ticked);
        assert!(ticked.iter().all(|(_, on)| *on));
        super::tick_all(&mut ticked);
        assert!(ticked.iter().all(|(_, on)| !*on));
        super::tick_all(&mut ticked);
        assert!(ticked.iter().all(|(_, on)| *on));
    }

    /// Every dialog's Cancel goes through `foot`, so the buttons sit alike
    /// everywhere: no Cancel button made by hand anywhere else.
    #[test]
    fn every_cancel_is_a_dialogs_foot() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (k, line) in text.lines().enumerate() {
                let by_hand = ["button(\"Cancel\")", "Button::new(\"Cancel\")", "button(\"Keep\")", "Button::new(\"Keep\")"];
                assert!(!by_hand.iter().any(|b| line.contains(b)), "{}:{}: a Cancel made by hand; use chrome::foot", path.display(), k + 1);
            }
        }
    }

    #[test]
    fn the_states_read_on_every_theme() {
        for t in crate::theme::builtin() {
            crate::theme::set(t.colors);
            for c in [t.colors.wait, t.colors.err, t.colors.done, t.colors.run] {
                let ink = super::ink(c);
                for g in [t.colors.panel, t.colors.side, t.colors.wait_bg(), t.colors.bg] {
                    let r = crate::theme::contrast(ink, g);
                    assert!(r >= 4.5, "{}: {c:?} on {g:?} is {r:.2}", t.name);
                }
            }
        }
        crate::theme::set(crate::theme::builtin()[0].colors);
    }

    #[test]
    fn the_save_is_dated_as_a_person_says_it() {
        let t = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(when_words(t("2026-10-05 23:41"), t("2026-10-06 08:00")), "yesterday at 23:41");
        assert_eq!(when_words(t("2026-10-06 07:02"), t("2026-10-06 08:00")), "today at 07:02");
        assert_eq!(when_words(t("2026-10-01 07:02"), t("2026-10-06 08:00")), "2026/10/01 07:02");
    }
    use eframe::egui::{ResizeDirection as D, pos2, Rect};

    #[test]
    fn the_edges_and_corners_resize() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
        assert_eq!(edge_at(r, pos2(798.0, 300.0)), Some(D::East));
        assert_eq!(edge_at(r, pos2(2.0, 300.0)), Some(D::West));
        assert_eq!(edge_at(r, pos2(400.0, 597.0)), Some(D::South));
        assert_eq!(edge_at(r, pos2(400.0, 1.0)), Some(D::North));
        assert_eq!(edge_at(r, pos2(400.0, 5.0)), None, "the top edge is thin: the band is dragged there");
        assert_eq!(edge_at(r, pos2(798.0, 595.0)), Some(D::SouthEast));
        assert_eq!(edge_at(r, pos2(790.0, 1.0)), Some(D::NorthEast), "the corners reach further along");
        assert_eq!(edge_at(r, pos2(400.0, 300.0)), None);
    }

    #[test]
    fn the_ring_breathes_and_the_line_runs_round() {
        assert!(breathe(0.0) < 0.001, "faint at the start");
        assert!((breathe(BREATHE / 2.0) - 1.0).abs() < 0.001, "brightest halfway");
        assert!((breathe(BREATHE * 3.0) - breathe(0.0)).abs() < 0.001, "and round again");
        assert!((sweep_at(0.0) + 0.3).abs() < 0.001, "starts out of sight on the left");
        assert!(sweep_at(SWEEP * 0.999) > 0.99, "ends gone on the right");
        assert!((sweep_at(SWEEP * 2.5) - sweep_at(SWEEP * 0.5)).abs() < 0.001);
    }

    #[test]
    fn time_is_said_the_way_the_design_says_it() {
        assert_eq!(elapsed(14_000), "14s");
        assert_eq!(elapsed(252_000), "4m 12s");
        assert_eq!(elapsed(720_000), "12m");
        assert_eq!(elapsed(3_840_000), "1h 04m");
        assert_eq!(elapsed(2 * 86_400_000 + 3_600_000), "2d 1h");
    }

    /// The band's tags: `shown` of them and +N, fewer as the room narrows,
    /// then +N alone, then nothing (the box never moves for them).
    #[test]
    fn the_tags_give_way_to_plus_n() {
        let w = [40.0, 40.0, 40.0, 40.0];
        let more = |_| 20.0;
        assert_eq!(super::tags_that_fit(&w, 3, 1000.0, more), (3, 3.0 * 45.0 + 20.0));
        assert_eq!(super::tags_that_fit(&w, 5, 1000.0, more), (4, 4.0 * 45.0 - 5.0));
        assert_eq!(super::tags_that_fit(&w, 3, 120.0, more), (2, 2.0 * 45.0 + 20.0));
        assert_eq!(super::tags_that_fit(&w, 3, 30.0, more), (0, 20.0));
        assert_eq!(super::tags_that_fit(&w, 3, 10.0, more), (0, 0.0));
        assert_eq!(super::tags_that_fit(&[], 3, 10.0, more), (0, 0.0));
    }
}
