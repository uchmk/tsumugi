//! Give `tsumugi.exe` its icon and version information.
//!
//! This is the icon Explorer draws on the file and a pinned taskbar entry
//! uses, and the name and version Task Manager and the file's Details show;
//! the window's own icon is set at startup from the same drawing
//! (`src/icon.rs`). The `.res` is written in Rust by ito's `ito-winres`, so
//! no resource compiler is needed and a build for Windows from another OS
//! gets it too. Only for the MSVC targets.

#[path = "src/icon.rs"]
mod icon;

/// The sizes Explorer and the taskbar pick from, at 100 % to 200 % scaling.
const SIDES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    ito_winres::emit("tsumugi", &SIDES, icon::pixels);
}
