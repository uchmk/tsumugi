//! A command's words as one line for the shell a session runs, each word
//! reaching the program as it was given (`tsumugi new DIR -- PROGRAM ARGS`).
//! The line is typed into the shell, so it is quoted for that shell: a path
//! with a space in it started pwsh's parser on an expression (`ParserError`,
//! the ARM64 lane's report of 2026-10-10).

/// How a shell reads quotes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// pwsh and Windows PowerShell: `'...'` is literal (`''` is a quote),
    /// and a quoted program is run with `&`.
    PowerShell,
    /// cmd: `"..."`, a quote inside as `\"` (how a program's argv reads it).
    Cmd,
    /// bash, zsh, fish, sh and the rest: `'...'`, a quote as `'\''`.
    Posix,
}

fn kind(program: &str) -> Kind {
    // By hand: a Windows path read on another system is one name.
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program).to_ascii_lowercase();
    match name.strip_suffix(".exe").unwrap_or(&name) {
        "pwsh" | "powershell" => Kind::PowerShell,
        "cmd" => Kind::Cmd,
        _ => Kind::Posix,
    }
}

/// The line typed into `program` (the session's shell) to run `words`, or
/// `None` for no words. One word is a line already (`-- "claude
/// --continue"`), typed as it is.
pub fn typed_line(program: &str, words: &[String]) -> Option<String> {
    if let [line] = words {
        return Some(line.clone());
    }
    let kind = kind(program);
    let quoted: Vec<String> = words.iter().map(|w| quote(kind, w)).collect();
    let first_quoted = words.first().is_some_and(|w| quoted[0] != *w);
    let line = quoted.join(" ");
    match (kind, first_quoted) {
        (_, _) if line.is_empty() => None,
        // A line starting with a string is an expression to PowerShell.
        (Kind::PowerShell, true) => Some(format!("& {line}")),
        _ => Some(line),
    }
}

/// The word as it is when the shell reads nothing in it, else quoted.
fn quote(kind: Kind, word: &str) -> String {
    // A backslash is a folder to Windows' shells and an escape to the rest.
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_./:=+%,".contains(c) || (c == '\\' && kind != Kind::Posix) || !c.is_ascii();
    if !word.is_empty() && word.chars().all(plain) && !(kind == Kind::PowerShell && word.starts_with(',')) {
        return word.to_owned();
    }
    match kind {
        Kind::PowerShell => format!("'{}'", word.replace('\'', "''")),
        Kind::Cmd => format!("\"{}\"", word.replace('"', "\\\"")),
        Kind::Posix => format!("'{}'", word.replace('\'', "'\\''")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &[&str]) -> Vec<String> {
        s.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn plain_words_are_typed_as_they_are() {
        for shell in ["pwsh", "cmd.exe", "/bin/bash"] {
            assert_eq!(typed_line(shell, &words(&["claude", "--continue"])).as_deref(), Some("claude --continue"), "{shell}");
        }
        assert_eq!(typed_line("pwsh", &words(&["claude --continue"])).as_deref(), Some("claude --continue"), "one word is the line");
        assert_eq!(typed_line("pwsh", &[]), None);
        assert_eq!(typed_line("pwsh", &words(&["git", "log", "C:\\dev\\a.txt", "日本"])).as_deref(), Some("git log C:\\dev\\a.txt 日本"));
        assert_eq!(typed_line("bash", &words(&["cat", "C:\\dev\\a.txt"])).as_deref(), Some("cat 'C:\\dev\\a.txt'"), "an escape to bash");
    }

    #[test]
    fn a_program_with_a_space_runs_in_powershell() {
        let w = words(&["C:\\Program Files\\Git\\bin\\bash.exe", "--rcfile", "C:\\Users\\me\\rc"]);
        assert_eq!(typed_line("pwsh", &w).as_deref(), Some("& 'C:\\Program Files\\Git\\bin\\bash.exe' --rcfile C:\\Users\\me\\rc"));
        assert_eq!(typed_line("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", &w).as_deref().map(|l| l.starts_with("& '")), Some(true));
        assert_eq!(typed_line("cmd.exe", &w).as_deref(), Some("\"C:\\Program Files\\Git\\bin\\bash.exe\" --rcfile C:\\Users\\me\\rc"));
        assert_eq!(typed_line("/usr/bin/zsh", &words(&["/opt/my apps/run", "x"])).as_deref(), Some("'/opt/my apps/run' x"));
    }

    #[test]
    fn what_a_shell_would_read_is_kept_from_it() {
        let w = words(&["echo", "it's $HOME", "a;b", ""]);
        assert_eq!(typed_line("pwsh", &w).as_deref(), Some("echo 'it''s $HOME' 'a;b' ''"));
        assert_eq!(typed_line("bash", &w).as_deref(), Some("echo 'it'\\''s $HOME' 'a;b' ''"));
        assert_eq!(typed_line("cmd", &words(&["echo", "say \"hi\""])).as_deref(), Some("echo \"say \\\"hi\\\"\""));
        // `,` alone starts an array in PowerShell.
        assert_eq!(typed_line("pwsh", &words(&["echo", ",a"])).as_deref(), Some("echo ',a'"));
    }
}
