//! Other machines' tsumugi servers, reached over ssh (TODO's Q15): the
//! window shows one machine's sessions at a time and keeps a line to each
//! of the others, so the sidebar can say how they are doing.

use std::sync::mpsc::Receiver;

use tsumugi_mux::{Client, Info, State};

use crate::newsession;

/// What to do once a machine answers.
pub enum Then {
    /// Show its sessions.
    Show,
    /// Show it and start what the new-session dialog asked for.
    Create(newsession::Create),
}

/// How the line to a machine is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Connecting,
    Up,
    /// Dropped, or never made: the row says why and offers "Reconnect".
    Lost,
}

pub struct Machine {
    /// As `~/.ssh/config` names it.
    pub host: String,
    pub client: Option<Client>,
    connecting: Option<Receiver<std::io::Result<Client>>>,
    pub error: Option<String>,
    pub then: Option<Then>,
    /// How many of its sessions waited when last looked: more, while it is
    /// not shown, is told.
    pub waited: usize,
}

impl Machine {
    pub fn new(host: String) -> Self {
        Self { host, client: None, connecting: None, error: None, then: None, waited: 0 }
    }

    /// Reach the machine on a thread: ssh can take seconds, or not answer.
    /// `command` runs tsumugi there (the settings' `[remote]`); `then`
    /// is `None` to keep the line without showing it (a window opening).
    pub fn connect(&mut self, ctx: &eframe::egui::Context, command: String, then: Option<Then>) {
        self.start(ctx, command, then, false);
    }

    /// Stop its server, then reach the one that starts in its place: one
    /// of another version, after tsumugi was updated there.
    pub fn restart(&mut self, ctx: &eframe::egui::Context, command: String, then: Option<Then>) {
        self.start(ctx, command, then, true);
    }

    fn start(&mut self, ctx: &eframe::egui::Context, command: String, then: Option<Then>, restart: bool) {
        self.then = then;
        self.error = None;
        if self.connecting.is_some() {
            return;
        }
        self.client = None;
        let (tx, rx) = std::sync::mpsc::channel();
        let (host, ctx) = (self.host.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name("machine".into()).spawn(move || {
            let reach = || {
                let wake = ctx.clone();
                Client::over_ssh(&host, &command, move || wake.request_repaint())
            };
            let got = match restart {
                false => reach(),
                true => Client::stop_over_ssh(&host, &command).and_then(|()| {
                    // The old server takes a moment to go; until then it
                    // answers as before.
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
                    loop {
                        match reach() {
                            Err(e) if gone_soon(e.kind()) && std::time::Instant::now() < deadline => {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                            }
                            done => break done,
                        }
                    }
                }),
            };
            // The sessions it has, before it is shown: the first list is
            // asked for, not waited on with the line still quiet.
            let got = got.and_then(|c| c.list().map(|_| c));
            let _ = tx.send(got);
            ctx.request_repaint();
        });
        self.connecting = Some(rx);
    }

    /// The answer to `connect`, once it has come: true when the machine is
    /// now up.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.connecting else { return false };
        match rx.try_recv() {
            Ok(Ok(c)) => {
                self.connecting = None;
                self.client = Some(c);
                true
            }
            Ok(Err(e)) => {
                self.connecting = None;
                self.error = Some(e.to_string());
                self.then = None;
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.connecting = None;
                self.error = Some("the connection thread stopped".into());
                self.then = None;
                false
            }
        }
    }

    pub fn link(&self) -> Link {
        match (&self.connecting, &self.client) {
            (Some(_), _) => Link::Connecting,
            (None, Some(c)) if !c.lost() => Link::Up,
            _ => Link::Lost,
        }
    }

    /// The sessions there as last told, and how many wait on someone.
    pub fn counts(&self) -> (usize, usize) {
        match &self.client {
            Some(c) if !c.lost() => counts(&c.sessions()),
            _ => (0, 0),
        }
    }

    /// Throw away what a client not shown gathers for its window.
    pub fn drain(&self) {
        if let Some(c) = &self.client {
            let _ = c.take_ended();
            let _ = c.take_texts();
            let _ = c.take_clipboard();
        }
    }
}

/// An answer from a server still on its way out after a stop: the old
/// version, or a line it closes at once.
fn gone_soon(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind::*;
    matches!(kind, InvalidData | UnexpectedEof | ConnectionReset | ConnectionAborted | BrokenPipe)
}

/// How many sessions, and how many of them wait on someone.
pub fn counts(sessions: &[Info]) -> (usize, usize) {
    (sessions.len(), sessions.iter().filter(|i| i.state == State::Waiting).count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_not_reached_is_lost_until_asked() {
        let m = Machine::new("box".into());
        assert_eq!(m.link(), Link::Lost);
        assert_eq!(m.counts(), (0, 0));
    }
}
