//! Where the window was and how big: ito's `window.toml` in the state folder
//! (`ito_common::window`), so the next start opens there. Only the first
//! window does this (not one torn off a tab, and not in quake mode, which
//! places the window itself), and a place that no screen shows any more is
//! not used. ito's `Tracker` follows the window and writes the file, as it
//! does for kura and yagura.

use std::path::{Path, PathBuf};

use ito_common::window::Window;

/// The primary screen where the OS is not asked for its screens.
const PRIMARY_GUESS: (f32, f32) = (1920.0, 1080.0);

/// The folder `window.toml` is in: the state folder.
pub fn dir() -> Option<PathBuf> {
    Some(tsumugi_mux::state::default_path()?.parent()?.to_path_buf())
}

/// Whether this window is the one that keeps its place.
pub fn wanted(quake: &str) -> bool {
    quake.is_empty() && std::env::var_os(crate::menu::SHOW_TAB).is_none() && std::env::var_os(crate::menu::WINDOW_AT).is_none()
}

/// The saved place, if there is one and a screen shows it. A file written
/// before tsumugi used ito's is read too.
pub fn load(dir: &Path) -> Option<Window> {
    read(dir, zoom).filter(|w| w.reachable_now(PRIMARY_GUESS, 100.0))
}

/// The place in `dir`, in ito's form or the one from before. One from
/// before is written again in ito's form, with the zoom it is read at
/// (`zoom`): ito's tracker writes only a place that changed, so it would
/// stay as it is, and a later zoom would size it anew.
fn read(dir: &Path, zoom: impl FnOnce() -> f32) -> Option<Window> {
    ito_common::window::read(dir).or_else(|| {
        let w = old(&std::fs::read_to_string(ito_common::window::path(dir)).ok()?, zoom())?;
        let _ = ito_common::window::write(dir, &w);
        Some(w)
    })
}

/// common.toml's scale: the zoom tsumugi sets once it has read the file.
fn zoom() -> f32 {
    ito_i18n::base_dir().and_then(|base| ito_common::Common::read(&base).ok()).map_or(1.0, |(common, _)| common.scale_or_one())
}

/// A `window.toml` from before (v0.113.1 and older): the corner and the
/// inner size (`width`, `height`) in points at `scale` points to pixels,
/// the zoom in both. The zoom itself was not kept; `zoom` is taken for it.
fn old(text: &str, zoom: f32) -> Option<Window> {
    let t: toml::Table = text.parse().ok()?;
    let num = |k: &str| t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|f| f as f32);
    let w = Window {
        x: num("x")?,
        y: num("y")?,
        w: num("width")?,
        h: num("height")?,
        maximized: t.get("maximized").and_then(toml::Value::as_bool).unwrap_or(false),
        // Without one, at the primary screen's, as ito reads such a place.
        scale: num("scale").filter(|s| *s > 0.0).unwrap_or(0.0),
        zoom,
    };
    w.sane().then_some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_from_before_is_read() {
        // tsumugi #176: closed on a 100% screen right of a 150% primary.
        let text = "x = 4493\ny = 280\nwidth = 1102\nheight = 721\nscale = 1\nmaximized = false\n";
        let w = old(text, 1.0).unwrap();
        assert_eq!(w, Window { x: 4493.0, y: 280.0, w: 1102.0, h: 721.0, maximized: false, scale: 1.0, zoom: 1.0 });
        assert_eq!(w.opening().size, [1102.0, 721.0]);
        // At common.toml's 125%: the window is made that much bigger, as it
        // was seen, not 125% smaller.
        let zoomed = old("x = 100\ny = 80\nwidth = 1000\nheight = 640\nscale = 1.875\nmaximized = true\n", 1.25).unwrap();
        assert_eq!((zoomed.scale, zoomed.zoom, zoomed.maximized), (1.875, 1.25, true));
        assert_eq!(zoomed.opening().size, [1250.0, 800.0]);
        // No scale: read at the primary screen's.
        assert_eq!(old("x = 1\ny = 2\nwidth = 900\nheight = 700\n", 1.0).map(|w| w.scale), Some(0.0));
    }

    #[test]
    fn a_bad_file_from_before_gives_nothing() {
        assert_eq!(old("x = 1\n", 1.0), None);
        assert_eq!(old("not toml [", 1.0), None);
        assert_eq!(old("x = 1\ny = 2\nwidth = \"wide\"\nheight = 5\n", 1.0), None);
        // A minimized window's parking spot, or a size no window has.
        assert_eq!(old("x = -32000\ny = -32000\nwidth = 900\nheight = 700\n", 1.0), None);
        assert_eq!(old("x = 1\ny = 2\nwidth = 50\nheight = 50\n", 1.0), None);
    }

    #[test]
    fn ito_s_file_is_not_read_as_one_from_before() {
        let dir = std::env::temp_dir().join(format!("tsumugi-winplace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let w = Window { x: 4493.0, y: 280.0, w: 1102.0, h: 721.0, maximized: false, scale: 1.0, zoom: 1.25 };
        ito_common::window::write(&dir, &w).unwrap();
        assert_eq!(ito_common::window::read(&dir), Some(w));
        assert_eq!(old(&std::fs::read_to_string(ito_common::window::path(&dir)).unwrap(), 1.0), None);
        // And the one from before is not ito's.
        std::fs::write(ito_common::window::path(&dir), "x = 4493\ny = 280\nwidth = 1102\nheight = 721\nscale = 1\n").unwrap();
        assert_eq!(ito_common::window::read(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_from_before_is_written_again_in_ito_s_form() {
        let dir = std::env::temp_dir().join(format!("tsumugi-winplace-again-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read(&dir, || 1.0), None, "no file");
        std::fs::write(ito_common::window::path(&dir), "x = 3456\ny = 215\nwidth = 846\nheight = 554\nscale = 1.3\n").unwrap();
        let w = Window { x: 3456.0, y: 215.0, w: 846.0, h: 554.0, maximized: false, scale: 1.3, zoom: 1.3 };
        assert_eq!(read(&dir, || 1.3), Some(w));
        assert_eq!(ito_common::window::read(&dir), Some(w), "written in ito's form");
        // From then on the zoom it was read at is kept.
        assert_eq!(read(&dir, || 1.0), Some(w));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
