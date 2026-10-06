//! "Start the server at sign-in" (Settings, General): the server is started
//! by the system when the user signs in, so the sessions of a restore are
//! there before the window is opened. Not kept in `settings.toml`: the
//! system's own list is the truth, and is what the switch reads.
//!
//! - Windows: a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//! - macOS: a LaunchAgent, `~/Library/LaunchAgents/dev.tsumugi.server.plist`.
//! - Linux: an XDG autostart entry, `~/.config/autostart/tsumugi-server.desktop`.

use std::path::{Path, PathBuf};

/// The server is started at sign-in.
pub fn is_on() -> bool {
    imp::is_on()
}

/// Start the server at sign-in, or stop doing so.
pub fn set(on: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    imp::set(on, &exe)
}

/// The XDG autostart entry for `exe`.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=tsumugi server\nComment=Keeps tsumugi's sessions running\nExec=\"{}\" server\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    )
}

/// The LaunchAgent for `exe`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launch_agent(exe: &Path) -> String {
    let exe = exe.display().to_string().replace('&', "&amp;").replace('<', "&lt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>dev.tsumugi.server</string>\n  <key>ProgramArguments</key><array><string>{exe}</string><string>server</string></array>\n  <key>RunAtLoad</key><true/>\n</dict>\n</plist>\n"
    )
}

/// Where a file-based entry goes (macOS and Linux).
#[cfg_attr(windows, allow(dead_code))]
fn entry_file() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        return Some(home.join("Library/LaunchAgents/dev.tsumugi.server.plist"));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config"));
    Some(config.join("autostart/tsumugi-server.desktop"))
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("reg").args(args).stdin(Stdio::null()).creation_flags(CREATE_NO_WINDOW).output()
    }

    pub fn is_on() -> bool {
        reg(&["query", KEY, "/v", "tsumugi"]).is_ok_and(|o| o.status.success())
    }

    pub fn set(on: bool, exe: &Path) -> Result<(), String> {
        let line = format!("\"{}\" server", exe.display());
        let out = if on { reg(&["add", KEY, "/v", "tsumugi", "/t", "REG_SZ", "/d", &line, "/f"]) } else { reg(&["delete", KEY, "/v", "tsumugi", "/f"]) };
        match out {
            Ok(o) if o.status.success() || !on => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn is_on() -> bool {
        super::entry_file().is_some_and(|p| p.is_file())
    }

    pub fn set(on: bool, exe: &Path) -> Result<(), String> {
        let file = super::entry_file().ok_or("no home folder")?;
        if !on {
            return match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let text = if cfg!(target_os = "macos") { super::launch_agent(exe) } else { super::desktop_entry(exe) };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entries_start_the_server() {
        let exe = Path::new("/opt/tsumugi/tsumugi");
        let d = desktop_entry(exe);
        assert!(d.contains("Exec=\"/opt/tsumugi/tsumugi\" server"), "{d}");
        let p = launch_agent(Path::new("/Apps/a&b/tsumugi"));
        assert!(p.contains("<string>/Apps/a&amp;b/tsumugi</string><string>server</string>"), "{p}");
        assert!(p.contains("<key>RunAtLoad</key><true/>"));
    }
}
