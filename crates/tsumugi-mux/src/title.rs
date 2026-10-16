//! What a session is called where a person reads it: the window's tabs and
//! cards, `tsumugi ls`, the bell's list, toasts and webhooks. The server
//! keeps the title the program set as it is (`Info::title`, `ls --json`).

/// What a session is called: its title, but a title that is only the path
/// of the program run (pwsh and cmd set the window's title to their exe,
/// `C:\Program Files\WindowsApps\…\pwsh.exe`) as the program's name, and
/// no title as the program.
pub fn display_title(title: &str, command: &str) -> String {
    let t = without_spinner(title);
    if t.is_empty() {
        return program_name(command);
    }
    let exe = t.to_ascii_lowercase().ends_with(".exe");
    // A drive's or a share's path to an exe (`Administrator: C:\…\pwsh.exe`
    // too); words that end in one (`Run setup.exe`) are a title of their own.
    let exe_path = exe && (t.contains(":\\") || t.starts_with("\\\\"));
    let one_word = !t.contains(' ') && (exe || t.contains('\\') || t.contains('/'));
    let is_program = t == command || program_name(t).eq_ignore_ascii_case(&program_name(command));
    if exe_path || one_word && (is_program || exe) {
        program_name(t)
    } else {
        t.to_owned()
    }
}

/// A title without the turning mark a program puts before it while it
/// works (Claude Code's `✳` and `✻`, the braille spinners): the card's own
/// mark says that already.
pub fn without_spinner(title: &str) -> &str {
    let spinner = |c: char| {
        matches!(c, '✳' | '✻' | '✽' | '✶' | '✢' | '✦' | '✧' | '·' | '*' | '◐' | '◓' | '◑' | '◒' | '⏺')
            || ('\u{2800}'..='\u{28FF}').contains(&c)
            || c.is_whitespace()
    };
    title.trim_start_matches(spinner).trim()
}

/// A program's name without its folder or `.exe`.
/// Both separators, whatever the system: a Windows path can come to a
/// window on another machine's server, and the other way round.
pub fn program_name(command: &str) -> String {
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    match name.len().checked_sub(4).filter(|&at| name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(".exe")) {
        Some(at) => name[..at].to_owned(),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinners_are_left_off_titles() {
        assert_eq!(without_spinner("✳ Claude Code"), "Claude Code");
        assert_eq!(without_spinner("⠋ Fixing the tests"), "Fixing the tests");
        assert_eq!(without_spinner("· Approve the edit"), "Approve the edit");
        assert_eq!(without_spinner("pwsh"), "pwsh");
        assert_eq!(display_title("✳", "claude"), "claude", "the program's name when nothing is left");
    }

    #[test]
    fn a_title_that_is_the_programs_path_is_its_name() {
        let pwsh = r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.6.0_x64__8wekyb3d8bbwe\pwsh.exe";
        assert_eq!(display_title(pwsh, pwsh), "pwsh");
        assert_eq!(display_title(r"C:\WINDOWS\system32\cmd.exe", "cmd.exe"), "cmd");
        assert_eq!(display_title("/bin/bash", "/bin/bash"), "bash");
        assert_eq!(display_title("", "/usr/bin/zsh"), "zsh");
        // A title of its own stays, paths and all.
        assert_eq!(display_title("root@vm: /tmp/proj", "/bin/bash"), "root@vm: /tmp/proj");
        assert_eq!(display_title("✳ Fix the zoom badge", "pwsh.exe"), "Fix the zoom badge", "less the working mark");
        assert_eq!(display_title("~/dev/filer", "/bin/bash"), "~/dev/filer");
        // Without the command (a notice carries only the title): an exe's
        // path is still its name.
        assert_eq!(display_title(pwsh, ""), "pwsh");
        assert_eq!(display_title(&format!("Administrator: {pwsh}"), ""), "pwsh");
        assert_eq!(display_title(r"\\server\tools\run.exe", ""), "run");
        assert_eq!(display_title("pwsh.exe", ""), "pwsh");
        assert_eq!(display_title("", ""), "");
        assert_eq!(display_title("✳", ""), "");
        // Words that end in an exe's name are a title.
        assert_eq!(display_title("Downloading setup.exe", "pwsh.exe"), "Downloading setup.exe");
        assert_eq!(display_title("✳ Build a/b.exe", ""), "Build a/b.exe");
    }
}
