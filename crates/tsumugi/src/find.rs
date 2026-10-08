//! Find in the pane (`Ctrl+Shift+F`, Windows Terminal's): a bar at the top
//! right of the pane with the keys. Typing finds from the newest line back;
//! Enter goes on to the older match, Shift+Enter to the newer, Esc closes.
//! The server finds and selects; this only asks and shows its answer.

use egui::RichText;
use tsumugi_mux::SessionId;

use crate::theme::Colors;

/// The bar while it is open.
pub struct Bar {
    /// The pane it finds in.
    pub id: SessionId,
    pub needle: String,
    /// The last answer: `None` nothing matched, `Some(true)` it ran off the
    /// end and started again. Unset until one comes.
    pub said: Option<Option<bool>>,
    /// Give the field the keys this frame (opened, or its key pressed again).
    pub focus: bool,
    /// What was last asked for, so a change starts a new search.
    asked: String,
    /// The field had the keys at the end of the last frame. egui drops the
    /// focus on Esc before the bar is drawn, so its own word is too late.
    keyed: bool,
}

impl Bar {
    pub fn new(id: SessionId) -> Self {
        Self { id, needle: String::new(), said: None, focus: true, asked: String::new(), keyed: false }
    }

    /// The field has the keys: none of them go to the pane, Esc included,
    /// which egui takes the focus away on before the bar reads it.
    pub fn keyed(&self) -> bool {
        self.keyed || self.focus
    }
}

/// What the bar asks of the pane.
#[derive(Debug, PartialEq)]
pub enum Ask {
    /// Find from the last match on: `back` toward the older lines.
    Find { needle: String, back: bool },
    /// Forget the search and start again from the view (the text changed).
    Restart,
    /// Close the bar, the search forgotten and its selection dropped.
    Close,
}

/// The asks for a frame's keys and text: `step` is Enter (`Some(false)`
/// with Shift), the buttons the same; `close` Esc or ×.
pub fn asks(bar: &mut Bar, step: Option<bool>, close: bool) -> Vec<Ask> {
    if close {
        return vec![Ask::Close];
    }
    let mut out = Vec::new();
    if bar.needle != bar.asked {
        bar.asked = bar.needle.clone();
        bar.said = None;
        out.push(Ask::Restart);
        if !bar.needle.is_empty() {
            // Typing finds the newest match first, as a step back from the
            // bottom of the view.
            out.push(Ask::Find { needle: bar.needle.clone(), back: true });
        }
        return out;
    }
    if let Some(back) = step.filter(|_| !bar.needle.is_empty()) {
        out.push(Ask::Find { needle: bar.needle.clone(), back });
    }
    out
}

/// Draw the bar over the top right of `pane` and read its keys.
pub fn show(ctx: &egui::Context, pane: egui::Rect, bar: &mut Bar, c: &Colors) -> Vec<Ask> {
    let field_id = egui::Id::new(("find-field", bar.id));
    // Taken before the field sees them: a single-line field gives up the
    // keys on Enter, and Esc would only leave it.
    let (mut step, mut close) = (None, false);
    if bar.keyed {
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter) {
                step = Some(false);
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                step = Some(true);
            }
            close = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        });
    }
    let width = 330.0_f32.min(pane.width() - 16.0).max(160.0);
    let at = egui::pos2(pane.right() - width - 10.0, pane.top() + 6.0);
    egui::Area::new(egui::Id::new(("find-bar", bar.id))).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
            ui.set_width(width - 16.0);
            ui.horizontal(|ui| {
                let words = match bar.said {
                    Some(None) => RichText::new("No match").color(c.err),
                    Some(Some(true)) => RichText::new("From the end again").color(c.dim),
                    _ => RichText::new(""),
                };
                let room = ui.available_width() - 3.0 * 24.0 - 96.0;
                let field = ui.add(egui::TextEdit::singleline(&mut bar.needle).id(field_id).hint_text("Find in the pane").desired_width(room.max(60.0)));
                if std::mem::take(&mut bar.focus) || (bar.keyed && step.is_some()) {
                    field.request_focus();
                }
                bar.keyed = field.has_focus() || ctx.memory(|m| m.has_focus(field_id));
                ui.add_sized(egui::vec2(96.0, 18.0), egui::Label::new(words.size(11.5)).truncate());
                if ui.small_button("↑").on_hover_text("Older match (Enter)").clicked() {
                    step = Some(true);
                }
                if ui.small_button("↓").on_hover_text("Newer match (Shift+Enter)").clicked() {
                    step = Some(false);
                }
                if ui.small_button("×").on_hover_text("Close (Esc)").clicked() {
                    close = true;
                }
            });
        });
    });
    asks(bar, step, close)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_starts_again_and_enter_walks_on() {
        let mut bar = Bar::new(1);
        assert_eq!(asks(&mut bar, None, false), []);
        bar.needle = "err".into();
        assert_eq!(asks(&mut bar, None, false), [Ask::Restart, Ask::Find { needle: "err".into(), back: true }]);
        assert_eq!(asks(&mut bar, None, false), [], "asked once");
        bar.said = Some(Some(false));
        assert_eq!(asks(&mut bar, Some(true), false), [Ask::Find { needle: "err".into(), back: true }]);
        assert_eq!(asks(&mut bar, Some(false), false), [Ask::Find { needle: "err".into(), back: false }]);
        bar.needle = "error".into();
        assert_eq!(asks(&mut bar, Some(true), false), [Ask::Restart, Ask::Find { needle: "error".into(), back: true }], "a change starts over");
        assert_eq!(bar.said, None, "the old answer goes with it");
        bar.needle.clear();
        assert_eq!(asks(&mut bar, None, false), [Ask::Restart]);
        assert_eq!(asks(&mut bar, Some(true), false), [], "nothing to find");
        assert_eq!(asks(&mut bar, None, true), [Ask::Close]);
    }

    /// Enter and Shift+Enter in the field step, and do not leave it.
    #[test]
    fn enter_in_the_field_steps() {
        let ctx = egui::Context::default();
        let mut bar = Bar::new(7);
        bar.needle = "x".into();
        let pane = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let frame = |ctx: &egui::Context, bar: &mut Bar, events: Vec<egui::Event>| {
            let mut got = Vec::new();
            let input = egui::RawInput { screen_rect: Some(pane), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| got = show(ui.ctx(), pane, bar, &crate::theme::colors()));
            out.textures_delta.clear();
            got
        };
        assert_eq!(frame(&ctx, &mut bar, vec![]).len(), 2, "the typed text is asked for");
        let key = |key, shift| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE } };
        assert_eq!(frame(&ctx, &mut bar, vec![key(egui::Key::Enter, false)]), [Ask::Find { needle: "x".into(), back: true }]);
        assert_eq!(frame(&ctx, &mut bar, vec![key(egui::Key::Enter, true)]), [Ask::Find { needle: "x".into(), back: false }]);
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new(("find-field", 7_u64)))), "the field keeps the keys");
        assert_eq!(frame(&ctx, &mut bar, vec![key(egui::Key::Escape, false)]), [Ask::Close]);
    }
}
