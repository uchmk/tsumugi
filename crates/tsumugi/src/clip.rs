//! An image on the clipboard, kept as a file to attach to a prompt (the
//! design's input box: a screenshot pasted becomes a file and a chip).
//! egui hands over text only, so the clipboard is read here, on a thread.

use std::path::{Path, PathBuf};

/// Where pasted images go: `attachments/` beside the settings.
pub fn folder() -> Option<PathBuf> {
    tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(|d| d.join("attachments")))
}

/// The clipboard's image written as a PNG in `dir`: its path and size.
/// `Ok(None)` when the clipboard holds no image.
pub fn paste_image(dir: &Path) -> Result<Option<(PathBuf, u64)>, String> {
    let mut board = arboard::Clipboard::new().map_err(|e| format!("the clipboard: {e}"))?;
    let Ok(img) = board.get_image() else { return Ok(None) };
    let name = format!("paste-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let path = dir.join(name);
    write_png(&path, img.width as u32, img.height as u32, img.bytes.into_owned())?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(Some((path, size)))
}

/// RGBA pixels as a PNG file, its folder made first.
fn write_png(path: &Path, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let img = image::RgbaImage::from_raw(width, height, rgba).ok_or("the image's size and its pixels disagree")?;
    img.save_with_format(path, image::ImageFormat::Png).map_err(|e| format!("{}: {e}", path.display()))
}

/// Files' sizes, for the chips (a thread's work).
pub fn sizes(files: Vec<PathBuf>) -> Vec<(PathBuf, u64)> {
    files.into_iter().map(|f| {
        let n = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
        (f, n)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_become_a_png() {
        let dir = std::env::temp_dir().join(format!("tsumugi-clip-{}", std::process::id()));
        let path = dir.join("sub/one.png");
        write_png(&path, 2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        let back = image::open(&path).unwrap().to_rgba8();
        assert_eq!((back.width(), back.height()), (2, 1));
        assert_eq!(back.get_pixel(1, 0).0, [0, 0, 255, 255]);
        assert!(write_png(&dir.join("bad.png"), 3, 3, vec![0; 4]).is_err());
        assert_eq!(sizes(vec![path.clone()])[0].1, std::fs::metadata(&path).unwrap().len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
