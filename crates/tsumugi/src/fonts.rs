//! The fonts: a Nerd Font in front when one is installed, as kura does (its
//! icons are what a prompt such as starship draws, and the branch mark), and
//! a system font behind egui's own for what that lacks (Japanese, in the
//! first place). Which files, and in what order, every uchmk app shares
//! (`ito_common::fonts`).

use eframe::egui;
use ito_common::fonts::{JAPANESE, JAPANESE_BOLD, NERD_FONTS, find_file, font_dirs, nerd_font, read_first};

/// The window's own words (titles, buttons) in a proportional face, as the
/// design has them (IBM Plex Sans JP): the first of these found in the font
/// folders, else egui's own. Then their bold, for titles.
const UI_REGULAR: &[&str] = &[
    "IBMPlexSansJP-Regular.ttf",
    "IBMPlexSansJP-Regular.otf",
    "IBMPlexSans-Regular.ttf",
    "IBMPlexSans-Regular.otf",
    "segoeui.ttf",
    "NotoSans-Regular.ttf",
    "DejaVuSans.ttf",
];
const UI_BOLD: &[&str] = &[
    "IBMPlexSansJP-SemiBold.ttf",
    "IBMPlexSansJP-SemiBold.otf",
    "IBMPlexSansJP-Bold.ttf",
    "IBMPlexSans-SemiBold.ttf",
    "IBMPlexSans-SemiBold.otf",
    "seguisb.ttf",
    "segoeuib.ttf",
    "NotoSans-SemiBold.ttf",
    "DejaVuSans-Bold.ttf",
];

/// The family the window's titles are drawn in: bold, proportional.
pub const UI_BOLD_FAMILY: &str = "ui-bold";

/// A title's font: the bold proportional face at `size`.
pub fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(UI_BOLD_FAMILY.into()))
}

/// The private-use character Nerd Fonts (and Powerline) draw the git branch
/// mark with; kura's status bar uses the same.
pub const BRANCH: char = '\u{e0a0}';

/// The families the pane's bold and italic text are drawn in, when found.
pub const BOLD: &str = "mono-bold";
pub const ITALIC: &str = "mono-italic";
pub const BOLD_ITALIC: &str = "mono-bold-italic";

/// The fonts read from disk for one `[font] family`, ready to install.
pub struct Loaded {
    pub defs: egui::FontDefinitions,
    /// A Nerd Font is in front: its characters (`BRANCH`) can be written.
    pub nerd: bool,
    /// Which of bold, italic and bold italic were found.
    pub faces: [bool; 3],
    /// The regular file the panes are drawn in, when not the built-in one.
    pub file: Option<std::path::PathBuf>,
    /// That file, to shape and draw its ligatures from (Q8).
    pub shaper: Option<std::sync::Arc<ito_pane::Shaper>>,
}

/// Read the fonts for `family` (see `[font]` in the settings): it in front
/// of the monospace text, else a Nerd Font when one is installed, as filer
/// does (its icons are what a prompt such as starship draws); a system font
/// behind egui's own for what those lack (Japanese, in the first place).
/// Reads files: not on the UI thread, but for the first frame.
pub fn load(family: &str) -> Loaded {
    let mut fonts = egui::FontDefinitions::default();
    let chosen = if family.trim().is_empty() { None } else { find(family.trim()) };
    let regular = chosen.clone().or_else(nerd_font);
    let read = |p: &std::path::Path| std::fs::read(p).ok().map(|b| egui::FontData::from_owned(b).into());
    let mut nerd = false;
    let mut faces = [false; 3];
    let mut file = None;
    let mut shaper = None;
    if let Some(path) = &regular {
        if let Some(data) = read(path) {
            shaper = std::fs::read(path).ok().and_then(|b| ito_pane::Shaper::new(b, 0));
            let name = format!("mono:{}", path.display());
            fonts.font_data.insert(name.clone(), data);
            // In front, as filer has it: the pane's text in the face its owner chose.
            fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, name.clone());
            let stem = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            nerd = NERD_FONTS.contains(&stem.as_str()) || stem.contains("NerdFont") || stem.contains("NF-");
            // The window's own words stay proportional (the design): a Nerd
            // Font only behind them, for its icons. Before, with no family
            // chosen, it went in front and every word came out monospace.
            if chosen.is_none() {
                fonts.families.entry(egui::FontFamily::Proportional).or_default().push(name);
            }
            file = Some(path.clone());
        }
    }
    // The window's words: a proportional face in front of egui's own, and its
    // bold for titles (egui has no bold of its own: `strong` is only a colour).
    if let Some((path, data)) = find_file(UI_REGULAR).and_then(|p| read(&p).map(|d| (p, d))) {
        let name = format!("ui:{}", path.display());
        fonts.font_data.insert(name.clone(), data);
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name);
    }
    let mut bold_list = Vec::new();
    for path in find_file(UI_BOLD).into_iter().chain(JAPANESE_BOLD.iter().map(std::path::PathBuf::from).filter(|p| p.is_file()).take(1)) {
        if let Some(data) = read(&path) {
            let name = format!("ui-bold:{}", path.display());
            fonts.font_data.insert(name.clone(), data);
            bold_list.push(name);
        }
    }
    if let Some((path, bytes)) = read_first(JAPANESE) {
        let name = format!("system:{}", path.display());
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    // The bold family: its faces, then the proportional ones for the rest.
    bold_list.extend(fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(egui::FontFamily::Name(UI_BOLD_FAMILY.into()), bold_list);
    // Bold and italic: the regular file's siblings, then the same fallbacks.
    let fallback = fonts.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    if let Some(path) = &file {
        for (k, (family, style)) in [(BOLD, Style::Bold), (ITALIC, Style::Italic), (BOLD_ITALIC, Style::BoldItalic)].into_iter().enumerate() {
            let Some((sib, data)) = siblings(path, style).into_iter().find_map(|p| read(&p).map(|d| (p, d))) else { continue };
            let name = format!("{family}:{}", sib.display());
            fonts.font_data.insert(name.clone(), data);
            let mut list = vec![name];
            list.extend(fallback.iter().skip(1).cloned());
            fonts.families.insert(egui::FontFamily::Name(family.into()), list);
            faces[k] = true;
        }
    }
    Loaded { defs: fonts, nerd, faces, file, shaper }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    Bold,
    Italic,
    BoldItalic,
}

/// The files a bold or italic face of `regular` is likely to be in: the
/// style word in place of `Regular` (`JetBrainsMono-Bold.ttf`), after the
/// name (`Hack-Bold.ttf`), or Windows' one letter (`consolab.ttf`).
pub fn siblings(regular: &std::path::Path, style: Style) -> Vec<std::path::PathBuf> {
    let (Some(dir), Some(stem), Some(ext)) = (regular.parent(), regular.file_stem(), regular.extension()) else { return vec![] };
    let (stem, ext) = (stem.to_string_lossy(), ext.to_string_lossy());
    // Italic is Oblique in some families (DejaVu, FreeMono).
    let (words, letter): (&[&str], &str) = match style {
        Style::Bold => (&["Bold"], "b"),
        Style::Italic => (&["Italic", "Oblique"], "i"),
        Style::BoldItalic => (&["BoldItalic", "BoldOblique", "Bold Italic"], "z"),
    };
    let mut names = Vec::new();
    for word in words {
        for regular_word in ["Regular", "regular", "Book"] {
            if stem.contains(regular_word) {
                names.push(stem.replacen(regular_word, word, 1));
            }
        }
        names.push(format!("{stem}-{word}"));
        names.push(format!("{stem}{word}"));
    }
    names.push(format!("{stem}{letter}"));
    names.into_iter().map(|n| dir.join(format!("{n}.{ext}"))).collect()
}

/// The font file `family` means: a path, else the file in the font folders
/// whose name has it in (spaces and case aside), a regular face first.
pub fn find(family: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(family);
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let want = squash(family);
    let mut found: Vec<std::path::PathBuf> = files().into_iter().filter(|p| p.file_stem().is_some_and(|s| squash(&s.to_string_lossy()).contains(&want))).collect();
    found.sort_by_key(|p| {
        let s = squash(&p.file_stem().unwrap_or_default().to_string_lossy());
        (rank(&s), s.len())
    });
    found.into_iter().next()
}

/// A file name's letters, lower case, without spaces, dashes or underscores.
fn squash(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).flat_map(char::to_lowercase).collect()
}

/// Regular faces first, styled ones last.
fn rank(squashed: &str) -> u8 {
    if squashed.contains("regular") {
        0
    } else if ["bold", "italic", "light", "thin", "medium", "semibold", "extra", "black", "oblique"].iter().any(|w| squashed.contains(w)) {
        2
    } else {
        1
    }
}

/// The font files in the font folders and one level below.
fn files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let is_font = |p: &std::path::Path| p.extension().is_some_and(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "ttf" | "otf" | "ttc"));
    for dir in font_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&p) {
                    out.extend(inner.flatten().map(|e| e.path()).filter(|p| is_font(p)));
                }
            } else if is_font(&p) {
                out.push(p);
            }
        }
    }
    out
}

static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Look for the installed monospace fonts on a thread, for [`names`].
pub fn scan_names() {
    let _ = std::thread::Builder::new().name("font-names".into()).spawn(|| {
        let _ = NAMES.set(monospace_names());
    });
}

/// The installed monospace fonts, once [`scan_names`] has found them.
pub fn names() -> &'static [String] {
    NAMES.get().map_or(&[], Vec::as_slice)
}

/// The monospace-looking families installed, by name, for the settings
/// screen's list: regular faces whose names say they are for code.
fn monospace_names() -> Vec<String> {
    let hints = ["mono", "code", "console", "consol", "nerd", "nf", "hack", "courier", "cascadia", "menlo", "monaco", "fira", "sarasa", "hackgen", "ricty", "inconsolata", "iosevka"];
    let mut names: Vec<String> = files()
        .into_iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .filter(|s| rank(&squash(s)) < 2 && hints.iter().any(|h| squash(s).contains(h)))
        .map(|s| s.trim_end_matches("-Regular").trim_end_matches("Regular").to_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bold_face_is_looked_for_beside_the_regular_one() {
        // The names only: the folder is joined with the system's separator.
        let s = |p: &str, st| {
            let found = siblings(&std::path::Path::new("f").join(p), st);
            assert!(found.iter().all(|f| f.parent() == Some(std::path::Path::new("f"))), "beside the regular one");
            found.into_iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>()
        };
        assert_eq!(s("JetBrainsMono-Regular.ttf", Style::Bold)[0], "JetBrainsMono-Bold.ttf");
        assert!(s("Hack.ttf", Style::Italic).contains(&"Hack-Italic.ttf".to_owned()));
        assert!(s("consola.ttf", Style::BoldItalic).contains(&"consolaz.ttf".to_owned()));
        assert!(s("DejaVuSansMono.ttf", Style::BoldItalic).contains(&"DejaVuSansMono-BoldOblique.ttf".to_owned()));
        assert_eq!(rank(&squash("JetBrainsMono-Regular")), 0);
        assert_eq!(rank(&squash("JetBrainsMono-Bold")), 2);
    }
}
