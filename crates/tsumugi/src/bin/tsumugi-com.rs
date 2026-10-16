//! `tsumugi.com`: the console-subsystem front for `tsumugi.exe`, the same as
//! kura's `kura.com` and yagura's `yagura.com`.
//!
//! `tsumugi.exe` is a windowed program, so PowerShell does not wait for it
//! at the end of a pipeline: `$id = & tsumugi new` and `$v = & tsumugi
//! --version` came back empty, and `tsumugi ls` printed after the next
//! prompt. The release zip ships this binary beside `tsumugi.exe` as
//! `tsumugi.com`. `PATHEXT` lists `.COM` before `.EXE`, so typing `tsumugi`
//! finds this one first. It is the shape of Visual Studio's `devenv.com` and
//! `devenv.exe`.
//!
//! For every command but the window's, it runs `tsumugi.exe` with the same
//! arguments and the same standard handles, waits, and passes on the exit
//! code. Being a console program, it is what the shell waits for. That
//! takes in the long-running ones too (`server`, `mcp`, `proxy`, `wait`,
//! `serial`, `tmux`): they run in the foreground, as on Linux. With no
//! argument, `--keys` or `attach` it starts `tsumugi.exe` and returns as
//! soon as the window is taking input, much as typing `tsumugi.exe` always
//! did. It waits that long, not less, so that a command line `tsumugi.exe`
//! refuses before opening a window (`--keys "<Tab"`, `attach` with no such
//! session) still prints its one line here and passes on its exit code.
//!
//! The window never gets a pipe of this program's. A reader of a pipe waits
//! until every process holding it has let go, which with the window holding
//! it was until the window closed (kura's #192); and the window writes now
//! and then (`TSUMUGI_KEYLOG=1`, the alert log), where a pipe nobody reads
//! any more would panic it. So the window writes to this console (or the
//! file the output went to), or nowhere.
//!
//! It holds no logic of its own beyond that choice: the help, the report
//! and the answers all come from `tsumugi.exe`, so the two cannot disagree.

/// The first arguments that open the window rather than print and exit.
/// Kept in step with `main` in `main.rs`.
const OPENS_WINDOW: &[&str] = &["--keys", "attach"];

/// Whether `tsumugi.exe` will answer this command line and exit (however
/// long it runs) rather than open a window. `main` reads only the first
/// argument; an unknown one is refused in text.
fn answers_in_text(args: &[std::ffi::OsString]) -> bool {
    args.first().is_some_and(|a| !OPENS_WINDOW.contains(&a.to_str().unwrap_or("")))
}

#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let exe = match std::env::current_exe() {
        Ok(me) => me.with_file_name("tsumugi.exe"),
        Err(e) => {
            eprintln!("tsumugi.com: cannot tell where it is: {e}");
            std::process::exit(2);
        }
    };
    if !exe.is_file() {
        eprintln!("tsumugi.com: no tsumugi.exe beside it ({})", exe.display());
        std::process::exit(2);
    }
    // Before either kind of start: `Command` hands `tsumugi.exe` every
    // inheritable handle, not only the three it is given, and from there
    // they reach the server `new` starts. Holding the pipe of `$id =
    // tsumugi new`, the server kept PowerShell waiting until it went.
    keep_own_handles();
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args);
    if answers_in_text(&args) {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(e) => {
                eprintln!("tsumugi.com: {}: {e}", exe.display());
                std::process::exit(2);
            }
        }
    }
    cmd.stdin(std::process::Stdio::null()).stdout(for_the_window(Out::Stdout)).stderr(for_the_window(Out::Stderr));
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tsumugi.com: {}: {e}", exe.display());
            std::process::exit(2);
        }
    };
    std::process::exit(until_the_window_is_up(&mut child));
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum Out {
    Stdout,
    Stderr,
}

/// Makes this program's standard handles stay here. `Command` duplicates
/// the ones it is told to inherit, so `cmd.status()` still passes them on.
#[cfg(windows)]
fn keep_own_handles() {
    use windows::Win32::Foundation::{SetHandleInformation, HANDLE_FLAGS, HANDLE_FLAG_INHERIT};
    use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};

    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        if let Ok(h) = unsafe { GetStdHandle(which) } {
            if !h.is_invalid() {
                let _ = unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) };
            }
        }
    }
}

/// Where the window writes: this program's own output when it is a console
/// or a file, which the window may keep as long as it likes; else (a pipe,
/// or none) this console, or nowhere when there is none.
#[cfg(windows)]
fn for_the_window(which: Out) -> std::process::Stdio {
    use windows::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_CHAR, FILE_TYPE_DISK};
    use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    let std = match which {
        Out::Stdout => STD_OUTPUT_HANDLE,
        Out::Stderr => STD_ERROR_HANDLE,
    };
    if let Ok(h) = unsafe { GetStdHandle(std) } {
        if !h.is_invalid() {
            let kind = unsafe { GetFileType(h) };
            if kind == FILE_TYPE_CHAR || kind == FILE_TYPE_DISK {
                return std::process::Stdio::inherit();
            }
        }
    }
    match std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$") {
        Ok(console) => console.into(),
        Err(_) => std::process::Stdio::null(),
    }
}

/// Waits for the window to start taking input, then lets it be: the prompt
/// comes back with tsumugi still open. If `tsumugi.exe` exits first, it
/// refused the command line, and its exit code is this one's. A refusal
/// comes before any window; the cap is for a machine so slow that neither
/// has happened yet.
#[cfg(windows)]
fn until_the_window_is_up(child: &mut std::process::Child) -> i32 {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{WaitForInputIdle, WaitForSingleObject};

    let process = HANDLE(child.as_raw_handle());
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code().unwrap_or(1),
            Ok(None) => {}
            Err(_) => return 0,
        }
        match unsafe { WaitForInputIdle(process, 50) } {
            // Idle: its message loop is running and waiting for input -- or
            // it has exited, which WaitForInputIdle also answers with 0
            // (kura's CI caught that). So look again, giving an exit that is
            // already under way a moment to land.
            0 => {
                if unsafe { WaitForSingleObject(process, 250) } == WAIT_OBJECT_0 {
                    return match child.try_wait() {
                        Ok(Some(status)) => status.code().unwrap_or(1),
                        _ => 1,
                    };
                }
                return 0;
            }
            // WAIT_TIMEOUT: not yet.
            0x102 => {}
            // WAIT_FAILED: no message queue to wait on, which is a debug
            // build (a console program). Give a refusal its moment, then go.
            _ => {
                std::thread::sleep(std::time::Duration::from_millis(300));
                return match child.try_wait() {
                    Ok(Some(status)) => status.code().unwrap_or(1),
                    _ => 0,
                };
            }
        }
        // `attach` asks the server for the session before the window
        // starts, which can take a while on a cold machine.
        if start.elapsed() > std::time::Duration::from_secs(10) {
            return 0;
        }
    }
}

#[cfg(not(windows))]
fn main() {
    let _ = answers_in_text;
    eprintln!("tsumugi-com is the console front for tsumugi.exe, and only does anything on Windows");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Vec<std::ffi::OsString> {
        args.iter().map(Into::into).collect()
    }

    /// Every command is waited for but the ones that open the window.
    #[test]
    fn it_waits_for_all_but_the_window() {
        for waits in [
            &["ls"][..],
            &["--version"],
            &["-V"],
            &["help"],
            &["--help"],
            &["env"],
            &["shell-hook", "pwsh"],
            &["new", "--", "claude"],
            &["layout", "list"],
            &["send", "3", "attach"],
            &["focus", "3"],
            &["wait", "3"],
            &["server"],
            &["proxy"],
            &["mcp"],
            &["--bogus"],
            &["attachment"],
        ] {
            assert!(answers_in_text(&line(waits)), "{waits:?}");
        }
        for opens in [&[][..], &["--keys", "<C-S-t>"], &["--keys"], &["attach", "3"]] {
            assert!(!answers_in_text(&line(opens)), "{opens:?}");
        }
    }
}
