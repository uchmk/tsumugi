//! The console a pane's program has, raw: what `tsumugi serial` and
//! `tsumugi tmux-pane` pass bytes through. Keys come as they are typed (VT
//! sequences on Windows), nothing is echoed, `Ctrl+C` is a byte; the screen
//! takes UTF-8 and VT.

use std::io::{Read, Write};

/// The keyboard, the screen, and what puts the console back as it was.
pub type Console<G> = (Box<dyn Read + Send>, Box<dyn Write + Send>, G);

pub use imp::{raw, size};

#[cfg(windows)]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, DISABLE_NEWLINE_AUTO_RETURN, ENABLE_PROCESSED_OUTPUT, ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_WRAP_AT_EOL_OUTPUT,
        GetConsoleCP, GetConsoleMode, GetConsoleOutputCP, GetConsoleScreenBufferInfo, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
    };

    fn handle(f: &File) -> HANDLE {
        HANDLE(f.as_raw_handle())
    }

    /// The console's modes and code pages as they were.
    pub struct Guard {
        input: File,
        output: File,
        modes: (CONSOLE_MODE, CONSOLE_MODE),
        cps: (u32, u32),
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            // SAFETY: the handles are the console's, open until the guard goes.
            unsafe {
                let _ = SetConsoleMode(handle(&self.input), self.modes.0);
                let _ = SetConsoleMode(handle(&self.output), self.modes.1);
                let _ = SetConsoleCP(self.cps.0);
                let _ = SetConsoleOutputCP(self.cps.1);
            }
        }
    }

    /// The console's own input and output (a GUI program's standard handles
    /// are not set after `AttachConsole`), raw: keys as VT sequences, no
    /// echo, no Ctrl+C; the output in UTF-8 and VT.
    pub fn raw() -> std::io::Result<super::Console<Guard>> {
        let input = OpenOptions::new().read(true).write(true).open("CONIN$")?;
        let output = OpenOptions::new().read(true).write(true).open("CONOUT$")?;
        let mut mi = CONSOLE_MODE::default();
        let mut mo = CONSOLE_MODE::default();
        // SAFETY: the handles are the console's; the modes are ours.
        let guard = unsafe {
            GetConsoleMode(handle(&input), &mut mi)?;
            GetConsoleMode(handle(&output), &mut mo)?;
            let guard = Guard { input: input.try_clone()?, output: output.try_clone()?, modes: (mi, mo), cps: (GetConsoleCP(), GetConsoleOutputCP()) };
            SetConsoleMode(handle(&input), ENABLE_VIRTUAL_TERMINAL_INPUT)?;
            SetConsoleMode(handle(&output), ENABLE_PROCESSED_OUTPUT | ENABLE_WRAP_AT_EOL_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN)?;
            let _ = SetConsoleCP(65001);
            let _ = SetConsoleOutputCP(65001);
            guard
        };
        Ok((Box::new(input), Box::new(output), guard))
    }

    /// The console's columns and rows (its window, not its buffer).
    pub fn size() -> Option<(u16, u16)> {
        let output = OpenOptions::new().read(true).write(true).open("CONOUT$").ok()?;
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        // SAFETY: the handle is the console's; the structure is ours.
        unsafe { GetConsoleScreenBufferInfo(handle(&output), &mut info).ok()? };
        let w = info.srWindow.Right - info.srWindow.Left + 1;
        let h = info.srWindow.Bottom - info.srWindow.Top + 1;
        (w > 0 && h > 0).then_some((w as u16, h as u16))
    }
}

#[cfg(unix)]
mod imp {
    fn check(r: libc::c_int) -> std::io::Result<()> {
        if r == -1 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
    }

    /// The terminal's settings as they were, when it is one.
    pub struct Guard {
        was: Option<libc::termios>,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(t) = self.was {
                // SAFETY: stdin's descriptor; the termios is ours.
                unsafe {
                    libc::tcsetattr(0, libc::TCSANOW, &t);
                }
            }
        }
    }

    /// Standard input and output, the terminal raw while it is one.
    pub fn raw() -> std::io::Result<super::Console<Guard>> {
        let mut guard = Guard { was: None };
        // SAFETY: stdin's descriptor; the termios is ours.
        unsafe {
            if libc::isatty(0) == 1 {
                let mut t: libc::termios = std::mem::zeroed();
                check(libc::tcgetattr(0, &mut t))?;
                guard.was = Some(t);
                libc::cfmakeraw(&mut t);
                check(libc::tcsetattr(0, libc::TCSANOW, &t))?;
            }
        }
        Ok((Box::new(std::io::stdin()), Box::new(std::io::stdout()), guard))
    }

    /// The terminal's columns and rows.
    pub fn size() -> Option<(u16, u16)> {
        // SAFETY: stdout's descriptor; the winsize is ours.
        let ws = unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            (libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0).then_some(ws)?
        };
        (ws.ws_col > 0 && ws.ws_row > 0).then_some((ws.ws_col, ws.ws_row))
    }
}
