//! Where the window was and how big, kept in `window.toml` in the state
//! folder, so the next start opens there. Only the first window does this
//! (not one torn off a tab, and not in quake mode, which places the window
//! itself), and a place that no screen shows any more is not used.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;

/// The smallest the window can be (as `with_min_inner_size`).
pub const MIN_SIZE: [f32; 2] = [420.0, 260.0];

/// How long the window must hold still before it is written down.
const SETTLE: Duration = Duration::from_millis(1000);

/// The window's top left and inner size in points; `scale` is the points to
/// pixels it was seen at (to check it against the screens' pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub scale: f32,
    pub maximized: bool,
}

pub fn path() -> Option<PathBuf> {
    Some(tsumugi_mux::state::default_path()?.parent()?.join("window.toml"))
}

/// Whether this window is the one that keeps its place.
pub fn wanted(quake: &str) -> bool {
    quake.is_empty() && std::env::var_os(crate::menu::SHOW_TAB).is_none() && std::env::var_os(crate::menu::WINDOW_AT).is_none()
}

fn render(p: &Place) -> String {
    format!(
        "x = {}\ny = {}\nwidth = {}\nheight = {}\nscale = {}\nmaximized = {}\n",
        p.pos[0].round(),
        p.pos[1].round(),
        p.size[0].round(),
        p.size[1].round(),
        p.scale,
        p.maximized
    )
}

fn parse(text: &str) -> Option<Place> {
    let t: toml::Table = text.parse().ok()?;
    let num = |k: &str| t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|f| f as f32).filter(|f| f.is_finite());
    Some(Place {
        pos: [num("x")?, num("y")?],
        size: [num("width")?, num("height")?],
        scale: num("scale").filter(|s| *s > 0.0).unwrap_or(1.0),
        maximized: t.get("maximized").and_then(toml::Value::as_bool).unwrap_or(false),
    })
}

/// Whether a window at `p` would be seen on `screens` (left, top, width and
/// height in pixels; none when the system does not say): a good part of its
/// top edge over a screen, so that it can be taken by its band.
fn shown(p: &Place, screens: Option<[f32; 4]>) -> bool {
    if p.size[0] < MIN_SIZE[0] || p.size[1] < MIN_SIZE[1] || p.size[0] > 20000.0 || p.size[1] > 20000.0 {
        return false;
    }
    let (x, y) = (p.pos[0] * p.scale, p.pos[1] * p.scale);
    let Some([left, top, width, height]) = screens else { return x.abs() < 30000.0 && y.abs() < 30000.0 };
    let band_left = x.max(left);
    let band_right = (x + p.size[0] * p.scale).min(left + width);
    band_right - band_left >= 100.0 && y >= top - 8.0 && y <= top + height - 40.0
}

/// The saved place, if there is one and a screen shows it.
pub fn load(path: &Path) -> Option<Place> {
    let p = parse(&std::fs::read_to_string(path).ok()?)?;
    shown(&p, screens()).then_some(p)
}

pub fn save(path: &Path, place: &Place) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, render(place));
}

/// All the screens together, in pixels: left, top, width, height.
#[cfg(windows)]
fn screens() -> Option<[f32; 4]> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};
    // SAFETY: plain queries of the system, with no pointers.
    let (x, y, w, h) = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN)) };
    (w > 0 && h > 0).then_some([x as f32, y as f32, w as f32, h as f32])
}

#[cfg(not(windows))]
fn screens() -> Option<[f32; 4]> {
    None
}

/// Follows the window and says when its place changed and has held still.
#[derive(Default)]
pub struct Tracker {
    /// The last place seen when the window was not maximized, minimized or
    /// full screen.
    normal: Option<Place>,
    /// What is written (or was loaded) already.
    written: Option<Place>,
    /// Since when `current` has been what it is.
    since: Option<Instant>,
    current: Option<Place>,
}

impl Tracker {
    /// Remember what was loaded, so a start that moves nothing writes nothing.
    pub fn loaded(&mut self, p: Place) {
        self.normal = Some(p);
        self.written = Some(p);
    }

    /// Call once a frame. The place to write now, if it changed and the
    /// window held still for a moment.
    pub fn observe(&mut self, ctx: &egui::Context) -> Option<Place> {
        let (outer, inner, maximized, minimized, full) = ctx.input(|i| {
            let v = i.viewport();
            (v.outer_rect, v.inner_rect, v.maximized.unwrap_or(false), v.minimized.unwrap_or(false), v.fullscreen.unwrap_or(false))
        });
        if minimized || full {
            return None;
        }
        let scale = ctx.pixels_per_point();
        let place = if maximized {
            // Keep the size it will go back to.
            Place { maximized: true, ..self.normal.or(self.written)? }
        } else {
            let (outer, inner) = (outer?, inner?);
            let place = Place { pos: [outer.min.x, outer.min.y], size: [inner.width(), inner.height()], scale, maximized: false };
            self.normal = Some(place);
            place
        };
        if self.current != Some(place) {
            self.current = Some(place);
            self.since = Some(Instant::now());
        }
        if self.written == Some(place) {
            return None;
        }
        let waited = self.since.map_or(Duration::ZERO, |s| s.elapsed());
        if waited >= SETTLE {
            self.written = Some(place);
            Some(place)
        } else {
            ctx.request_repaint_after(SETTLE - waited);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, w: f32, h: f32) -> Place {
        Place { pos: [x, y], size: [w, h], scale: 1.0, maximized: false }
    }

    #[test]
    fn a_place_survives_the_file() {
        let p = Place { pos: [-1200.0, 40.0], size: [900.0, 700.0], scale: 1.5, maximized: true };
        assert_eq!(parse(&render(&p)), Some(p));
    }

    #[test]
    fn a_bad_file_gives_nothing() {
        assert_eq!(parse("x = 1\n"), None);
        assert_eq!(parse("not toml ["), None);
        assert_eq!(parse("x = 1\ny = 2\nwidth = \"wide\"\nheight = 5\n"), None);
    }

    #[test]
    fn a_window_no_screen_shows_is_not_used() {
        let one = Some([0.0, 0.0, 1920.0, 1080.0]);
        assert!(shown(&at(100.0, 100.0, 1280.0, 800.0), one));
        // Partly over the edge, but the band can be taken.
        assert!(shown(&at(1500.0, 100.0, 1280.0, 800.0), one));
        // On a screen that is gone.
        assert!(!shown(&at(2500.0, 100.0, 1280.0, 800.0), one));
        assert!(!shown(&at(-2000.0, 100.0, 1280.0, 800.0), one));
        // Its band below the bottom, or too small to be a window.
        assert!(!shown(&at(100.0, 1070.0, 1280.0, 800.0), one));
        assert!(!shown(&at(100.0, 100.0, 50.0, 50.0), one));
        // A screen to the left makes negative places good.
        assert!(shown(&at(-1500.0, 100.0, 1280.0, 800.0), Some([-1920.0, 0.0, 3840.0, 1080.0])));
    }

    #[test]
    fn the_scale_turns_points_into_pixels() {
        let one = Some([0.0, 0.0, 1920.0, 1080.0]);
        let p = Place { pos: [1000.0, 100.0], size: [800.0, 600.0], scale: 2.0, maximized: false };
        assert!(!shown(&p, one), "1000 points at 2x is 2000 pixels");
        assert!(shown(&Place { scale: 1.0, ..p }, one));
    }
}
