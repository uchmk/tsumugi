//! A tab's changes not committed, over the window: `git diff HEAD` read on
//! a thread, each file under its own heading, added lines green, removed
//! red, the hunks' heads cyan. Opened from the card's `+N −M` or the search
//! box, so what a session did can be looked at before saying yes to more.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use eframe::egui::{self, RichText};

use crate::theme::Colors;

pub struct View {
    pub title: String,
    pub cwd: PathBuf,
    rx: Option<Receiver<Result<String, String>>>,
    text: Option<Result<Vec<Line>, String>>,
    /// Opened this frame: the click that opened it is not a click outside.
    opening: bool,
}

/// One line of the diff, as it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A file's heading (its path).
    File,
    /// `index`, `---`, `+++`, mode lines: what git says about the file.
    Meta,
    Hunk,
    Added,
    Removed,
    Same,
    /// The files git does not know yet.
    New,
}

impl View {
    /// Start reading the diff of `cwd` on a thread.
    pub fn open(title: String, cwd: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = cwd.clone();
        let _ = std::thread::Builder::new().name("git-diff".into()).spawn(move || {
            let _ = tx.send(crate::gitinfo::diff(&dir));
            wake();
        });
        Self { title, cwd, rx: Some(rx), text: None, opening: true }
    }
}

/// The diff's text as lines to draw: `diff --git a/x b/x` becomes the
/// heading `x`.
pub fn lines(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut new_files = false;
    for l in text.lines() {
        let (kind, text) = if new_files {
            (Kind::New, l.strip_prefix("?? ").unwrap_or(l).to_owned())
        } else if let Some(rest) = l.strip_prefix("diff --git ") {
            let path = rest.split(" b/").last().unwrap_or(rest).to_owned();
            (Kind::File, path)
        } else if l.starts_with("new files, not added to git yet") {
            new_files = true;
            (Kind::File, l.to_owned())
        } else if l.starts_with("@@") {
            (Kind::Hunk, l.to_owned())
        } else if l.starts_with("+++") || l.starts_with("---") || l.starts_with("index ") || l.starts_with("new file mode") || l.starts_with("deleted file mode") || l.starts_with("similarity") || l.starts_with("rename ") || l.starts_with("Binary files") || l.starts_with("old mode") || l.starts_with("new mode") {
            (Kind::Meta, l.to_owned())
        } else if l.starts_with('+') {
            (Kind::Added, l.to_owned())
        } else if l.starts_with('-') {
            (Kind::Removed, l.to_owned())
        } else {
            (Kind::Same, l.to_owned())
        };
        out.push(Line { kind, text });
    }
    out
}

/// Draw it; `false` once it is closed.
pub fn show(ctx: &egui::Context, view: &mut View, c: &Colors) -> bool {
    if let Some(rx) = &view.rx {
        if let Ok(got) = rx.try_recv() {
            view.text = Some(got.map(|t| lines(&t)));
            view.rx = None;
        }
    }
    let mut open = true;
    let screen = ctx.content_rect();
    let dim = egui::Area::new(egui::Id::new("diff-dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
        ui.painter().rect_filled(r, 0.0, egui::Color32::from_black_alpha(110));
        resp
    });
    if (dim.inner.clicked() && !view.opening) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        open = false;
    }
    view.opening = false;
    let width = (screen.width() - 48.0).clamp(300.0, 1100.0);
    let height = (screen.height() - 120.0).max(200.0);
    egui::Area::new(egui::Id::new("diff")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 52.0)).show(ctx, |ui| {
        egui::Frame::NONE.fill(c.panel).stroke(egui::Stroke::new(1.0, c.border_strong())).corner_radius(12.0).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Changes · {}", view.title)).size(14.0).strong().color(c.strong()));
                ui.label(RichText::new(view.cwd.display().to_string()).monospace().size(11.0).color(c.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Close (Esc)").clicked() {
                        open = false;
                    }
                    if let Some(Ok(lines)) = &view.text {
                        let added = lines.iter().filter(|l| l.kind == Kind::Added).count();
                        let removed = lines.iter().filter(|l| l.kind == Kind::Removed).count();
                        ui.label(RichText::new(format!("−{removed}")).monospace().color(crate::chrome::ink(crate::chrome::red())));
                        ui.label(RichText::new(format!("+{added}")).monospace().color(crate::chrome::ink(crate::chrome::green())));
                    }
                });
            });
            ui.separator();
            match &view.text {
                None => {
                    ui.label(RichText::new("Reading the changes…").color(c.dim));
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(e).color(c.dim));
                }
                Some(Ok(lines)) if lines.is_empty() => {
                    ui.label(RichText::new("Nothing changed since the last commit.").color(c.dim));
                }
                Some(Ok(lines)) => {
                    let row = 16.0;
                    // As tall as the diff, up to the window's room.
                    let tall = (row * lines.len() as f32 + 8.0).min(height);
                    egui::ScrollArea::both().max_height(height).min_scrolled_height(tall).auto_shrink([false, false]).show_rows(ui, row, lines.len(), |ui, range| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for l in &lines[range] {
                            let (color, strong) = match l.kind {
                                Kind::File => (c.strong(), true),
                                Kind::Meta => (c.faint(), false),
                                Kind::Hunk => (crate::chrome::ink(crate::chrome::cyan()), false),
                                Kind::Added => (crate::chrome::ink(crate::chrome::green()), false),
                                Kind::Removed => (crate::chrome::ink(crate::chrome::red()), false),
                                Kind::Same | Kind::New => (c.fg, false),
                            };
                            let mut text = RichText::new(&l.text).monospace().size(12.0).color(color);
                            if strong {
                                text = text.strong();
                            }
                            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), row), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(text).extend());
                            });
                        }
                    });
                }
            }
        });
    });
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diff_reads_as_files_hunks_and_lines() {
        let text = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\nnew files, not added to git yet (1)\n?? notes.md\n";
        let kinds: Vec<Kind> = lines(text).iter().map(|l| l.kind).collect();
        use Kind::*;
        assert_eq!(kinds, [File, Meta, Meta, Meta, Hunk, Same, Removed, Added, File, New]);
        assert_eq!(lines(text)[0].text, "src/a.rs");
        assert_eq!(lines(text)[9].text, "notes.md");
    }
}
