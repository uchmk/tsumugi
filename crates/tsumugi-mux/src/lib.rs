//! The tsumugi server, which holds the sessions, and the client a window (or
//! the CLI) talks to it with. Closing the window, or the window crashing,
//! leaves the shells running; the next window attaches to them again.
//!
//! The design is `docs/mux-design.md`. Over a named pipe on Windows and a Unix
//! domain socket elsewhere (`transport`), each message is a length and a
//! postcard body (`frame`) of the types in `proto`.

pub mod client;
pub mod diff;
pub mod frame;
pub mod proto;
pub mod server;
pub mod transport;

#[cfg(test)]
mod tests;

pub use client::{Client, RemotePane};
pub use proto::{Dir, Info, Node, Place, SessionId, State, Workspace, WorkspaceId};
pub use transport::Address;
