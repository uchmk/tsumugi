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
        let line = with_app_path(&line);
        let mut cmd = shell(&line);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(mut child) = cmd.spawn() {
            let _ = child.wait();
        }
    });
}

/// `line` with a first word that names another uchmk app (`kura {folder}`)
/// swapped for the exe that app registered or was pinned to, so it need not
/// be on `PATH`. Any other line stays as it is.
fn with_app_path(line: &str) -> String {
    let (first, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    if !ito_common::apps::valid_name(first) {
        return line.to_owned();
    }
    let Some(found) = ito_common::apps::pinned(first) else { return line.to_owned() };
    let exe = found.exe.to_string_lossy();
    let quoted = if cfg!(windows) { format!("\"{exe}\"") } else { format!("'{}'", exe.replace('\'', "'\\''")) };
    format!("{quoted} {rest}")
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

/// The path a `file://` address names: the host (`ls --hyperlink` puts the
/// machine's name there) dropped, `%20` and the like decoded, and on Windows
/// `/C:/x` as `C:/x`. `None` for any other address.
pub fn file_uri(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |k: usize| bytes.get(k).and_then(|b| (*b as char).to_digit(16));
        match (bytes[i], hex(i + 1), hex(i + 2)) {
            (b'%', Some(a), Some(b)) => {
                out.push((a * 16 + b) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    let path = String::from_utf8_lossy(&out).into_owned();
    Some(if cfg!(windows) && path.get(2..3) == Some(":") { path[1..].to_owned() } else { path })
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
/// the line and column, or the system's own way when that is empty, it is
/// a folder, or its extension is one of `[open] system`. Looks at the disk on
/// a thread; what went wrong goes to `told`.
pub fn open_path(
    path: PathBuf,
    line: Option<u32>,
    column: Option<u32>,
    open: tsumugi_mux::settings::Open,
    told: Sender<Result<String, String>>,
) {
    let _ = std::thread::Builder::new().name("open-path".into()).spawn(move || {
        let Ok(meta) = std::fs::metadata(&path) else {
            let _ = told.send(Err(format!("No such file: {}", path.display())));
            return;
        };
        let command = &open.file;
        if meta.is_dir() || command.trim().is_empty() || open.opens_with_system(&path) {
            open_with_system(&path);
        } else {
            run(tsumugi_mux::settings::fill_file(command, &path, line.unwrap_or(1), column.unwrap_or(1)));
        }
    });
}

/// The environment variable a new window reads to show a tab first.
pub const SHOW_TAB: &str = "TSUMUGI_SHOW_TAB";

/// The environment variable a new window reads to open at a place on the
/// screen: `x,y,scale` (a tab dropped outside the window it was dragged
/// from), as [`At`] says. `x,y` alone is read at scale 0.
pub const WINDOW_AT: &str = "TSUMUGI_WINDOW_AT";

/// Where a new window opens: its top left in logical pixels, and the points
/// to pixels of the screen they were taken on, without the app's zoom. With
/// the scale the window can be put on those pixels even when it is made on
/// a screen of another scale (#181); at 0 it is made at the corner and not
/// moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct At {
    pub pos: [f32; 2],
    pub scale: f32,
}

/// Where this window was asked to open, if anywhere.
pub fn window_at() -> Option<At> {
    std::env::var(WINDOW_AT).ok().and_then(|v| parse_at(&v))
}

fn parse_at(v: &str) -> Option<At> {
    let mut parts = v.split(',').map(|p| p.trim().parse::<f32>().ok());
    let (x, y) = (parts.next()??, parts.next()??);
    let scale = match parts.next() {
        Some(s) => s?,
        None => 0.0,
    };
    let fine = [x, y, scale].iter().all(|v| v.is_finite()) && scale >= 0.0 && parts.next().is_none();
    fine.then_some(At { pos: [x, y], scale })
}

/// [`At`] as [`WINDOW_AT`] says it.
fn at_text(at: At) -> String {
    format!("{},{},{}", at.pos[0].round(), at.pos[1].round(), at.scale)
}

/// Another window of this program, showing `tab`; at `at` on the screen, or
/// where the system puts a new one.
pub fn new_window(tab: WorkspaceId, at: Option<At>) {
    let _ = std::thread::Builder::new().name("new-window".into()).spawn(move || {
        let Ok(exe) = std::env::current_exe() else { return };
        let mut cmd = Command::new(exe);
        cmd.env(SHOW_TAB, tab.to_string()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        // Not where this window was asked to open, which it would inherit.
        match at {
            Some(at) => cmd.env(WINDOW_AT, at_text(at)),
            None => cmd.env_remove(WINDOW_AT),
        };
        if let Ok(mut child) = cmd.spawn() {
            let _ = child.wait();
        }
    });
}

#[cfg(test)]
mod tests {
    /// A first word that is not an app name leaves the line alone.
    #[test]
    fn a_line_without_an_app_name_is_unchanged() {
        assert_eq!(super::with_app_path("code --goto {file}"), "code --goto {file}");
        assert_eq!(super::with_app_path(r"C:xy.exe a"), r"C:xy.exe a");
    }

    /// A line starting with a quoted program keeps every quote: only the
    /// outer pair `/S` takes off is added.
    #[test]
    fn a_line_with_quotes_goes_to_cmd_whole() {
        let line = r#""C:\Program Files\Code\Code.exe" "C:\dev\x""#;
        assert_eq!(super::windows_line(line), format!("/S /C \"{line}\""));
    }

    #[test]
    fn a_window_reads_where_to_open() {
        use super::{at_text, parse_at, At};
        assert_eq!(parse_at("120,-40,1.5"), Some(At { pos: [120.0, -40.0], scale: 1.5 }));
        assert_eq!(parse_at(" 8 , 9 , 1 "), Some(At { pos: [8.0, 9.0], scale: 1.0 }));
        assert_eq!(parse_at("8,9"), Some(At { pos: [8.0, 9.0], scale: 0.0 }), "the old form: not moved");
        assert_eq!(parse_at("8"), None);
        assert_eq!(parse_at("x,9"), None);
        assert_eq!(parse_at("NaN,9"), None);
        assert_eq!(parse_at("8,9,-1"), None);
        assert_eq!(parse_at("8,9,inf"), None);
        assert_eq!(parse_at("8,9,"), None);
        assert_eq!(parse_at("8,9,1.5,2"), None);
        // What a window on a screen at 125 % sends, read back by the new one.
        let sent = at_text(At { pos: [3000.4, -199.6], scale: 1.25 });
        assert_eq!(sent, "3000,-200,1.25");
        assert_eq!(parse_at(&sent), Some(At { pos: [3000.0, -200.0], scale: 1.25 }));
    }

    #[test]
    fn only_plain_web_addresses_reach_the_shell() {
        assert!(super::url_ok("https://github.com/o/r/pull/12"));
        assert!(super::url_ok("http://localhost:5173/a?b=c"));
        assert_eq!(super::file_uri("file:///home/u/a%20b.txt").as_deref(), Some("/home/u/a b.txt"));
        assert_eq!(super::file_uri("file://box/home/u/%E3%81%82").as_deref(), Some("/home/u/あ"), "the host is dropped");
        assert_eq!(super::file_uri("file://box").as_deref(), None);
        assert_eq!(super::file_uri("https://x.example/").as_deref(), None);
        assert!(super::url_ok("http://127.0.0.1:8080"));
        for bad in
            ["file:///etc/passwd", "https://", "https://a.b/x&calc", "https://a.b/'$(x)'", "https://a.b/%PATH%", "javascript:alert(1)"]
        {
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
