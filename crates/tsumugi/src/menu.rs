//! What the tab's right-click menu (the design's 1j) does outside the
//! window: run a command from the settings, or open another window on a tab.
//! Each on a thread of its own, so a slow start never holds up a frame.

use std::process::{Command, Stdio};

use tsumugi_mux::WorkspaceId;

/// Run a command line through the system's shell (`cmd /C`, `sh -c`), the
/// way `[open]` and `[[menu.session]]` in the settings are written.
pub fn run(line: String) {
    let _ = std::thread::Builder::new().name("menu-command".into()).spawn(move || {
        let mut cmd = shell(&line);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(mut child) = cmd.spawn() {
            let _ = child.wait();
        }
    });
}

#[cfg(windows)]
fn shell(line: &str) -> Command {
    use std::os::windows::process::CommandExt;
    // No console window flashing up for it.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").raw_arg(line).creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn shell(line: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(line);
    cmd
}

/// Open a file or folder the way the system would on a double click.
pub fn open_with_system(path: &std::path::Path) {
    let quoted = tsumugi_mux::settings::fill("{folder}", path, 0);
    let line = if cfg!(windows) {
        format!("start \"\" {quoted}")
    } else if cfg!(target_os = "macos") {
        format!("open {quoted}")
    } else {
        format!("xdg-open {quoted}")
    };
    run(line);
}

/// The environment variable a new window reads to show a tab first.
pub const SHOW_TAB: &str = "TSUMUGI_SHOW_TAB";

/// Another window of this program, showing `tab`.
pub fn new_window(tab: WorkspaceId) {
    let _ = std::thread::Builder::new().name("new-window".into()).spawn(move || {
        let Ok(exe) = std::env::current_exe() else { return };
        let mut cmd = Command::new(exe);
        cmd.env(SHOW_TAB, tab.to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(mut child) = cmd.spawn() {
            let _ = child.wait();
        }
    });
}
