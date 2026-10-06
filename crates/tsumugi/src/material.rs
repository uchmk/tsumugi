//! The window's material (v1-scope, the look): the desktop seen through the
//! band, sidebar and status bar, which are drawn see-through over it. On
//! Windows 11 Mica or Acrylic, on macOS the vibrancy (`NSVisualEffectView`),
//! through the `window-vibrancy` crate (QUESTIONS.md Q9). Elsewhere none.

use raw_window_handle::HasWindowHandle;

/// Ask the system for `kind` (`mica`, `acrylic`, `vibrancy`) behind the
/// window; true when it took it. `light` picks the light or dark tint
/// where the system asks.
pub fn apply(window: &impl HasWindowHandle, kind: &str, light: bool) -> bool {
    os::apply(window, kind, light)
}

#[cfg(windows)]
mod os {
    use super::HasWindowHandle;

    pub fn apply(window: &impl HasWindowHandle, kind: &str, light: bool) -> bool {
        match kind {
            // A light or dark tint, a little of the theme's own.
            "acrylic" => window_vibrancy::apply_acrylic(window, Some(if light { (238, 240, 243, 120) } else { (18, 20, 24, 120) })).is_ok(),
            "mica" | "vibrancy" => window_vibrancy::apply_mica(window, Some(!light)).is_ok(),
            _ => false,
        }
    }
}

#[cfg(target_os = "macos")]
mod os {
    use super::HasWindowHandle;
    use window_vibrancy::NSVisualEffectMaterial;

    pub fn apply(window: &impl HasWindowHandle, kind: &str, _light: bool) -> bool {
        // Any of the three is the vibrancy here: the sidebar's, or the
        // thinner HUD's for "acrylic".
        let material = if kind == "acrylic" { NSVisualEffectMaterial::HudWindow } else { NSVisualEffectMaterial::Sidebar };
        kind != "none" && window_vibrancy::apply_vibrancy(window, material, None, None).is_ok()
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod os {
    use super::HasWindowHandle;

    pub fn apply(_window: &impl HasWindowHandle, _kind: &str, _light: bool) -> bool {
        false
    }
}
