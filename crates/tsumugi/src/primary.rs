//! Linux's primary selection: what was selected last, which a middle-click
//! pastes (X11, and a Wayland session's XWayland). egui knows only the
//! clipboard, so arboard does it, on a thread of its own: X11 hands the
//! text over only while its owner is alive, and asking may wait on another
//! program. Elsewhere there is no such thing; the window's middle-click
//! pastes the clipboard instead.

#[cfg(target_os = "linux")]
use std::sync::mpsc::{channel, Receiver, Sender};

#[cfg(target_os = "linux")]
enum Ask {
    Set(String),
    Get,
}

/// The thread, begun the first time it is wanted.
#[derive(Default)]
pub struct Primary {
    #[cfg(target_os = "linux")]
    thread: Option<(Sender<Ask>, Receiver<String>)>,
}

#[cfg(target_os = "linux")]
impl Primary {
    fn thread(&mut self, ctx: &egui::Context) -> &(Sender<Ask>, Receiver<String>) {
        self.thread.get_or_insert_with(|| {
            let (ask, asked) = channel::<Ask>();
            let (tell, told) = channel();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                use arboard::{GetExtLinux, LinuxClipboardKind, SetExtLinux};
                // No X server (a Wayland session without XWayland): nothing
                // to do, and the asks go nowhere.
                let Ok(mut board) = arboard::Clipboard::new() else { return };
                while let Ok(first) = asked.recv() {
                    // Only the newest selection counts; a get after it reads it.
                    let (mut set, mut get) = (None, false);
                    for a in std::iter::once(first).chain(asked.try_iter()) {
                        match a {
                            Ask::Set(text) => set = Some(text),
                            Ask::Get => get = true,
                        }
                    }
                    if let Some(text) = set {
                        let _ = board.set().clipboard(LinuxClipboardKind::Primary).text(text);
                    }
                    if get {
                        if let Ok(text) = board.get().clipboard(LinuxClipboardKind::Primary).text() {
                            if tell.send(text).is_err() {
                                return;
                            }
                            ctx.request_repaint();
                        }
                    }
                }
            });
            (ask, told)
        })
    }

    /// Make `text` the primary selection.
    pub fn set(&mut self, ctx: &egui::Context, text: String) {
        let _ = self.thread(ctx).0.send(Ask::Set(text));
    }

    /// Read it; the text comes back through [`Primary::take`].
    pub fn get(&mut self, ctx: &egui::Context) {
        let _ = self.thread(ctx).0.send(Ask::Get);
    }

    /// What a [`Primary::get`] read, if it has come.
    pub fn take(&self) -> Option<String> {
        self.thread.as_ref().and_then(|(_, told)| told.try_iter().last())
    }
}

#[cfg(not(target_os = "linux"))]
impl Primary {
    pub fn set(&mut self, _: &egui::Context, _: String) {}

    pub fn get(&mut self, _: &egui::Context) {}

    pub fn take(&self) -> Option<String> {
        None
    }
}
