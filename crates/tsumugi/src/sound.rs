//! The fourth way to tell (the design's 1h): the system's own sound, once,
//! on a thread of its own. Windows' message beep; on macOS a sound of the
//! system's played by `afplay`; elsewhere the freedesktop sound theme's, by
//! `canberra-gtk-play` or `paplay`, when either is there.

/// Play the sound for something that wants a person, or (`error`) failed.
pub fn play(error: bool) {
    let _ = std::thread::Builder::new().name("sound".into()).spawn(move || os::play(error));
}

#[cfg(windows)]
mod os {
    pub fn play(error: bool) {
        use windows::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONASTERISK, MB_ICONHAND};
        // SAFETY: no pointers; the sound is the system's.
        let _ = unsafe { MessageBeep(if error { MB_ICONHAND } else { MB_ICONASTERISK }) };
    }
}

#[cfg(not(windows))]
mod os {
    use std::process::{Command, Stdio};

    fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    #[cfg(target_os = "macos")]
    pub fn play(error: bool) {
        let name = if error { "Basso" } else { "Glass" };
        run("afplay", &[&format!("/System/Library/Sounds/{name}.aiff")]);
    }

    #[cfg(not(target_os = "macos"))]
    pub fn play(error: bool) {
        let (event, file) = if error { ("dialog-error", "dialog-error.oga") } else { ("message-new-instant", "message.oga") };
        if !run("canberra-gtk-play", &["-i", event]) {
            run("paplay", &[&format!("/usr/share/sounds/freedesktop/stereo/{file}")]);
        }
    }
}
