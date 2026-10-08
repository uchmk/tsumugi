//! What the tab's right-click menu (the design's 1j) does outside the
//! window: run a command from the settings, or open another window on a tab.
//! Each on a thread of its own, so a slow start never holds up a frame.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

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

/// Open a web page in the system's browser; `false` when the address was
/// not one to hand to the shell.
pub fn open_url(url: &str) -> bool {
    if !url_ok(url) {
        return false;
    }
    let line = if cfg!(windows) {
        format!("start \"\" \"{url}\"")
    } else if cfg!(target_os = "macos") {
        format!("open '{url}'")
    } else {
        format!("xdg-open '{url}'")
    };
    run(line);
    true
}

/// Whether `url` may reach the shell: a web address (a pull request's, or
/// one a pane printed) with none of the characters a shell or `cmd` would
/// read as its own.
fn url_ok(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty()) && !url.contains(|c: char| c.is_whitespace() || c.is_control() || "\"'`$&|;<>^%!(){}".contains(c))
}

/// Where a path a pane printed points: `~` is the home folder, and a
/// relative path is under the session's folder.
pub fn resolve(path: &str, cwd: &Path, home: Option<&Path>) -> PathBuf {
    let tilde = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    match (tilde, home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if path == "~" && home.is_some() => home.unwrap_or(cwd).to_path_buf(),
        _ => cwd.join(path),
    }
}

/// Open a file a pane printed (a Ctrl+click on it): with `[open] file` at
/// the line and column, or the system's own way when that is empty or it is
/// a folder. Looks at the disk on a thread; what went wrong goes to `told`.
pub fn open_path(path: PathBuf, line: Option<u32>, column: Option<u32>, command: String, told: Sender<Result<String, String>>) {
    let _ = std::thread::Builder::new().name("open-path".into()).spawn(move || {
        let Ok(meta) = std::fs::metadata(&path) else {
            let _ = told.send(Err(format!("No such file: {}", path.display())));
            return;
        };
        if meta.is_dir() || command.trim().is_empty() {
            open_with_system(&path);
        } else {
            run(tsumugi_mux::settings::fill_file(&command, &path, line.unwrap_or(1), column.unwrap_or(1)));
        }
    });
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

    #[test]
    fn only_plain_web_addresses_reach_the_shell() {
        assert!(super::url_ok("https://github.com/o/r/pull/12"));
        assert!(super::url_ok("http://localhost:5173/a?b=c"));
        assert!(super::url_ok("http://127.0.0.1:8080"));
        for bad in ["file:///etc/passwd", "https://", "https://a.b/x&calc", "https://a.b/'$(x)'", "https://a.b/%PATH%", "javascript:alert(1)"] {
            assert!(!super::url_ok(bad), "{bad}");
        }
    }

    #[test]
    fn a_printed_path_is_under_the_session_folder() {
        use std::path::Path;
        let (cwd, home) = (Path::new("/w/proj"), Path::new("/home/me"));
        assert_eq!(super::resolve("src/main.rs", cwd, Some(home)), cwd.join("src/main.rs"));
        assert_eq!(super::resolve("~/notes.md", cwd, Some(home)), home.join("notes.md"));
        assert_eq!(super::resolve("~", cwd, Some(home)), home);
        assert_eq!(super::resolve("/etc/hosts", cwd, Some(home)), Path::new("/etc/hosts"));
    }
}
