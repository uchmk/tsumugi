//! Quake mode (`[window] quake`): a key the system keeps for tsumugi, so it
//! works from any program. It brings the window down from the top of the
//! screen, and sends it away again when it has the keys.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// Pressed since the window last looked; the system's handler is set once.
static PRESSED: AtomicBool = AtomicBool::new(false);
static REPAINT: OnceLock<egui::Context> = OnceLock::new();

#[derive(Default)]
pub struct Quake {
    /// Made on the first key asked for, on the window's thread (Windows and
    /// macOS want their own).
    manager: Option<GlobalHotKeyManager>,
    key: Option<HotKey>,
    /// The setting it follows.
    word: String,
}

impl Quake {
    /// Follow the setting: let the old key go and take the new one. Why it
    /// could not, to say once.
    pub fn follow(&mut self, ctx: &egui::Context, word: &str) -> Option<String> {
        if self.word == word {
            return None;
        }
        self.word = word.to_owned();
        if let (Some(m), Some(k)) = (&self.manager, self.key.take()) {
            let _ = m.unregister(k);
        }
        let word = word.trim();
        if word.is_empty() {
            return None;
        }
        let key = match parse(word) {
            Ok(k) => k,
            Err(e) => return Some(e),
        };
        if self.manager.is_none() {
            match GlobalHotKeyManager::new() {
                Ok(m) => self.manager = Some(m),
                Err(e) => return Some(format!("Quake mode: the system's keys cannot be reached ({e})")),
            }
            let _ = REPAINT.set(ctx.clone());
            GlobalHotKeyEvent::set_event_handler(Some(|e: GlobalHotKeyEvent| {
                if e.state == HotKeyState::Pressed {
                    PRESSED.store(true, Ordering::SeqCst);
                    if let Some(ctx) = REPAINT.get() {
                        ctx.request_repaint();
                    }
                }
            }));
        }
        match self.manager.as_ref().map(|m| m.register(key)) {
            Some(Ok(())) => {
                self.key = Some(key);
                None
            }
            Some(Err(e)) => Some(format!("Quake mode: {word} could not be taken ({e})")),
            None => None,
        }
    }

    /// The key was pressed since the last look.
    pub fn pressed(&self) -> bool {
        self.key.is_some() && PRESSED.swap(false, Ordering::SeqCst)
    }
}

/// `Ctrl+`` `, `Cmd+Shift+Space`, `F12`: the keys' own way of writing them.
pub fn parse(word: &str) -> Result<HotKey, String> {
    word.trim().parse::<HotKey>().map_err(|e| format!("window.quake: `{}` is not a key ({e})", word.trim()))
}

/// Where the window comes down to: the screen's width, `height` % of it,
/// from its top.
pub fn drop_rect(monitor: egui::Vec2, height: u8) -> egui::Rect {
    let h = (monitor.y * f32::from(height.clamp(20, 100)) / 100.0).round();
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(monitor.x, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_written_as_the_window_writes_them() {
        assert!(parse("Ctrl+`").is_ok());
        assert!(parse("Ctrl+Shift+Space").is_ok());
        assert!(parse("F12").is_ok());
        assert!(parse("Ctrl+Nothing").unwrap_err().contains("window.quake"));
    }

    #[test]
    fn it_comes_down_from_the_top() {
        let r = drop_rect(egui::vec2(1920.0, 1080.0), 40);
        assert_eq!((r.min, r.size()), (egui::Pos2::ZERO, egui::vec2(1920.0, 432.0)));
        assert_eq!(drop_rect(egui::vec2(100.0, 100.0), 5).height(), 20.0);
    }
}
