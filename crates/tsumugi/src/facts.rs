//! What the Settings screen states about the machine rather than the file:
//! the shells installed, whether Claude Code's hooks and the shell
//! integration are in, whether the server starts at sign-in, whether the
//! newer ConPTY sits beside the exe. Each asks the disk or starts a
//! program, so they are read on a thread when the screen opens and after
//! each change it makes.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// The shells found on `PATH`: (what the list says, the program).
    pub shells: Vec<(String, String)>,
    pub hooks: bool,
    /// The shell the sessions start, and whether its profile has the hook
    /// (`None` when there is no hook for that shell).
    pub shell: String,
    pub shell_hook: Option<bool>,
    pub autostart: bool,
    /// `conpty.dll` and `OpenConsole.exe` are beside the exe (Windows).
    pub conpty: bool,
}

/// Read them on a thread of its own.
pub fn read(shell: Option<String>, wake: impl Fn() + Send + 'static) -> Receiver<Facts> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("facts".into()).spawn(move || {
        let _ = tx.send(gather(shell));
        wake();
    });
    rx
}

fn gather(shell: Option<String>) -> Facts {
    let shell = shell.filter(|s| !s.is_empty()).unwrap_or_else(ito_pane::default_program);
    let shell_hook = crate::shellhook::profile(&shell).map(|p| std::fs::read_to_string(p).is_ok_and(|t| crate::shellhook::has_hook(&t)));
    let shell_hook = shell_hook.filter(|_| crate::shellhook::text(std::path::Path::new(&shell).file_stem().and_then(|s| s.to_str())).is_ok());
    let conpty = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).is_some_and(|d| d.join("conpty.dll").is_file() && d.join("OpenConsole.exe").is_file());
    Facts { shells: installed_shells(), hooks: crate::hooks::installed(), shell, shell_hook, autostart: crate::autostart::is_on(), conpty }
}

/// The shells there are, in the order the list shows them.
const KNOWN: [(&str, &str); 8] = [
    ("PowerShell 7", "pwsh"),
    ("Windows PowerShell", "powershell"),
    ("Command Prompt", "cmd"),
    ("bash", "bash"),
    ("zsh", "zsh"),
    ("fish", "fish"),
    ("nushell", "nu"),
    ("sh", "sh"),
];

fn installed_shells() -> Vec<(String, String)> {
    KNOWN.iter().filter(|(_, p)| locate(p)).map(|(l, p)| (l.to_string(), p.to_string())).collect()
}

pub fn locate(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat"] } else { &[""] };
    std::env::split_paths(&path).any(|dir| exts.iter().any(|e| dir.join(format!("{program}{e}")).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_on_the_path_is_found() {
        // `sh` is on every test machine but Windows; `cmd` on Windows.
        let found = installed_shells();
        let want = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(found.iter().any(|(_, p)| p == want), "{found:?}");
        assert!(!locate("no-such-shell-tsumugi"));
    }
}
