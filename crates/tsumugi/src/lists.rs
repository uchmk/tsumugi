//! Three lists over the window, on three pages of one panel:
//!
//! - **All sessions**: every session on one screen, the ones waiting for a
//!   person first, with its state, folder, tags, tokens and last words;
//!   Up and Down pick one, Enter (or a click) goes to it.
//! - **Waiting**: every session waiting for a person, with what it said and
//!   its menu's choices as buttons, picked by a tick each, and "Yes" or "No"
//!   typed into all those picked at once. Only a menu on the session's
//!   screen is answered; one with no "Yes" (or "No") choice is left alone.
//! - **Recently closed**: the sessions that ended (`history.rs`), with the
//!   tokens they used and their last lines, to start again where they were.

use std::collections::HashSet;

use eframe::egui::{self, RichText};
use tsumugi_mux::SessionId;

use crate::answer::{self, Choice};
use crate::history::Closed;
use crate::theme::Colors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    All,
    Waiting,
    Closed,
}

/// The panel while it is open.
pub struct View {
    pub page: Page,
    /// Waiting sessions whose tick was taken off.
    off: HashSet<SessionId>,
    /// The closed session whose last lines are shown.
    shown: Option<usize>,
    /// The waiting session whose "Always allow" is being confirmed.
    confirming: Option<SessionId>,
    /// Opened this frame: the click that opened it is not a click outside.
    opening: bool,
    /// The row of All sessions the arrow keys are on.
    picked: usize,
}

impl View {
    pub fn new(page: Page) -> Self {
        Self { page, off: HashSet::new(), shown: None, confirming: None, opening: true, picked: 0 }
    }
}

/// A waiting session as the list shows it.
pub struct Row {
    pub id: SessionId,
    pub name: String,
    pub folder: String,
    /// What it said (its notification), if anything.
    pub note: String,
    /// How long it has waited.
    pub waited: String,
    pub choices: Vec<Choice>,
    /// What the question is about, as its screen says it (the command, the
    /// file), read above the menu.
    pub asking: Vec<String>,
    /// Claude Code's rule that would allow it from now on (`permit.rs`),
    /// when it asks about one Bash command.
    pub rule: Option<String>,
    /// Where that rule would be written (its project's settings).
    pub rule_file: std::path::PathBuf,
}

/// A session as All sessions shows it.
pub struct Card {
    pub id: SessionId,
    pub name: String,
    pub state: tsumugi_mux::State,
    /// What it is doing and for how long (`Running · 3m`).
    pub words: String,
    /// Its folder, and its branch when it has one.
    pub folder: String,
    pub tags: Vec<String>,
    /// Tokens its conversation used, as words (`12.3k tokens`), or empty.
    pub tokens: String,
    /// The last lines on its screen.
    pub last: Vec<String>,
}

/// Which comes first in All sessions: those waiting for a person, then
/// errors, then running, then done; within one, the longest in it first.
pub fn rank(state: tsumugi_mux::State, since_ms: u64) -> (u8, u64) {
    use tsumugi_mux::State;
    let k = match state {
        State::Waiting => 0,
        State::MaybeWaiting => 1,
        State::Error => 2,
        State::Running => 3,
        State::Done => 4,
    };
    (k, since_ms)
}

/// What the panel asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Do {
    /// Type this key into the session.
    Type(SessionId, char),
    /// Write Claude Code's rule into the project's settings, then say yes.
    Allow(SessionId, String),
    /// Go to the session (and close the panel).
    Go(SessionId),
    /// Start the closed session again in its folder.
    StartAgain(usize),
    /// Put text on the clipboard.
    Copy(String),
    /// Save the closed session's last lines to a file.
    Save(usize),
    /// Take a closed session off the list.
    Forget(usize),
    /// Take every closed session off the list.
    ForgetAll,
    Close,
}

/// The keys of the choices "Yes to all" (or "No to all") would type: one
/// per ticked row whose menu has that answer.
pub fn bulk(rows: &[Row], off: &HashSet<SessionId>, yes: bool) -> Vec<(SessionId, char)> {
    rows.iter()
        .filter(|r| !off.contains(&r.id))
        .filter_map(|r| if yes { answer::yes(&r.choices) } else { answer::no(&r.choices) }.map(|k| (r.id, k)))
        .collect()
}

/// `all` is filled only while its page shows; `count` is how many there are.
pub fn show(ctx: &egui::Context, view: &mut View, all: &[Card], count: usize, rows: &[Row], closed: &[Closed], c: &Colors) -> Vec<Do> {
    let mut out = Vec::new();
    let screen = ctx.content_rect();
    // The rest of the window dimmed behind it; a click there closes it.
    let dim = egui::Area::new(egui::Id::new("lists-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
        resp
    });
    if dim.inner.clicked() && !view.opening {
        out.push(Do::Close);
    }
    view.opening = false;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        out.push(Do::Close);
    }
    let width = (screen.width() - 32.0).clamp(280.0, 640.0);
    egui::Area::new(egui::Id::new("lists")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0)).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                for (page, title) in [(Page::All, format!("All sessions · {count}")), (Page::Waiting, format!("Waiting · {}", rows.len())), (Page::Closed, format!("Recently closed · {}", closed.len()))] {
                    if ui.selectable_label(view.page == page, RichText::new(title).size(14.0).strong()).clicked() {
                        view.page = page;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Close (Esc)").clicked() {
                        out.push(Do::Close);
                    }
                });
            });
            ui.separator();
            let max = (screen.height() - 220.0).max(160.0);
            match view.page {
                Page::All => overview(ui, view, all, c, max, &mut out),
                Page::Waiting => waiting(ui, view, rows, c, max, &mut out),
                Page::Closed => history(ui, view, closed, c, max, &mut out),
            }
        });
    });
    out
}

fn waiting(ui: &mut egui::Ui, view: &mut View, rows: &[Row], c: &Colors, max: f32, out: &mut Vec<Do>) {
    view.off.retain(|id| rows.iter().any(|r| r.id == *id));
    if rows.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No session is waiting for you.").size(13.0).color(c.dim));
        ui.add_space(12.0);
        return;
    }
    egui::ScrollArea::vertical().max_height(max).auto_shrink([false, true]).show(ui, |ui| {
        for r in rows {
            ui.horizontal(|ui| {
                let mut on = !view.off.contains(&r.id);
                if ui.checkbox(&mut on, "").on_hover_text("Answered by Yes to all / No to all").changed() {
                    if on {
                        view.off.remove(&r.id);
                    } else {
                        view.off.insert(r.id);
                    }
                }
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui.link(RichText::new(&r.name).size(13.5).strong().color(c.strong())).on_hover_text("Go to it").clicked() {
                            out.push(Do::Go(r.id));
                        }
                        ui.label(RichText::new(format!("{} · {}", r.folder, r.waited)).size(11.5).color(c.dim));
                    });
                    if !r.note.is_empty() {
                        ui.label(RichText::new(&r.note).size(12.0).color(crate::chrome::ink(crate::chrome::gold())));
                    }
                    // What would be said yes to: read before answering.
                    if !r.asking.is_empty() {
                        egui::Frame::NONE.fill(c.bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for l in &r.asking {
                                ui.label(RichText::new(l).monospace().size(11.5).color(c.fg));
                            }
                        });
                    }
                    // The command on Claude Code's list from now on: the exact
                    // rule and file shown before anything is written.
                    if let Some(rule) = &r.rule {
                        if view.confirming == Some(r.id) {
                            ui.label(RichText::new(format!("Adds {rule} to {}, then says yes", r.rule_file.display())).size(11.5).color(c.dim));
                            ui.horizontal(|ui| {
                                let (yes, cancel) = crate::chrome::foot(ui, egui::Button::new(RichText::new("Add and say yes").strong()), true, "Cancel");
                                if yes.clicked() {
                                    out.push(Do::Allow(r.id, rule.clone()));
                                    view.confirming = None;
                                }
                                if cancel.clicked() {
                                    view.confirming = None;
                                }
                            });
                        }
                    }
                    ui.horizontal_wrapped(|ui| {
                        if r.choices.is_empty() {
                            ui.label(RichText::new("No menu on its screen: answer it there").size(11.5).color(c.faint()));
                        }
                        for ch in &r.choices {
                            let label = format!("{} {}", ch.key, answer::short(&ch.text));
                            if ui.button(RichText::new(label).size(12.0)).on_hover_text(format!("Type {}: {}", ch.key, ch.text)).clicked() {
                                out.push(Do::Type(r.id, ch.key));
                            }
                        }
                        let offered = r.rule.is_some() && view.confirming != Some(r.id) && answer::yes(&r.choices).is_some();
                        if offered && ui.button(RichText::new("Always allow…").size(12.0)).on_hover_text("Claude Code runs this exact command without asking from now on, in this project").clicked() {
                            view.confirming = Some(r.id);
                        }
                    });
                });
            });
            ui.separator();
        }
    });
    ui.add_space(6.0);
    let yes = bulk(rows, &view.off, true);
    let no = bulk(rows, &view.off, false);
    ui.horizontal(|ui| {
        let ticked = rows.iter().filter(|r| !view.off.contains(&r.id)).count();
        if ui.small_button(if ticked == rows.len() { "Untick all" } else { "Tick all" }).clicked() {
            if ticked == rows.len() {
                view.off = rows.iter().map(|r| r.id).collect();
            } else {
                view.off.clear();
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let no_button = egui::Button::new(RichText::new(format!("No to {}", no.len())).strong()).min_size(egui::vec2(90.0, 28.0));
            if ui.add_enabled(!no.is_empty(), no_button).on_hover_text("Types each ticked menu's \"No\" choice").on_disabled_hover_text("No ticked menu has a \"No\" choice").clicked() {
                out.extend(no.iter().map(|(id, k)| Do::Type(*id, *k)));
            }
            let yes_button = egui::Button::new(RichText::new(format!("Yes to {}", yes.len())).color(c.on_accent()).strong()).fill(c.run).min_size(egui::vec2(90.0, 28.0));
            if ui.add_enabled(!yes.is_empty(), yes_button).on_hover_text("Types 1 (Yes, once) into each ticked menu").on_disabled_hover_text("No ticked menu starts with \"Yes\"").clicked() {
                out.extend(yes.iter().map(|(id, k)| Do::Type(*id, *k)));
            }
        });
    });
}

fn overview(ui: &mut egui::Ui, view: &mut View, all: &[Card], c: &Colors, max: f32, out: &mut Vec<Do>) {
    if all.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No session is running.").size(13.0).color(c.dim));
        ui.add_space(12.0);
        return;
    }
    let (up, down, enter) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
    view.picked = if up { view.picked.saturating_sub(1) } else if down { view.picked + 1 } else { view.picked }.min(all.len() - 1);
    if enter {
        out.push(Do::Go(all[view.picked].id));
    }
    egui::ScrollArea::vertical().max_height(max).auto_shrink([false, true]).show(ui, |ui| {
        for (k, s) in all.iter().enumerate() {
            let picked = k == view.picked;
            let frame = egui::Frame::NONE.fill(if picked { c.hover() } else { egui::Color32::TRANSPARENT }).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8, 6));
            let shown = frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.5, crate::chrome::state_color(s.state));
                    ui.label(RichText::new(&s.name).size(13.5).strong().color(c.strong()));
                    ui.label(RichText::new(&s.words).size(11.5).color(crate::chrome::state_ink(s.state)));
                });
                let mut facts = vec![s.folder.clone()];
                facts.extend(s.tags.iter().map(|t| format!("#{t}")));
                if !s.tokens.is_empty() {
                    facts.push(s.tokens.clone());
                }
                ui.label(RichText::new(facts.join(" · ")).size(11.5).color(c.dim));
                for l in &s.last {
                    ui.label(RichText::new(l).monospace().size(11.0).color(c.fg));
                }
            });
            let resp = ui.interact(shown.response.rect, egui::Id::new(("overview", s.id)), egui::Sense::click()).on_hover_text("Go to it (Enter)");
            if picked && (up || down) {
                resp.scroll_to_me(None);
            }
            if resp.clicked() {
                out.push(Do::Go(s.id));
            }
        }
    });
}

fn history(ui: &mut egui::Ui, view: &mut View, closed: &[Closed], c: &Colors, max: f32, out: &mut Vec<Do>) {
    if closed.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new("No session has ended yet.").size(13.0).color(c.dim));
        ui.add_space(12.0);
        return;
    }
    egui::ScrollArea::vertical().max_height(max).auto_shrink([false, true]).show(ui, |ui| {
        for (k, s) in closed.iter().enumerate() {
            let name = crate::sort::display_title(&s.title, &s.command);
            let folder = s.cwd.file_name().map_or_else(|| s.cwd.display().to_string(), |f| f.to_string_lossy().into_owned());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(13.5).strong().color(c.strong()));
                        ui.label(RichText::new(folder).size(11.5).color(c.dim)).on_hover_text(s.cwd.display().to_string());
                    });
                    let mut facts = vec![ended_words(s.ended_ms)];
                    if !s.state.is_empty() {
                        facts.push(s.state.clone());
                    }
                    if s.tokens > 0 {
                        facts.push(format!("{} tokens", crate::usage::short(s.tokens)));
                    }
                    if s.cost > 0.0 {
                        facts.push(format!("≈{}", crate::price::dollars(s.cost)));
                    }
                    ui.label(RichText::new(facts.join(" · ")).size(11.5).color(c.dim));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Take it off the list").clicked() {
                        out.push(Do::Forget(k));
                    }
                    let again = if s.claude && !s.conversation.is_empty() { "Resume" } else { "Start again" };
                    if ui.small_button(again).on_hover_text("A new tab in its folder; Claude Code resumes its conversation").clicked() {
                        out.push(Do::StartAgain(k));
                    }
                    if !s.last.is_empty() {
                        let open = view.shown == Some(k);
                        if ui.small_button(if open { "Hide output" } else { "Last output" }).clicked() {
                            view.shown = if open { None } else { Some(k) };
                        }
                    }
                });
            });
            if view.shown == Some(k) {
                egui::Frame::NONE.fill(c.bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for l in &s.last {
                        ui.label(RichText::new(l).monospace().size(11.5).color(c.fg));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.small_button("Copy the output").clicked() {
                        out.push(Do::Copy(s.last.join("\n")));
                    }
                    if ui.small_button("Save to a file").on_hover_text("These lines, as text in Downloads").clicked() {
                        out.push(Do::Save(k));
                    }
                });
            }
            ui.separator();
        }
    });
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Clear the list").clicked() {
                out.push(Do::ForgetAll);
            }
        });
    });
}

/// `today at 14:02`, `yesterday at 09:10`, `3 Oct at 18:40`.
fn ended_words(ms: u64) -> String {
    use chrono::TimeZone;
    let Some(t) = chrono::Local.timestamp_millis_opt(ms as i64).single() else { return String::new() };
    let today = chrono::Local::now().date_naive();
    let day = t.date_naive();
    let time = t.format("%H:%M");
    if day == today {
        format!("today at {time}")
    } else if today.pred_opt() == Some(day) {
        format!("yesterday at {time}")
    } else {
        format!("{} at {time}", t.format("%-d %b"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: SessionId, texts: &[&str]) -> Row {
        let choices = texts.iter().enumerate().map(|(k, t)| Choice { key: char::from_digit(k as u32 + 1, 10).unwrap(), text: t.to_string() }).collect();
        Row { id, name: format!("s{id}"), folder: "f".into(), note: String::new(), waited: "1m".into(), choices, asking: Vec::new(), rule: None, rule_file: Default::default() }
    }

    #[test]
    fn those_waiting_for_a_person_come_first() {
        use tsumugi_mux::State;
        let mut all = [(State::Done, 1), (State::Running, 5), (State::Waiting, 9), (State::Error, 3), (State::Waiting, 2), (State::MaybeWaiting, 1)];
        all.sort_by_key(|(s, t)| rank(*s, *t));
        assert_eq!(all, [(State::Waiting, 2), (State::Waiting, 9), (State::MaybeWaiting, 1), (State::Error, 3), (State::Running, 5), (State::Done, 1)]);
    }

    #[test]
    fn all_at_once_answers_only_the_ticked_menus_that_say_so() {
        let rows = vec![row(1, &["Yes", "Yes, and don't ask again", "No, and tell Claude"]), row(2, &["Use A", "Use B"]), row(3, &[]), row(4, &["Yes", "No"])];
        let mut off = HashSet::new();
        assert_eq!(bulk(&rows, &off, true), vec![(1, '1'), (4, '1')]);
        assert_eq!(bulk(&rows, &off, false), vec![(1, '3'), (4, '2')]);
        off.insert(4);
        assert_eq!(bulk(&rows, &off, true), vec![(1, '1')], "an unticked row is left alone");
    }
}
