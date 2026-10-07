//! `tsumugi shell-hook`: the lines that make a shell say which folder it is
//! in (OSC 7), as filer's `filer shell-hook` does for its pane.
//!
//! On Linux and macOS the server can read a shell's folder from the system,
//! so the hook only makes it quicker. On Windows it cannot, and without the
//! hook a tab comes back after a restart in the folder it was opened in,
//! not the one it was in. Add the output to the shell's profile:
//! `tsumugi shell-hook pwsh >> $PROFILE`.
//!
//! Each starts with an empty line, so appending it to a file that does not
//! end in a newline does not glue it onto that file's last line.

/// PowerShell 7. Runs on each `cd` rather than replacing `prompt`, which
/// would break Starship and the other prompt generators, and calls whatever
/// handler was there before, which `mise activate pwsh` puts there.
pub const PWSH: &str = r#"
# tsumugi: say where the shell is (OSC 7), for the sidebar and for a restore
$prev = $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction
$ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = {
    param($sender, $e)
    if ($prev) { $prev.Invoke($sender, $e) }
    $p = $e.NewPath.ProviderPath -replace '\\', '/' -replace '^(?!/)', '/'
    [Console]::Write("$([char]27)]7;file://$p$([char]27)\")
}.GetNewClosure()
"#;

/// bash has no hook on `cd`, so this says it at every prompt; tsumugi only acts
/// on a folder that differs from the one it has.
pub const BASH: &str = r#"
# tsumugi: say where the shell is (OSC 7), for the sidebar and for a restore
__tsumugi_osc7() { printf '\e]7;file://%s%s\e\\' "$HOSTNAME" "$PWD"; }
PROMPT_COMMAND="__tsumugi_osc7${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
"#;

/// zsh has one: `chpwd`. It does not fire for the directory the shell starts
/// in, hence the call at the end.
pub const ZSH: &str = r#"
# tsumugi: say where the shell is (OSC 7), for the sidebar and for a restore
__tsumugi_osc7() { printf '\e]7;file://%s%s\e\\' "$HOST" "$PWD" }
autoload -Uz add-zsh-hook
add-zsh-hook chpwd __tsumugi_osc7
__tsumugi_osc7
"#;

/// The shells there is a hook for, as `shell-hook` takes them.
pub const SHELLS: &str = "pwsh, bash, zsh";

/// The hook for `shell`, by the name its program goes by. With none named,
/// PowerShell's: it is the default shell on Windows, and the one shell that
/// sends nothing unless told to.
pub fn text(shell: Option<&str>) -> Result<&'static str, String> {
    match shell.map(str::to_ascii_lowercase).as_deref() {
        None | Some("pwsh" | "pwsh.exe") => Ok(PWSH),
        Some("bash" | "bash.exe") => Ok(BASH),
        Some("zsh") => Ok(ZSH),
        Some("powershell" | "powershell.exe") => Err(
            "Windows PowerShell 5.1 has no LocationChangedAction; the hook needs PowerShell 7 (pwsh)".into(),
        ),
        Some(other) => Err(format!("no hook for {other:?} (one of: {SHELLS})")),
    }
}

/// The lines that mark the hook in a profile, so it can be found and taken
/// out again (Settings, Shell & hooks).
const BEGIN: &str = "# >>> tsumugi shell integration >>>";
const END: &str = "# <<< tsumugi shell integration <<<";

/// The profile with the hook between the marks: added at the end, or put in
/// place of the one there.
pub fn with_hook(profile: &str, hook: &str) -> String {
    let mut out = without_hook(profile);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(BEGIN);
    out.push_str(hook);
    out.push_str(END);
    out.push('\n');
    out
}

/// The profile with the marked hook taken out.
pub fn without_hook(profile: &str) -> String {
    let (Some(a), Some(b)) = (profile.find(BEGIN), profile.find(END)) else { return profile.to_string() };
    if b < a {
        return profile.to_string();
    }
    let rest = &profile[b + END.len()..];
    let mut out = profile[..a].to_string();
    out.push_str(rest.strip_prefix('\n').unwrap_or(rest));
    out
}

/// The marked hook is in the profile.
pub fn has_hook(profile: &str) -> bool {
    profile.contains(BEGIN)
}

/// The profile file a shell reads at start, by the shell's program name.
/// PowerShell is asked for its `$PROFILE` (it moves with OneDrive), so this
/// is a thread's work.
pub fn profile(shell: &str) -> Option<std::path::PathBuf> {
    let home = tsumugi_mux::settings::home()?;
    let name = std::path::Path::new(shell).file_stem()?.to_string_lossy().to_ascii_lowercase();
    match name.as_str() {
        "bash" => Some(home.join(".bashrc")),
        "zsh" => Some(std::env::var_os("ZDOTDIR").map(std::path::PathBuf::from).unwrap_or(home).join(".zshrc")),
        "pwsh" => {
            let mut cmd = std::process::Command::new(shell);
            cmd.args(["-NoLogo", "-NoProfile", "-Command", "$PROFILE.CurrentUserCurrentHost"]).stdin(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            let asked = cmd.output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|p| !p.is_empty());
            asked.map(std::path::PathBuf::from).or_else(|| {
                Some(if cfg!(windows) { home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1") } else { home.join(".config/powershell/Microsoft.PowerShell_profile.ps1") })
            })
        }
        _ => None,
    }
}

/// Add the hook to the shell's profile, or take it out (a thread's work).
pub fn set_installed(shell: &str, on: bool) -> Result<std::path::PathBuf, String> {
    let name = std::path::Path::new(shell).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let hook = text(Some(&name))?;
    let path = profile(shell).ok_or_else(|| format!("no profile known for {name}"))?;
    // A profile that does not read as UTF-8 (Windows PowerShell 5.1 saves
    // UTF-16) is refused, not replaced by the hook alone; and the old one
    // is kept beside it (the source review, 2026-10-07).
    let old = crate::files::read_or_empty(&path)?;
    let new = if on { with_hook(&old, hook) } else { without_hook(&old) };
    if new != old {
        if !old.is_empty() {
            let mut backup = path.as_os_str().to_owned();
            backup.push(".tsumugi-backup");
            crate::files::write_atomic(std::path::Path::new(&backup), &old).map_err(|e| format!("the backup: {e}"))?;
        }
        crate::files::write_atomic(&path, new)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shell_gets_its_own_and_the_rest_are_refused() {
        assert_eq!(text(None), Ok(PWSH));
        assert_eq!(text(Some("bash")), Ok(BASH));
        assert_eq!(text(Some("zsh")), Ok(ZSH));
        assert!(text(Some("powershell")).unwrap_err().contains("PowerShell 7"));
        assert!(text(Some("fish")).unwrap_err().contains(SHELLS));
        for hook in [PWSH, BASH, ZSH] {
            assert!(hook.starts_with("\n#") && hook.ends_with('\n') && hook.contains("]7;file://"), "{hook:?}");
            assert!(!hook.contains("filer"), "{hook:?}");
        }
    }

    #[test]
    fn the_marked_hook_goes_in_once_and_comes_out_clean() {
        let mine = "alias ll='ls -l'";
        let once = with_hook(mine, BASH);
        assert!(has_hook(&once) && once.starts_with("alias ll='ls -l'\n# >>> tsumugi"), "{once}");
        assert_eq!(with_hook(&once, BASH), once, "twice is once");
        assert_eq!(without_hook(&once), "alias ll='ls -l'\n");
        assert_eq!(without_hook(mine), mine);
        assert!(has_hook(&with_hook("", ZSH)));
    }
}
