//! Several pieces of work on one repository at once: each in a git worktree
//! of its own, on its own branch, with Claude Code started there on its own
//! prompt. One dialog for what took a worktree, a tab and a prompt each.

use std::path::PathBuf;

use eframe::egui::{self, RichText};

use crate::theme::Colors;

/// How many pieces of work at most.
pub const MOST: usize = 8;

pub struct View {
    pub folder: String,
    pub tasks: Vec<String>,
    opening: bool,
}

/// What to start.
#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    pub folder: PathBuf,
    pub tasks: Vec<String>,
}

impl View {
    pub fn new(folder: &std::path::Path) -> Self {
        Self { folder: folder.display().to_string(), tasks: vec![String::new(), String::new()], opening: true }
    }

    /// The pieces of work written, without the empty ones.
    pub fn start(&self) -> Option<Start> {
        let tasks: Vec<String> = self.tasks.iter().map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()).collect();
        (!tasks.is_empty() && !self.folder.trim().is_empty()).then(|| Start { folder: PathBuf::from(self.folder.trim()), tasks })
    }
}

/// The branch of the `k`th piece of work (from 0) started at `stamp`
/// (`tsumugi/1007-1432`): `tsumugi/1007-1432-1`.
pub fn branch(stamp: &str, k: usize) -> String {
    format!("{stamp}-{}", k + 1)
}

/// The line typed in a new pane: Claude Code given the prompt, quoted for
/// the shell, its lines run into one.
pub fn typed(claude: &str, task: &str, how: tsumugi_pane::Quoting) -> String {
    let one: String = task.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{claude} {}", tsumugi_pane::quote(&one, how))
}

pub enum Answer {
    Start(Start),
    Close,
}

pub fn show(ctx: &egui::Context, view: &mut View, c: &Colors) -> Option<Answer> {
    let mut answer = None;
    let screen = ctx.content_rect();
    let dim = egui::Area::new(egui::Id::new("parallel-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
        resp
    });
    if (dim.inner.clicked() && !view.opening) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(Answer::Close);
    }
    let first = std::mem::take(&mut view.opening);
    egui::Area::new(egui::Id::new("parallel")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 70.0)).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(20, 16)).show(ui, |ui| {
            ui.set_width((screen.width() - 40.0).clamp(300.0, 600.0));
            ui.label(RichText::new("Start in parallel").size(16.0).strong().color(c.strong()));
            ui.label(RichText::new("Each piece of work gets a git worktree beside the repository, on a branch of its own, and a tab with Claude Code started on its prompt.").size(12.0).color(c.dim));
            ui.add_space(10.0);
            ui.label(RichText::new("REPOSITORY").size(11.0).color(c.faint()));
            ui.add(egui::TextEdit::singleline(&mut view.folder).font(egui::FontId::monospace(12.5)).desired_width(f32::INFINITY));
            ui.add_space(8.0);
            ui.label(RichText::new("WORK, ONE PROMPT EACH").size(11.0).color(c.faint()));
            let mut gone = None;
            for (k, t) in view.tasks.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}", k + 1)).monospace().color(c.dim));
                    let field = ui.add(egui::TextEdit::multiline(t).desired_rows(2).desired_width(ui.available_width() - 30.0).hint_text("What this one should do"));
                    if first && k == 0 {
                        field.request_focus();
                    }
                    if ui.small_button("×").on_hover_text("Take it out").clicked() {
                        gone = Some(k);
                    }
                });
            }
            if let Some(k) = gone {
                view.tasks.remove(k);
            }
            if view.tasks.len() < MOST && ui.button("+ Another").clicked() {
                view.tasks.push(String::new());
            }
            ui.add_space(10.0);
            let ready = view.start();
            ui.horizontal(|ui| {
                let n = ready.as_ref().map_or(0, |s| s.tasks.len());
                let go = egui::Button::new(RichText::new(format!("Start {n}")).color(c.on_accent()).strong()).fill(c.run);
                let (go, cancel) = crate::chrome::foot(ui, go, ready.is_some(), "Cancel");
                if go.clicked() {
                    answer = ready.clone().map(Answer::Start);
                }
                if cancel.clicked() {
                    answer = Some(Answer::Close);
                }
            });
        });
    });
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_is_named_and_typed_safely() {
        assert_eq!(branch("tsumugi/1007-1432", 0), "tsumugi/1007-1432-1");
        let line = typed("claude", "Fix the 'login'\n  bug", tsumugi_pane::Quoting::Posix);
        assert!(line.starts_with("claude '") && line.contains("Fix the") && !line.contains('\n'), "{line}");
        let mut v = View::new(std::path::Path::new("/r"));
        assert_eq!(v.start(), None, "nothing written");
        v.tasks[1] = "  b ".into();
        assert_eq!(v.start(), Some(Start { folder: "/r".into(), tasks: vec!["b".into()] }));
    }
}
