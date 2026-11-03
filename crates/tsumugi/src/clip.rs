//! An image on the clipboard, kept as a file to attach to a prompt (the
//! design's input box: a screenshot pasted becomes a file and a chip).
//! egui hands over text only, so the clipboard is read here, on a thread.

use std::path::{Path, PathBuf};

/// Where pasted images go: `attachments/` beside the settings.
pub fn folder() -> Option<PathBuf> {
    tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(|d| d.join("attachments")))
}

/// The clipboard's text, or nothing: what `--keys` pastes for `<Paste>`
/// and `<C-v>`, as Ctrl+V pastes it.
pub fn text() -> String {
    arboard::Clipboard::new().and_then(|mut board| board.get_text()).unwrap_or_default()
}

/// WebP quality of a pasted screenshot (0 to 100).
const WEBP_QUALITY: f32 = 75.0;

/// The clipboard's image written as a WebP in `dir`: its path and size.
/// `Ok(None)` when the clipboard holds no image. Encoding a 4K picture takes
/// about a second, so call this from a worker thread.
pub fn paste_image(dir: &Path) -> Result<Option<(PathBuf, u64)>, String> {
    let Some(img) = ito_image::clipboard_image().map_err(|e| format!("the clipboard: {e}"))? else { return Ok(None) };
    let path = dir.join(format!("paste-{}.webp", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    write_webp(&path, &img)?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(Some((path, size)))
}

/// A picture as a lossy WebP file, its folder made first.
fn write_webp(path: &Path, img: &ito_image::Rgba) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let bytes = ito_image::encode_webp(img, WEBP_QUALITY).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Files' sizes, for the chips (a thread's work).
pub fn sizes(files: Vec<PathBuf>) -> Vec<(PathBuf, u64)> {
    files
        .into_iter()
        .map(|f| {
            let n = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            (f, n)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_become_a_webp() {
        let dir = std::env::temp_dir().join(format!("tsumugi-clip-{}", std::process::id()));
        let path = dir.join("sub/one.webp");
        let img = ito_image::Rgba::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        write_webp(&path, &img).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP", "not a WebP file");
        assert_eq!(sizes(vec![path.clone()])[0].1, std::fs::metadata(&path).unwrap().len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
