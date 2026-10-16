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
//! The front itself is ito's `ito_console::front` (v0.114.0), shared with
//! kura, yagura, minato and michi; what is tsumugi's is which command lines
//! answer in text. For every command but the window's, it runs
//! `tsumugi.exe` with the same arguments and the same standard handles,
//! waits, and passes on the exit code. That takes in the long-running ones
//! too (`server`, `mcp`, `proxy`, `wait`, `serial`, `tmux`): they run in the
//! foreground, as on Linux. With no argument, `--keys` or `attach` it starts
//! `tsumugi.exe` and returns as soon as the window is taking input, so that
//! a command line `tsumugi.exe` refuses before opening a window (`--keys
//! "<Tab"`, `attach` with no such session) still prints its one line here
//! and passes on its exit code.
//!
//! Neither the window nor a server `new` starts gets a pipe of this
//! program's: a reader of a pipe waits until every process holding it has
//! let go (kura's #192). From a pipe, the window writes (`TSUMUGI_KEYLOG=1`,
//! the alert log) into a file of its own in the temporary folder, copied out
//! here once it is up, and gone when it closes.
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

fn main() {
    ito_console::front("tsumugi", answers_in_text)
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
