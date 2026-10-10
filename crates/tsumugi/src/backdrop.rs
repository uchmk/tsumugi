//! A picture behind the panes (`[window] image`): read and made smaller on
//! a thread, then drawn to fill each pane, cut to its shape rather than
//! stretched.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;

/// The longest side kept: more is memory the screen does not show.
const LONGEST: u32 = 2560;

#[derive(Default)]
pub struct Backdrop {
    /// The setting it was loaded for.
    path: String,
    texture: Option<egui::TextureHandle>,
    loading: Option<Receiver<Result<egui::ColorImage, String>>>,
    /// Why the picture could not be shown, said once in a toast.
    pub error: Option<String>,
}

impl Backdrop {
    /// The picture for the setting `path` (empty for none): a new one is
    /// read on a thread, and is there from a later frame on.
    pub fn texture(&mut self, ctx: &egui::Context, path: &str) -> Option<&egui::TextureHandle> {
        if self.path != path {
            self.path = path.to_owned();
            self.texture = None;
            self.loading = None;
            if !path.trim().is_empty() {
                let (tx, rx) = channel();
                let (file, ctx2) = (resolve(path), ctx.clone());
                let _ = std::thread::Builder::new().name("backdrop".into()).spawn(move || {
                    let _ = tx.send(load(&file));
                    ctx2.request_repaint();
                });
                self.loading = Some(rx);
            }
        }
        if let Some(Ok(got)) = self.loading.as_ref().map(Receiver::try_recv) {
            self.loading = None;
            match got {
                Ok(image) => self.texture = Some(ctx.load_texture("backdrop", image, egui::TextureOptions::LINEAR)),
                Err(e) => self.error = Some(e),
            }
        }
        self.texture.as_ref()
    }
}

/// `~/` is the home folder; a relative path is beside config.toml.
fn resolve(path: &str) -> PathBuf {
    let path = path.trim();
    if let (Some(rest), Some(home)) = (path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")), tsumugi_mux::settings::home()) {
        return home.join(rest);
    }
    let p = PathBuf::from(path);
    match tsumugi_mux::settings::default_path().as_deref().and_then(Path::parent) {
        Some(dir) if p.is_relative() => dir.join(p),
        _ => p,
    }
}

fn load(file: &Path) -> Result<egui::ColorImage, String> {
    let said = |e: &dyn std::fmt::Display| format!("The background image {}: {e}", file.display());
    let image = image::ImageReader::open(file).map_err(|e| said(&e))?.with_guessed_format().map_err(|e| said(&e))?.decode().map_err(|e| said(&e))?;
    let image = if image.width().max(image.height()) > LONGEST { image.thumbnail(LONGEST, LONGEST) } else { image };
    let rgba = image.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}

/// The part of a picture of `size` that fills `rect` without stretching:
/// all of its height or all of its width, the middle of the other.
pub fn cover(size: egui::Vec2, rect: egui::Vec2) -> egui::Rect {
    if size.x <= 0.0 || size.y <= 0.0 || rect.x <= 0.0 || rect.y <= 0.0 {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    }
    let (pic, room) = (size.x / size.y, rect.x / rect.y);
    if pic > room {
        // Wider than the room: the middle of its width.
        let w = room / pic;
        egui::Rect::from_min_max(egui::pos2((1.0 - w) / 2.0, 0.0), egui::pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = pic / room;
        egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - h) / 2.0), egui::pos2(1.0, (1.0 + h) / 2.0))
    }
}

/// Draw the picture to fill `rect`, `strength` (0 to 1) of it.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, texture: &egui::TextureHandle, strength: f32) {
    let uv = cover(texture.size_vec2(), rect.size());
    painter.image(texture.id(), rect, uv, egui::Color32::WHITE.gamma_multiply(strength.clamp(0.0, 1.0)));
}

/// The parts of `area` outside every one of `holes`, as one mesh of
/// rectangles that meet without a seam: the window's backdrop around the
/// panes when the panes have their own, see-through one.
pub fn around(area: egui::Rect, holes: &[egui::Rect], color: egui::Color32) -> egui::Mesh {
    let mut xs = vec![area.left(), area.right()];
    let mut ys = vec![area.top(), area.bottom()];
    for h in holes {
        xs.extend([h.left(), h.right()]);
        ys.extend([h.top(), h.bottom()]);
    }
    for v in [&mut xs, &mut ys] {
        v.retain(|x| x.is_finite());
        v.sort_by(f32::total_cmp);
        v.dedup();
    }
    let mut mesh = egui::Mesh::default();
    for y in ys.windows(2) {
        for x in xs.windows(2) {
            let cell = egui::Rect::from_min_max(egui::pos2(x[0], y[0]), egui::pos2(x[1], y[1]));
            if !area.contains(cell.center()) || holes.iter().any(|h| h.contains(cell.center())) {
                continue;
            }
            mesh.add_colored_rect(cell, color);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_cut_not_stretched() {
        let wide = cover(egui::vec2(200.0, 100.0), egui::vec2(100.0, 100.0));
        assert_eq!((wide.left(), wide.right(), wide.top(), wide.bottom()), (0.25, 0.75, 0.0, 1.0));
        let tall = cover(egui::vec2(100.0, 100.0), egui::vec2(200.0, 100.0));
        assert_eq!((tall.left(), tall.right(), tall.top(), tall.bottom()), (0.0, 1.0, 0.25, 0.75));
        assert_eq!(cover(egui::vec2(0.0, 1.0), egui::vec2(1.0, 1.0)), egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)));
    }

    #[test]
    fn the_backdrop_leaves_the_panes_out() {
        let area = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(100.0, 50.0));
        let holes = [egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(45.0, 40.0)), egui::Rect::from_min_max(egui::pos2(55.0, 10.0), egui::pos2(90.0, 40.0))];
        let mesh = around(area, &holes, egui::Color32::BLACK);
        let covered: f32 = mesh.indices.chunks(6).map(|q| {
            let a = mesh.vertices[q[0] as usize].pos;
            let b = mesh.vertices[q[1] as usize].pos;
            let c = mesh.vertices[q[2] as usize].pos;
            let (x0, x1) = (a.x.min(b.x).min(c.x), a.x.max(b.x).max(c.x));
            let (y0, y1) = (a.y.min(b.y).min(c.y), a.y.max(b.y).max(c.y));
            (x1 - x0) * (y1 - y0)
        }).sum();
        assert_eq!(covered, 100.0 * 50.0 - 2.0 * 35.0 * 30.0);
    }

    #[test]
    fn a_relative_picture_is_beside_the_settings() {
        let p = resolve("bg.png");
        if let Some(dir) = tsumugi_mux::settings::default_path().as_deref().and_then(Path::parent) {
            assert_eq!(p, dir.join("bg.png"));
        }
        assert!(resolve("/x/y.png").ends_with("y.png"));
    }
}
