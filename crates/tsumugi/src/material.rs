//! The window's material (v1-scope, the look): on Windows 11, Mica or
//! Acrylic behind the band, sidebar and status bar, which are drawn
//! see-through over it. Elsewhere nothing yet (macOS's vibrancy waits on
//! QUESTIONS.md Q9).

/// Ask the system for `kind` (`mica`, `acrylic`) behind the window `hwnd`;
/// true when it took it. `light` picks the light or dark tint.
#[cfg(windows)]
pub fn apply(hwnd: Option<isize>, kind: &str, light: bool) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DWM_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea,
        DwmSetWindowAttribute,
    };
    use windows::Win32::UI::Controls::MARGINS;
    let Some(hwnd) = hwnd else { return false };
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let backdrop: DWM_SYSTEMBACKDROP_TYPE = if kind == "acrylic" { DWMSBT_TRANSIENTWINDOW } else { DWMSBT_MAINWINDOW };
    let dark: i32 = i32::from(!light);
    // SAFETY: a window handle of this process, and attributes of the sizes
    // the calls are told.
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&raw const dark).cast(), size_of::<i32>() as u32);
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        if DwmExtendFrameIntoClientArea(hwnd, &margins).is_err() {
            return false;
        }
        // Before Windows 11 22H2 the attribute is unknown, and this fails.
        DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, (&raw const backdrop).cast(), size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32).is_ok()
    }
}

#[cfg(not(windows))]
pub fn apply(_hwnd: Option<isize>, _kind: &str, _light: bool) -> bool {
    false
}
