//! Starting the server as a process of its own, cut loose from the window,
//! so that closing the window -- or the terminal the window was started
//! from -- does not take the sessions with it.

use std::io;
use std::process::{Command, Stdio};

pub fn server() -> io::Result<()> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("server").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    detach(&mut cmd).spawn().map(drop)
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
