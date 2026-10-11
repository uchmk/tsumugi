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

use crate::i18n::{tr, trf};
use crate::theme::Colors;

/// How many prompts the history keeps.
const HISTORY: usize = 100;

/// One session's draft.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draft {
    pub text: String,
    pub files: Vec<PathBuf>,
    /// The files' sizes in bytes, when known (read on a thread).
    pub sizes: HashMap<PathBuf, u64>,
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
    /// The session `back` was walked in: another session's draft is never
    /// put in place of this one's (the source review, 2026-10-07).
    back_for: Option<SessionId>,
    /// Tags chosen as where to send instead of the pane.
    to_tags: Vec<String>,
    /// Other sessions to send to as well as the pane (the same prompt to
    /// several at once).
    to_also: Vec<SessionId>,
    /// Focus the field on the next frame.
    focus: bool,
    /// Send when the session is next done rather than now.
    pub later: bool,
    /// Ctrl+V came with no text: the clipboard may hold an image.
    image_asked: bool,
    /// Text was pasted in the frame before: its key's release is not an
    /// image's.
    pasted_text_lately: bool,
    /// The V key's press was seen: typed, not pasted.
    v_down: bool,
    /// Prompts kept to send again (`prompts.rs`), and whether they changed
    /// here and want writing.
    pub prompts: Vec<crate::prompts::Prompt>,
    pub prompts_changed: bool,
    /// The name being given to the draft, to keep it as a prompt.
    naming: String,
    /// One of its menus was open last frame: when it closes, the keys come
    /// back to the box.
    menu_was_open: bool,
    /// The frame at whose end the field had the keys. egui takes them from
    /// it as an Esc arrives, before anything reads the Esc: this says the
    /// Esc is still the box's (closing it), not the shell's.
    had_keys_at: Option<u64>,
}

/// Where a prompt goes.
#[derive(Clone, Debug, PartialEq)]
pub enum To {
    Session(SessionId),
    /// Each of these: the pane and those picked beside it.
    Sessions(Vec<SessionId>),
    /// Every session wearing any of these.
    Tags(Vec<String>),
}

pub struct Send {
    pub to: To,
    pub text: String,
    /// Queued: to go when the session has finished what it is doing.
    pub later: bool,
}

impl InputBox {
    /// Closed, with the prompts sent before and those kept to send again.
    pub fn with_history(history: Vec<String>, prompts: Vec<crate::prompts::Prompt>) -> Self {
        Self { history, prompts, ..Self::default() }
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

    /// Files with their sizes, read on a thread: on the draft of `session`.
    pub fn sized_files(&mut self, session: SessionId, files: Vec<(PathBuf, u64)>) {
        let paths: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
        self.drop_files(session, paths);
        let d = self.drafts.entry(session).or_default();
        d.sizes.extend(files);
    }

    /// Whether the box had the keys as this frame began: they are not the
    /// shell's, even in the frame an Esc took them from it.
    pub fn had_keys(&self, ctx: &egui::Context) -> bool {
        self.had_keys_at.is_some_and(|f| f + 1 == ctx.cumulative_frame_nr())
    }

    /// Whether Ctrl+V was pressed in the box with no text to paste: the
    /// window then reads the clipboard's image (egui gives text only).
    pub fn take_image_request(&mut self) -> bool {
        std::mem::take(&mut self.image_asked)
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
    /// the tags in use to send to instead and the other sessions (`others`,
    /// by name) to send to as well. What to send, when it is sent.
    /// `queued`: prompts waiting to go to `to`, shown so they can be seen.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, c: &Colors, to: SessionId, name: &str, tags: &[String], others: &[(SessionId, String)], queued: usize) -> Option<Send> {
        self.to_tags.retain(|t| tags.contains(t));
        self.to_also.retain(|s| *s != to && others.iter().any(|(o, _)| o == s));
        let id = egui::Id::new("input-box");
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let esc_for_box = self.had_keys(ui.ctx()) && !focused && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if self.back_for != Some(to) {
            self.back = None;
            self.back_for = Some(to);
        }
        let mut draft = self.drafts.remove(&to).unwrap_or_default();
        let mut sent = None;
        // The keys the field would take otherwise: send, history, close.
        // `↑` and `↓` walk the history only while the field is empty or shows
        // a line from it; in a draft of several lines they move the cursor.
        let walking = draft.text.is_empty() || (self.back.is_some() && !draft.text.contains('\n'));
        // An Esc while one of its menus is open closes the menu, not the box.
        let menu_open = egui::Popup::is_any_open(ui.ctx());
        let mut menus_open = false;
        let (later_key, send, up, down, close) = ui.input_mut(|i| {
            if !focused {
                return (false, false, false, false, false);
            }
            let cmd = egui::Modifiers::COMMAND;
            // Up and Down with nothing held: egui's own match lets Shift
            // through, and Shift+↑ is the selection's, not the history's.
            let bare = |i: &egui::InputState, key: egui::Key| !i.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && !modifiers.is_none()));
            let (bare_up, bare_down) = (bare(i, egui::Key::ArrowUp), bare(i, egui::Key::ArrowDown));
            (
                // Before the plain one, which would match it too.
                i.consume_key(cmd | egui::Modifiers::SHIFT, egui::Key::Enter),
                i.consume_key(cmd, egui::Key::Enter),
                walking && bare_up && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                walking && bare_down && self.back.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(cmd, egui::Key::I) || (!menu_open && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)),
            )
        });
        // Ctrl+V: egui-winit takes the V key's press as a paste and passes
        // on only its release, with no text when the clipboard holds an
        // image -- and Ctrl may be let go first. So a V let go whose press
        // never came, with no text pasted, asks the window for the image.
        if focused {
            let (pasted, v_down, v_up) = ui.input(|i| {
                let key = |down: bool| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::V, pressed, .. } if *pressed == down));
                (i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))), key(true), key(false))
            });
            if v_down {
                self.v_down = true;
            }
            if v_up && !self.v_down && !pasted && !self.pasted_text_lately {
                self.image_asked = true;
            }
            if v_up {
                self.v_down = false;
            }
            self.pasted_text_lately = pasted;
        }
        if up {
            self.older(&mut draft);
        }
        if down {
            self.newer(&mut draft);
        }
        if close || (esc_for_box && !menu_open) {
            self.open = false;
            // Back to the pane.
            ui.ctx().memory_mut(|m| m.surrender_focus(id));
        }

        let frame = egui::Frame::NONE
            .fill(c.side)
            .stroke(egui::Stroke::new(1.0, if focused { c.run } else { c.border_strong() }))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(12, 8));
        let shown = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Where it goes: the pane, or tags instead.
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("inputbox.to")).size(12.0).color(c.dim));
                let pane_on = self.to_tags.is_empty();
                if ui.selectable_label(pane_on, RichText::new(name).size(12.0)).on_hover_text(tr("inputbox.to_hover")).clicked() {
                    self.to_tags.clear();
                }
                // Others picked to get the same prompt, each taken off by a click.
                if pane_on {
                    let mut gone = None;
                    for s in &self.to_also {
                        let n = others.iter().find(|(o, _)| o == s).map_or("", |(_, n)| n.as_str());
                        if ui.selectable_label(true, RichText::new(format!("+ {n}")).size(12.0)).on_hover_text(tr("inputbox.also_hover")).clicked() {
                            gone = Some(*s);
                        }
                    }
                    if let Some(g) = gone {
                        self.to_also.retain(|s| *s != g);
                    }
                }
                let rest: Vec<&(SessionId, String)> = others.iter().filter(|(o, _)| *o != to).collect();
                if !rest.is_empty() {
                    let menu = stay_open(RichText::new(tr("inputbox.sessions")).size(12.0).color(c.dim)).ui(ui, |ui| {
                        ui.set_min_width(220.0);
                        ui.label(RichText::new(tr("inputbox.send_to")).size(11.5).color(c.dim));
                        for (o, n) in &rest {
                            let mut on = self.to_also.contains(o);
                            if ui.checkbox(&mut on, n.as_str()).changed() {
                                self.to_tags.clear();
                                if on {
                                    self.to_also.push(*o);
                                } else {
                                    self.to_also.retain(|s| s != o);
                                }
                            }
                        }
                        ui.separator();
                        if ui.button(tr("inputbox.every")).clicked() {
                            self.to_tags.clear();
                            self.to_also = rest.iter().map(|(o, _)| *o).collect();
                            ui.close();
                        }
                        if !self.to_also.is_empty() && ui.button(tr("inputbox.only_this")).clicked() {
                            self.to_also.clear();
                            ui.close();
                        }
                    });
                    // The keys stay with the box (a click on the menu took
                    // them): an Esc to close it must not reach the shell.
                    if menu.0.clicked() {
                        self.focus = true;
                    }
                    menus_open |= menu.1.is_some();
                    menu.0.on_hover_text(tr("inputbox.sessions_hint"));
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
                    if resp.on_hover_text(trf("inputbox.tag_hover", &[t])).clicked() {
                        if on {
                            self.to_tags.retain(|x| x != t);
                        } else {
                            self.to_tags.push(t.clone());
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Prompts kept: one picked goes into the draft.
                    let mut picked = false;
                    let menu = stay_open(RichText::new(tr("inputbox.prompts")).size(12.0).color(c.dim)).ui(ui, |ui| {
                        ui.set_min_width(260.0);
                        if self.prompts.is_empty() {
                            ui.label(RichText::new(tr("inputbox.none_kept")).size(11.5).color(c.dim));
                        }
                        let mut gone = None;
                        for (k, p) in self.prompts.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.small_button("×").on_hover_text(tr("inputbox.forget")).clicked() {
                                    gone = Some(k);
                                }
                                if ui.button(&p.name).on_hover_text(&p.text).clicked() {
                                    if draft.text.trim().is_empty() {
                                        draft.text.clone_from(&p.text);
                                    } else {
                                        draft.text = format!("{}\n{}", draft.text.trim_end(), p.text);
                                    }
                                    picked = true;
                                    ui.close();
                                }
                            });
                        }
                        if let Some(k) = gone {
                            self.prompts.remove(k);
                            self.prompts_changed = true;
                        }
                        ui.separator();
                        if draft.text.trim().is_empty() {
                            ui.label(RichText::new(tr("inputbox.placeholders")).size(11.0).color(c.faint()));
                        } else {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.naming).hint_text(tr("inputbox.name_hint")).desired_width(160.0));
                                if ui.add_enabled(!self.naming.trim().is_empty(), egui::Button::new(tr("inputbox.save"))).clicked() {
                                    let name = std::mem::take(&mut self.naming).trim().to_owned();
                                    self.prompts.retain(|p| p.name != name);
                                    self.prompts.push(crate::prompts::Prompt { name, text: draft.text.trim_end().to_owned() });
                                    self.prompts_changed = true;
                                    ui.close();
                                }
                            });
                        }
                    });
                    // Back to the box to go on writing, or to send.
                    if menu.0.clicked() || picked {
                        self.focus = true;
                    }
                    menus_open |= menu.1.is_some();
                    if queued > 0 {
                        ui.label(RichText::new(trf("inputbox.queued", &[&queued.to_string()])).size(11.5).color(c.wait)).on_hover_text(tr("inputbox.queued_hover"));
                    }
                    // The design's: every session keeps its draft.
                    ui.label(RichText::new(tr("inputbox.draft")).size(11.5).color(c.faint())).on_hover_text(tr("inputbox.draft_hover"));
                });
            });
            // A line under where it goes (the design's).
            let (line, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 7.0), egui::Sense::hover());
            ui.painter().hline(line.x_range().expand(12.0), line.center().y, egui::Stroke::new(1.0, c.border));
            let edit = egui::TextEdit::multiline(&mut draft.text)
                .id(id)
                .hint_text(trf("inputbox.hint", &[if cfg!(target_os = "macos") { "Cmd+Enter" } else { "Ctrl+Enter" }]))
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
                    // An image's mark or a file's, and its size when known.
                    let image = f.extension().and_then(|e| e.to_str()).is_some_and(|e| ["png", "jpg", "jpeg", "gif", "webp", "bmp"].contains(&e.to_ascii_lowercase().as_str()));
                    let mark = if image { "▣" } else { "▤" };
                    let size = draft.sizes.get(f).map(|b| format!(" · {}", size_words(*b))).unwrap_or_default();
                    let chip = egui::Button::new(RichText::new(format!("{mark} {name}{size}  ×")).size(12.0)).fill(c.panel).stroke(egui::Stroke::new(1.0, c.border));
                    if ui.add(chip).on_hover_text(format!("{}: click to take it off", f.display())).clicked() {
                        gone = Some(k);
                    }
                }
                if let Some(k) = gone {
                    draft.files.remove(k);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let key = if cfg!(target_os = "macos") { "Cmd+Enter" } else { "Ctrl+Enter" };
                    let label = if self.later { tr("inputbox.queue") } else { tr("inputbox.send") };
                    let button = egui::Button::new(RichText::new(format!("{label}   {key}")).color(c.on_accent()).strong()).fill(if self.later { c.wait } else { c.run }).min_size(egui::vec2(110.0, 28.0));
                    let empty = draft.text.trim().is_empty() && draft.files.is_empty();
                    let clicked = ui.add_enabled(!empty, button).clicked();
                    let shift = if cfg!(target_os = "macos") { "Cmd+Shift+Enter" } else { "Ctrl+Shift+Enter" };
                    ui.toggle_value(&mut self.later, RichText::new(tr("inputbox.when_done")).size(12.0)).on_hover_text(trf("inputbox.when_done_hover", &[shift]));
                    if (clicked || send || later_key) && !empty {
                        let text = Self::prompt(&draft);
                        let to = if !self.to_tags.is_empty() {
                            To::Tags(self.to_tags.clone())
                        } else if self.to_also.is_empty() {
                            To::Session(to)
                        } else {
                            To::Sessions(std::iter::once(to).chain(self.to_also.iter().copied()).collect())
                        };
                        sent = Some(Send { to, text: text.clone(), later: self.later || later_key });
                        self.remember(draft.text.trim_end());
                        self.back = None;
                        draft = Draft::default();
                    }
                    let paste = if cfg!(target_os = "macos") { "Cmd+V" } else { "Ctrl+V" };
                    ui.label(RichText::new(trf("inputbox.foot", &[paste])).size(11.5).color(c.faint()));
                });
            });
        });
        if self.menu_was_open && !menus_open {
            self.focus = true;
        }
        self.menu_was_open = menus_open;
        // An Esc for one of its menus leaves the keys with the box.
        if esc_for_box && menu_open {
            self.focus = true;
        }
        let keeps = self.open && (ui.ctx().memory(|m| m.has_focus(id)) || self.focus);
        self.had_keys_at = keeps.then(|| ui.ctx().cumulative_frame_nr());
        // Picked out by a glow while it has the keys (the design's).
        if focused {
            let r = shown.response.rect;
            ui.painter().rect_stroke(r.expand(2.0), 12.0, egui::Stroke::new(4.0, c.run.gamma_multiply(0.18)), egui::StrokeKind::Outside);
        }
        if draft != Draft::default() {
            self.drafts.insert(to, draft);
        }
        sent
    }
}

/// A menu button whose menu stays open while it is used (a field typed in,
/// several ticks), closing on a click outside it or on Esc.
fn stay_open(text: RichText) -> egui::containers::menu::MenuButton<'static> {
    use egui::containers::menu::{MenuButton, MenuConfig};
    MenuButton::new(text).config(MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
}

/// `214 KB`, `1.2 MB`.
pub fn size_words(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{} KB", bytes / 1024),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_said_as_people_do() {
        assert_eq!(size_words(512), "512 B");
        assert_eq!(size_words(219_136), "214 KB");
        assert_eq!(size_words(1_258_291), "1.2 MB");
    }

    #[test]
    fn the_prompt_carries_the_files() {
        let d = Draft { text: "look at these\n".into(), files: vec!["/tmp/a.png".into(), "/tmp/my shot.png".into()], ..Draft::default() };
        assert_eq!(InputBox::prompt(&d), "look at these\n/tmp/a.png \"/tmp/my shot.png\"");
        let only = Draft { text: " ".into(), files: vec!["/tmp/a.png".into()], ..Draft::default() };
        assert_eq!(InputBox::prompt(&only), "/tmp/a.png");
    }

    #[test]
    fn the_history_goes_back_and_forth() {
        let mut b = InputBox::default();
        for p in ["one", "two", "two", ""] {
            b.remember(p);
        }
        assert_eq!(b.history, ["one", "two"], "the same twice or nothing is kept once");
        let mut d = Draft { text: "draft".into(), ..Draft::default() };
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
