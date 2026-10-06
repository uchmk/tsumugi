//! Where a pane dragged by its header lands (the design's 1f): over the
//! middle of another pane the two trade places; near one of its edges the
//! pane goes to that side and the two share the room.

use eframe::egui;
use tsumugi_layout::Toward;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    Swap,
    Side(Toward),
}

/// The middle's share of each side: what is inside it is a swap.
const MIDDLE: f32 = 0.4;

/// What dropping at `at` on the pane in `r` does.
pub fn zone(r: egui::Rect, at: egui::Pos2) -> Zone {
    // Where in the pane, 0 to 1 each way.
    let x = ((at.x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0);
    let y = ((at.y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0);
    let edge = (1.0 - MIDDLE) / 2.0;
    if (edge..=1.0 - edge).contains(&x) && (edge..=1.0 - edge).contains(&y) {
        return Zone::Swap;
    }
    // The nearest edge.
    let sides = [(x, Toward::Left), (1.0 - x, Toward::Right), (y, Toward::Up), (1.0 - y, Toward::Down)];
    let (_, toward) = sides.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).expect("four sides");
    Zone::Side(toward)
}

/// The part of `r` the pane would take: all of it for a swap, else the half
/// on that side.
pub fn preview(r: egui::Rect, z: Zone) -> egui::Rect {
    let (w, h) = (r.width() / 2.0, r.height() / 2.0);
    match z {
        Zone::Swap => r,
        Zone::Side(Toward::Left) => egui::Rect::from_min_size(r.min, egui::vec2(w, r.height())),
        Zone::Side(Toward::Right) => egui::Rect::from_min_size(r.min + egui::vec2(w, 0.0), egui::vec2(w, r.height())),
        Zone::Side(Toward::Up) => egui::Rect::from_min_size(r.min, egui::vec2(r.width(), h)),
        Zone::Side(Toward::Down) => egui::Rect::from_min_size(r.min + egui::vec2(0.0, h), egui::vec2(r.width(), h)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_swaps_and_the_edges_split() {
        let r = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 200.0));
        assert_eq!(zone(r, r.center()), Zone::Swap);
        assert_eq!(zone(r, egui::pos2(110.0, 200.0)), Zone::Side(Toward::Left));
        assert_eq!(zone(r, egui::pos2(490.0, 210.0)), Zone::Side(Toward::Right));
        assert_eq!(zone(r, egui::pos2(300.0, 105.0)), Zone::Side(Toward::Up));
        assert_eq!(zone(r, egui::pos2(300.0, 295.0)), Zone::Side(Toward::Down));
        assert_eq!(preview(r, Zone::Side(Toward::Down)), egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(400.0, 100.0)));
    }
}
