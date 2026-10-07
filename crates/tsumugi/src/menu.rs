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
    // `/S /C "…"`: cmd takes off exactly the outer pair of quotes and runs
    // the rest as it is. A bare `/C` with more than two quotes strips the
    // first and the last, so a line starting with a quoted program
    // (`"C:\Program Files\…\Code.exe" {folder}`) broke (the source review,
    // 2026-10-07).
    let mut cmd = Command::new("cmd");
    cmd.raw_arg(windows_line(line)).creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// What follows `cmd` for `line`.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_line(line: &str) -> String {
    format!("/S /C \"{line}\"")
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

/// Open a web page in the system's browser.
pub fn open_url(url: &str) {
    // Only what a pull request's address is made of reaches the shell, or
    // a port on this machine (a session's dev server).
    let local = url.strip_prefix("http://localhost:").is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if !(url.starts_with("https://") || local) || url.contains(|c: char| c.is_whitespace() || "\"'`$&|;<>^%".contains(c)) {
        return;
    }
    let line = if cfg!(windows) {
        format!("start \"\" \"{url}\"")
    } else if cfg!(target_os = "macos") {
        format!("open '{url}'")
    } else {
        format!("xdg-open '{url}'")
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

#[cfg(test)]
mod tests {
    /// A line starting with a quoted program keeps every quote: only the
    /// outer pair `/S` takes off is added.
    #[test]
    fn a_line_with_quotes_goes_to_cmd_whole() {
        let line = r#""C:\Program Files\Code\Code.exe" "C:\dev\x""#;
        assert_eq!(super::windows_line(line), format!("/S /C \"{line}\""));
    }
}
