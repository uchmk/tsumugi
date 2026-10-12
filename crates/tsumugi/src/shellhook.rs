//! `tsumugi shell-hook`: the lines that make a shell say which folder it is
//! in (OSC 7), as kura's `kura shell-hook` does for its pane, and mark
//! where each prompt starts (OSC 133;A) for `Ctrl+Shift+Up` / `Down`, with
//! how the command before it ended (`D;<code>`) and, where the shell can
//! say it, where its output starts (`C`) or where the prompt ends and the
//! command is typed (`B`): the bars beside each command and `Ctrl+Shift+L`.
//!
//! The lines and the marked insert into a profile are ito's
//! `ito_common::shellhook`, shared with kura; what stays here is the app's
//! name, where the profile lives and how it is written.
//!
//! On Linux and macOS the server can read a shell's folder from the system,
//! so the hook only makes it quicker. On Windows it cannot, and without the
//! hook a tab comes back after a restart in the folder it was opened in,
//! not the one it was in. Add the output to the shell's profile:
//! `tsumugi shell-hook pwsh >> $PROFILE`.

use ito_common::shellhook as common;

const APP: &str = "tsumugi";
const PURPOSE: &str = "for the sidebar and for a restore";

/// The hook for `shell`, by the name its program goes by. With none named,
/// PowerShell's: it is the default shell on Windows, and the one shell that
/// sends nothing unless told to.
pub fn text(shell: Option<&str>) -> Result<String, String> {
    common::text(APP, PURPOSE, shell)
}

/// The profile with the hook between the marks: added at the end, or put in
/// place of the one there.
pub fn with_hook(profile: &str, hook: &str) -> String {
    common::with_hook(APP, profile, hook)
}

/// The profile with the marked hook taken out.
pub fn without_hook(profile: &str) -> String {
    common::without_hook(APP, profile)
}

/// The marked hook is in the profile.
pub fn has_hook(profile: &str) -> bool {
    common::has_hook(APP, profile)
}

/// The profile file a shell reads at start (a thread's work: see ito's).
pub fn profile(shell: &str) -> Option<std::path::PathBuf> {
    common::profile(shell, &tsumugi_mux::settings::home()?)
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
    let new = if on { with_hook(&old, &hook) } else { without_hook(&old) };
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
    fn the_hooks_are_tsumugis_and_the_marked_one_goes_in_once() {
        for shell in [None, Some("bash"), Some("zsh")] {
            let hook = text(shell).unwrap();
            assert!(hook.starts_with("\n# tsumugi: say where the shell is") && !hook.contains("kura"), "{hook:?}");
        }
        assert!(text(Some("fish")).unwrap_err().contains(common::SHELLS));
        let hook = text(Some("bash")).unwrap();
        let once = with_hook("alias ll='ls -l'", &hook);
        assert!(has_hook(&once) && once.contains("# >>> tsumugi shell integration >>>"), "{once}");
        assert_eq!(with_hook(&once, &hook), once);
        assert_eq!(without_hook(&once), "alias ll='ls -l'\n");
    }
}
