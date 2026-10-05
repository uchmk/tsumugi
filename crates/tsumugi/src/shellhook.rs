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
}
