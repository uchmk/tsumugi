//! The fonts: a Nerd Font in front when one is installed, as filer does (its
//! icons are what a prompt such as starship draws, and the branch mark), and
//! a system font behind egui's own for what that lacks (Japanese, in the
//! first place).

use eframe::egui;

/// Fonts to try, in order; the first that reads is used.
#[cfg(windows)]
const CANDIDATES: &[&str] = &[r"C:\Windows\Fonts\BIZ-UDGothicR.ttc", r"C:\Windows\Fonts\msgothic.ttc", r"C:\Windows\Fonts\YuGothM.ttc", r"C:\Windows\Fonts\meiryo.ttc"];
#[cfg(target_os = "macos")]
const CANDIDATES: &[&str] = &["/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", "/System/Library/Fonts/Hiragino Sans GB.ttc"];
#[cfg(not(any(windows, target_os = "macos")))]
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
    "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
];

/// The Nerd Fonts filer looks for (filer's `NERD_FONTS`), in the same order.
const NERD_FONTS: [&str; 6] = [
    "HackGen35ConsoleNF-Regular.ttf",
    "HackGenConsoleNF-Regular.ttf",
    "HackGen35Console-Regular.ttf",
    "FiraCodeNerdFont-Regular.ttf",
    "CaskaydiaCoveNerdFont-Regular.ttf",
    "JetBrainsMonoNerdFont-Regular.ttf",
];

/// Where a user's own fonts are, then the system's: a Nerd Font is usually
/// installed for the user alone.
fn font_dirs() -> Vec<std::path::PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let mut out = Vec::new();
    if cfg!(windows) {
        out.extend(var("LOCALAPPDATA").map(|d| d.join("Microsoft").join("Windows").join("Fonts")));
        out.push(std::path::PathBuf::from(r"C:\Windows\Fonts"));
    } else if cfg!(target_os = "macos") {
        out.extend(var("HOME").map(|h| h.join("Library").join("Fonts")));
        out.push("/Library/Fonts".into());
    } else {
        out.extend(var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("share"))).map(|d| d.join("fonts")));
        out.extend(var("HOME").map(|h| h.join(".fonts")));
        out.push("/usr/share/fonts/truetype".into());
        out.push("/usr/local/share/fonts".into());
    }
    out
}

/// The private-use character Nerd Fonts (and Powerline) draw the git branch
/// mark with; filer's status bar uses the same.
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
}

/// Read the fonts for `family` (see `[font]` in the settings): it in front
/// of the monospace text, else a Nerd Font when one is installed, as filer
/// does (its icons are what a prompt such as starship draws); a system font
/// behind egui's own for what those lack (Japanese, in the first place).
/// Reads files: not on the UI thread, but for the first frame.
pub fn load(family: &str) -> Loaded {
    let mut fonts = egui::FontDefinitions::default();
    let nerd_file = || font_dirs().iter().flat_map(|d| NERD_FONTS.iter().map(move |n| d.join(n))).find(|p| p.is_file());
    let chosen = if family.trim().is_empty() { None } else { find(family.trim()) };
    let regular = chosen.clone().or_else(nerd_file);
    let read = |p: &std::path::Path| std::fs::read(p).ok().map(|b| egui::FontData::from_owned(b).into());
    let mut nerd = false;
    let mut faces = [false; 3];
    let mut file = None;
    if let Some(path) = &regular {
        if let Some(data) = read(path) {
            let name = format!("mono:{}", path.display());
            fonts.font_data.insert(name.clone(), data);
            // In front, as filer has it: the pane's text in the face its owner chose.
            fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, name.clone());
            let stem = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            nerd = NERD_FONTS.contains(&stem.as_str()) || stem.contains("NerdFont") || stem.contains("NF-");
            // The window's own words in it only when it is a Nerd Font, as before.
            if chosen.is_none() {
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name);
            }
            file = Some(path.clone());
        }
    }
    let system = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok().map(|b| (*p, b)));
    if let Some((path, bytes)) = &system {
        let name = format!("system:{path}");
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes.clone()).into());
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
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
    Loaded { defs: fonts, nerd, faces, file }
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
        let s = |p: &str, st| siblings(std::path::Path::new(p), st).into_iter().map(|p| p.display().to_string()).collect::<Vec<_>>();
        assert_eq!(s("/f/JetBrainsMono-Regular.ttf", Style::Bold)[0], "/f/JetBrainsMono-Bold.ttf");
        assert!(s("/f/Hack.ttf", Style::Italic).contains(&"/f/Hack-Italic.ttf".to_owned()));
        assert!(s("/f/consola.ttf", Style::BoldItalic).contains(&"/f/consolaz.ttf".to_owned()));
        assert!(s("/f/DejaVuSansMono.ttf", Style::BoldItalic).contains(&"/f/DejaVuSansMono-BoldOblique.ttf".to_owned()));
        assert_eq!(rank(&squash("JetBrainsMono-Regular")), 0);
        assert_eq!(rank(&squash("JetBrainsMono-Bold")), 2);
    }
}
