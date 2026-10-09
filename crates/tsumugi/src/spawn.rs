//! Starting the server as a process of its own, cut loose from the window,
//! so that closing the window -- or the terminal the window was started
//! from -- does not take the sessions with it.

use std::io;
use std::process::{Command, Stdio};

pub fn server() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let run = server_exe(&exe).unwrap_or(exe);
    let mut cmd = Command::new(run);
    cmd.arg("server").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    keep_std_handles();
    detach(&mut cmd).spawn().map(drop)
}

/// Windows hands every inheritable handle to a child, not only the three it
/// is given, and the std handles a process was started with are inheritable.
/// So the server held on to the pipe of whoever ran `tsumugi new` and read its
/// output (`$id = tsumugi new` in PowerShell, a test's `output()`): the read
/// waited for the server to stop. Children this process starts later with
/// inherited stdio still get them: those are duplicated for the child.
#[cfg(windows)]
fn keep_std_handles() {
    use windows::Win32::Foundation::{HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};
    use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        if let Ok(h) = unsafe { GetStdHandle(which) } {
            if !h.is_invalid() {
                let _ = unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) };
            }
        }
    }
}

#[cfg(not(windows))]
fn keep_std_handles() {}

/// On Windows, the server runs from a copy of the exe: a running exe can
/// be neither replaced nor removed there, so a server started from
/// `target\debug\tsumugi.exe` stopped the next `cargo build`, and an update
/// could not replace the installed one while sessions ran. The copy is
/// named by the version and the build (the exe's size and time), so a new
/// build starts a new copy; the ConPTY beside the exe goes beside it too,
/// since only the exe's own folder is searched for it. Copies no server runs
/// any more are removed; one a server still runs cannot be, and stays.
/// `None`, and the exe itself, when the copy cannot be made.
#[cfg(windows)]
fn server_exe(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::time::UNIX_EPOCH;
    let dir = tsumugi_mux::state::default_path()?.parent()?.join("server");
    std::fs::create_dir_all(&dir).ok()?;
    let meta = std::fs::metadata(exe).ok()?;
    let when = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let name = format!("tsumugi-{}-{:x}.exe", env!("CARGO_PKG_VERSION"), when ^ meta.len().rotate_left(32));
    let copy = dir.join(&name);
    if !copy.exists() {
        let aside = dir.join(format!("{name}.new"));
        std::fs::copy(exe, &aside).ok()?;
        std::fs::rename(&aside, &copy).ok()?;
    }
    if let Some(from) = exe.parent() {
        for side in ["conpty.dll", "OpenConsole.exe"] {
            let (src, dst) = (from.join(side), dir.join(side));
            let same = |a: &std::path::Path, b: &std::path::Path| {
                let (Ok(a), Ok(b)) = (std::fs::metadata(a), std::fs::metadata(b)) else { return false };
                a.len() == b.len() && a.modified().ok() == b.modified().ok()
            };
            // In use by a server still running: it stays as it is.
            if src.exists() && !same(&src, &dst) {
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("tsumugi-") && n != name {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Some(copy)
}

/// Elsewhere a running program can be replaced, and the server runs from
/// the exe itself.
#[cfg(not(windows))]
fn server_exe(_exe: &std::path::Path) -> Option<std::path::PathBuf> {
    None
}

#[cfg(windows)]
fn detach(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    // No console of its own (the window has none), and outside the window's
    // console group, so a Ctrl+C there does not reach it.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
}

#[cfg(unix)]
fn detach(cmd: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    // A process group of its own: the hangup a closing terminal sends its
    // foreground group does not reach it.
    cmd.process_group(0)
}

#[cfg(not(any(windows, unix)))]
fn detach(cmd: &mut Command) -> &mut Command {
    cmd
}
