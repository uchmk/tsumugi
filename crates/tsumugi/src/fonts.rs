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

/// Install the fonts; true when a Nerd Font is among them, so that its
/// characters (`BRANCH`) can be written as text.
pub fn install(ctx: &egui::Context) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let nerd = font_dirs()
        .iter()
        .flat_map(|d| NERD_FONTS.iter().map(move |n| d.join(n)))
        .find_map(|p| std::fs::read(&p).ok().map(|b| (p, b)));
    if let Some((path, bytes)) = &nerd {
        let name = format!("nerd:{}", path.display());
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes.clone()).into());
        // In front, as filer has it: the pane's text in the face its owner chose.
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().insert(0, name.clone());
        }
    }
    if let Some((path, bytes)) = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok().map(|b| (*p, b))) {
        let name = format!("system:{path}");
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
    nerd.is_some()
}
