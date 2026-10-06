//! The input box (the design's 12, v1-scope 1l, `Ctrl+I`): a field below the
//! panes to write a prompt in as in any editor -- `Enter` is a new line,
//! `Ctrl+Enter` sends -- and then hand it to the session whole. No fight
//! with `Shift+Enter` inside a TUI. Files dropped on the window become
//! chips whose paths go with the prompt; `↑` brings back what was sent; a
//! tag sends the same prompt to every session wearing it.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, FontId, RichText};
use tsumugi_mux::SessionId;

use crate::theme::Colors;

/// How many prompts the history keeps.
const HISTORY: usize = 100;

/// One session's draft.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub text: String,
    pub files: Vec<PathBuf>,
}

#[derive(Default)]
pub struct InputBox {
    pub open: bool,
    /// Each session's draft, kept while the box is closed or another
    /// session is shown.
    drafts: HashMap<SessionId, Draft>,
    /// What was sent, oldest first.
    pub history: Vec<String>,
    /// Where `↑` has got to in the history, and the draft it left.
    back: Option<(usize, String)>,
    /// Tags chosen as where to send instead of the pane.
    to_tags: Vec<String>,
    /// Focus the field on the next frame.
    focus: bool,
}

/// Where a prompt goes.
#[derive(Clone, Debug, PartialEq)]
pub enum To {
    Session(SessionId),
    /// Every session wearing any of these.
    Tags(Vec<String>),
}

pub struct Send {
    pub to: To,
    pub text: String,
}

impl InputBox {
    /// Closed, with the prompts sent before.
    pub fn with_history(history: Vec<String>) -> Self {
        Self { history, ..Self::default() }
    }

    /// Open it and give it the keys, or close it.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus = self.open;
    }

    /// Files dropped on the window: on the draft of `session`, and the box
    /// open to show them.
    pub fn drop_files(&mut self, session: SessionId, files: Vec<PathBuf>) {
        let d = self.drafts.entry(session).or_default();
        for f in files {
            if !d.files.contains(&f) {
                d.files.push(f);
            }
        }
        self.open = true;
        self.focus = true;
    }

    /// The prompt as sent: the text, then the files' paths, each quoted when
    /// it has a space, so Claude Code reads them as files.
    pub fn prompt(d: &Draft) -> String {
        let paths: Vec<String> = d
            .files
            .iter()
            .map(|p| {
                let s = p.display().to_string();
                if s.contains(' ') { format!("\"{s}\"") } else { s }
            })
            .collect();
        match (d.text.trim().is_empty(), paths.is_empty()) {
            (_, true) => d.text.trim_end().to_owned(),
            (true, false) => paths.join(" "),
            (false, false) => format!("{}\n{}", d.text.trim_end(), paths.join(" ")),
        }
    }

    fn remember(&mut self, text: &str) {
        if text.trim().is_empty() || self.history.last().is_some_and(|l| l == text) {
            return;
        }
        self.history.push(text.to_owned());
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    /// `↑`: the prompt sent before the one shown (the draft is kept to come
    /// back to with `↓`).
    fn older(&mut self, draft: &mut Draft) {
        let at = match &self.back {
            Some((k, _)) => k.checked_sub(1),
            None => self.history.len().checked_sub(1),
        };
        let Some(k) = at else { return };
        if self.back.is_none() {
            self.back = Some((k, draft.text.clone()));
        }
        if let Some((pos, _)) = &mut self.back {
            *pos = k;
        }
        draft.text.clone_from(&self.history[k]);
    }

    /// `↓`: the one after, and past the newest, the draft as it was.
    fn newer(&mut self, draft: &mut Draft) {
        let Some((k, kept)) = self.back.take() else { return };
        if k + 1 < self.history.len() {
            draft.text.clone_from(&self.history[k + 1]);
            self.back = Some((k + 1, kept));
        } else {
            draft.text = kept;
        }
    }

    /// The box, for the session with the keys (`to`, called `name`), with
    /// the tags in use to send to instead. What to send, when it is sent.
    pub fn show(&mut self, ui: &mut egui::Ui, c: &Colors, to: SessionId, name: &str, tags: &[String]) -> Option<Send> {
        self.to_tags.retain(|t| tags.contains(t));
        let id = egui::Id::new("input-box");
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let mut draft = self.drafts.remove(&to).unwrap_or_default();
        let mut sent = None;
        // The keys the field would take otherwise: send, history, close.
        // `↑` and `↓` walk the history only while the field is empty or shows
        // a line from it; in a draft of several lines they move the cursor.
        let walking = draft.text.is_empty() || (self.back.is_some() && !draft.text.contains('\n'));
        let (send, up, down, close) = ui.input_mut(|i| {
            if !focused {
                return (false, false, false, false);
            }
            let cmd = egui::Modifiers::COMMAND;
            (
                i.consume_key(cmd, egui::Key::Enter),
                walking && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                walking && self.back.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(cmd, egui::Key::I) || i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if up {
            self.older(&mut draft);
        }
        if down {
            self.newer(&mut draft);
        }
        if close {
            self.open = false;
            // Back to the pane.
            ui.ctx().memory_mut(|m| m.surrender_focus(id));
        }

        let frame = egui::Frame::NONE
            .fill(c.side)
            .stroke(egui::Stroke::new(1.0, if focused { c.run } else { c.border_strong() }))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(12, 8));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Where it goes: the pane, or tags instead.
            ui.horizontal(|ui| {
                ui.label(RichText::new("To").size(12.0).color(c.dim));
                let pane_on = self.to_tags.is_empty();
                if ui.selectable_label(pane_on, RichText::new(name).size(12.0)).on_hover_text("The pane with the keys").clicked() {
                    self.to_tags.clear();
                }
                if !tags.is_empty() {
                    ui.label(RichText::new("or").size(12.0).color(c.faint()));
                }
                for t in tags {
                    let on = self.to_tags.contains(t);
                    let w = ui.fonts_mut(|f| f.layout_no_wrap(t.clone(), FontId::proportional(11.0), c.fg).size().x) + 12.0;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 16.0), egui::Sense::click());
                    crate::chrome::tag_chip(ui.painter(), r.min, t, !on);
                    if on {
                        ui.painter().rect_stroke(r.expand(1.5), 5.0, egui::Stroke::new(1.0, c.strong()), egui::StrokeKind::Outside);
                    }
                    if resp.on_hover_text(format!("Send to every session tagged {t}")).clicked() {
                        if on {
                            self.to_tags.retain(|x| x != t);
                        } else {
                            self.to_tags.push(t.clone());
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !draft.text.is_empty() {
                        ui.label(RichText::new("Draft kept").size(11.5).color(c.faint()));
                    }
                });
            });
            ui.add_space(4.0);
            let edit = egui::TextEdit::multiline(&mut draft.text)
                .id(id)
                .hint_text("Write a prompt: Enter is a new line, Ctrl+Enter sends")
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .font(FontId::proportional(14.0))
                .frame(egui::Frame::NONE);
            let resp = ui.add(edit);
            if std::mem::take(&mut self.focus) {
                resp.request_focus();
            }
            if resp.changed() {
                self.back = None;
            }
            ui.horizontal(|ui| {
                let mut gone = None;
                for (k, f) in draft.files.iter().enumerate() {
                    let name = f.file_name().map_or_else(|| f.display().to_string(), |n| n.to_string_lossy().into_owned());
                    let chip = egui::Button::new(RichText::new(format!("{name}  ×")).size(12.0)).fill(c.panel).stroke(egui::Stroke::new(1.0, c.border));
                    if ui.add(chip).on_hover_text(format!("{}: click to take it off", f.display())).clicked() {
                        gone = Some(k);
                    }
                }
                if let Some(k) = gone {
                    draft.files.remove(k);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let key = if cfg!(target_os = "macos") { "Cmd+Enter" } else { "Ctrl+Enter" };
                    let button = egui::Button::new(RichText::new(format!("Send   {key}")).color(c.on_accent()).strong()).fill(c.run).min_size(egui::vec2(110.0, 28.0));
                    let empty = draft.text.trim().is_empty() && draft.files.is_empty();
                    if (ui.add_enabled(!empty, button).clicked() || send) && !empty {
                        let text = Self::prompt(&draft);
                        let to = if self.to_tags.is_empty() { To::Session(to) } else { To::Tags(self.to_tags.clone()) };
                        sent = Some(Send { to, text: text.clone() });
                        self.remember(draft.text.trim_end());
                        self.back = None;
                        draft = Draft::default();
                    }
                    ui.label(RichText::new("↑ history · Enter new line · Esc back to the pane").size(11.5).color(c.faint()));
                });
            });
        });
        if draft != Draft::default() {
            self.drafts.insert(to, draft);
        }
        sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_carries_the_files() {
        let d = Draft { text: "look at these\n".into(), files: vec!["/tmp/a.png".into(), "/tmp/my shot.png".into()] };
        assert_eq!(InputBox::prompt(&d), "look at these\n/tmp/a.png \"/tmp/my shot.png\"");
        let only = Draft { text: " ".into(), files: vec!["/tmp/a.png".into()] };
        assert_eq!(InputBox::prompt(&only), "/tmp/a.png");
    }

    #[test]
    fn the_history_goes_back_and_forth() {
        let mut b = InputBox::default();
        for p in ["one", "two", "two", ""] {
            b.remember(p);
        }
        assert_eq!(b.history, ["one", "two"], "the same twice or nothing is kept once");
        let mut d = Draft { text: "draft".into(), files: vec![] };
        b.older(&mut d);
        assert_eq!(d.text, "two");
        b.older(&mut d);
        assert_eq!(d.text, "one");
        b.older(&mut d);
        assert_eq!(d.text, "one", "nothing older");
        b.newer(&mut d);
        assert_eq!(d.text, "two");
        b.newer(&mut d);
        assert_eq!(d.text, "draft", "back to what was being written");
    }
}
