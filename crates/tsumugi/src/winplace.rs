//! Where the window was and how big: ito's `window.toml` in the state folder
//! (`ito_common::window::Saved`), so the next start opens there. Only the
//! first window does this (not one torn off a tab, and not in quake mode,
//! which places the window itself). ito reads the file (a place no screen
//! shows any more is not used, and tsumugi's own file from before is read
//! too), and its `Tracker` follows the window and writes it, as it does for
//! kura and yagura.

use std::path::PathBuf;

/// The folder `window.toml` is in: the state folder.
pub fn dir() -> Option<PathBuf> {
    Some(tsumugi_mux::state::default_path()?.parent()?.to_path_buf())
}

/// Whether this window is the one that keeps its place.
pub fn wanted(quake: &str) -> bool {
    quake.is_empty() && std::env::var_os(crate::menu::SHOW_TAB).is_none() && std::env::var_os(crate::menu::WINDOW_AT).is_none()
}
