//! Ask before a paste that could run more than was meant (Windows Terminal's
//! and iTerm2's warning): several lines into a program that did not ask for
//! bracketed paste, where each line break is an Enter, or a very long text.
//! Claude Code and the shells that ask for bracketed paste get several lines
//! without a word, as a paste there is only text until Enter is pressed.

use egui::RichText;
use tsumugi_mux::SessionId;
use tsumugi_mux::settings::{General, LARGE_PASTE};

use crate::chrome;
use crate::theme::Colors;

/// Why a paste is held.
#[derive(Debug, PartialEq)]
pub enum Why {
    /// This many lines, each run as it lands.
    Lines(usize),
    /// This many bytes.
    Large(usize),
}

/// Why `text` is asked about, if it is: `bracketed` the panes it goes to
/// asked for bracketed paste.
pub fn why(text: &str, bracketed: bool, g: &General) -> Option<Why> {
    if g.warn_large_paste && text.len() >= LARGE_PASTE {
        return Some(Why::Large(text.len()));
    }
    // One trailing line break is the usual end of a copied line; it runs
    // the line, but the one line is what was meant.
    let body = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).or_else(|| text.strip_suffix('\r')).unwrap_or(text);
    let lines = body.replace("\r\n", "\n").split(['\n', '\r']).count();
    (g.warn_multiline_paste && !bracketed && lines >= 2).then_some(Why::Lines(lines))
}

/// A paste waiting for its answer.
pub struct Held {
    /// The panes it goes to: the focus, and the tab's others when typing
    /// into all of them.
    pub to: Vec<SessionId>,
    pub text: String,
    pub why: Why,
}

/// The first lines of the text, as shown in the dialog.
fn preview(text: &str) -> String {
    const LINES: usize = 5;
    const WIDTH: usize = 80;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines.iter().take(LINES).map(|l| if l.chars().count() > WIDTH { format!("{}…", l.chars().take(WIDTH).collect::<String>()) } else { l.to_string() }).collect();
    if lines.len() > LINES {
        out.push(format!("… {} more lines", lines.len() - LINES));
    }
    out.join("\n")
}

/// Draw the dialog: `Some(true)` paste, `Some(false)` drop it.
pub fn show(ctx: &egui::Context, held: &Held, c: &Colors) -> Option<bool> {
    let mut answer = None;
    ctx.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
            answer = Some(true);
        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
            answer = Some(false);
        }
    });
    egui::Area::new(egui::Id::new("paste-ask")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(20, 16)).show(ui, |ui| {
            ui.set_max_width(520.0);
            let (title, words) = match held.why {
                Why::Lines(n) => (format!("Paste {n} lines?"), "The program here did not ask for bracketed paste, so each line runs as it lands."),
                Why::Large(n) => (format!("Paste {} KB?", n.div_ceil(1024)), "This is a long text to send to the program."),
            };
            ui.label(RichText::new(title).size(15.0).strong().color(c.strong()));
            ui.add_space(4.0);
            ui.label(RichText::new(words).size(12.5).color(c.dim));
            if held.to.len() > 1 {
                ui.label(RichText::new(format!("It goes to all {} panes of the tab.", held.to.len())).size(12.5).color(c.dim));
            }
            ui.add_space(8.0);
            egui::Frame::NONE.fill(c.bg).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(preview(&held.text)).monospace().size(12.0).color(c.fg));
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Turn this off in Settings, General, Copy and paste.").size(11.5).color(c.dim));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let paste = egui::Button::new(RichText::new("Paste").color(c.on_accent()).strong()).fill(chrome::gold());
                let (paste, cancel) = chrome::foot(ui, paste, true, "Cancel");
                if paste.clicked() {
                    answer = Some(true);
                }
                if cancel.clicked() {
                    answer = Some(false);
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
    fn several_lines_are_asked_about_unless_bracketed() {
        let g = General::default();
        assert_eq!(why("ls -l", false, &g), None);
        assert_eq!(why("ls -l\n", false, &g), None, "one copied line with its end");
        assert_eq!(why("ls -l\r\n", false, &g), None);
        assert_eq!(why("cd /\nrm -rf x\n", false, &g), Some(Why::Lines(2)));
        assert_eq!(why("a\r\nb\r\nc", false, &g), Some(Why::Lines(3)), "CRLF is one break");
        assert_eq!(why("cd /\nrm -rf x\n", true, &g), None, "bracketed: only text until Enter");
        let off = General { warn_multiline_paste: false, ..General::default() };
        assert_eq!(why("a\nb", false, &off), None);
    }

    #[test]
    fn a_long_paste_is_asked_about_even_bracketed() {
        let g = General::default();
        let long = "x".repeat(LARGE_PASTE);
        assert_eq!(why(&long, true, &g), Some(Why::Large(LARGE_PASTE)));
        assert_eq!(why(&long[1..], true, &g), None);
        let off = General { warn_large_paste: false, ..General::default() };
        assert_eq!(why(&long, true, &off), None);
    }

    #[test]
    fn the_preview_is_the_first_lines() {
        assert_eq!(preview("a\nb"), "a\nb");
        let many = (1..=8).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(preview(&many), "1\n2\n3\n4\n5\n… 3 more lines");
        assert_eq!(preview(&"y".repeat(100)).chars().count(), 81);
    }
}
