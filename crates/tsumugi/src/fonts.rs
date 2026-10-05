//! A system font for the characters egui's own monospace lacks (Japanese, in
//! the first place), behind it so that Latin text keeps egui's Hack.

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

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some((path, bytes)) = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok().map(|b| (*p, b))) {
        let name = format!("system:{path}");
        fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
}
