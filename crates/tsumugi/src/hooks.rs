//! Claude Code's hooks for tsumugi (v1-scope 2, and the step left from 5:
//! asking once whether to add them): `Notification` runs `tsumugi notify
//! --stdin`, `Stop` runs `tsumugi notify --state done`, in Claude Code's
//! `settings.json`. Added beside whatever hooks are there, the file kept
//! first as `settings.json.tsumugi-backup`.
//!
//! The commands name this tsumugi by its full path: the shell Claude Code
//! runs its hooks in (Git Bash's on Windows) need not have tsumugi on its
//! `PATH`, and a bare `tsumugi` was "command not found" there on every
//! reply. Hooks of tsumugi's whose program is not found any more (a bare
//! one from before, or a tsumugi since moved) are pointed at this one when
//! the window starts (`repair_on_disk`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::json::Json;

/// The two hooks: Claude Code's event and what tsumugi is given.
pub const HOOKS: [(&str, &str); 2] = [("Notification", "notify --stdin"), ("Stop", "notify --state done")];

/// Claude Code's own settings (`CLAUDE_CONFIG_DIR` moves them).
pub fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| tsumugi_mux::settings::home().map(|h| h.join(".claude")))?;
    Some(base.join("settings.json"))
}

/// The hooks' commands for this tsumugi, in the order of `HOOKS`.
pub fn commands() -> &'static [String; 2] {
    static ONCE: OnceLock<[String; 2]> = OnceLock::new();
    ONCE.get_or_init(|| {
        let program = exe().map(|p| shown(&p)).unwrap_or_else(|| "tsumugi".into());
        HOOKS.map(|(_, args)| format!("{program} {args}"))
    })
}

/// This tsumugi's file. An AppImage's own file, not its mount, which is
/// somewhere else every run.
fn exe() -> Option<PathBuf> {
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(p) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    std::env::current_exe().ok()
}

/// `exe` as the hooks' shell reads it: `/` for Windows's `\` (Git Bash takes
/// `C:/…`), quoted only when it has to be.
fn shown(exe: &Path) -> String {
    let s = exe.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    quote(&if cfg!(windows) { s.replace('\\', "/") } else { s.to_owned() })
}

/// `s` as one word for a POSIX shell: as it is when nothing in it means
/// anything to one (so that cmd and PowerShell read it the same), else in
/// single quotes.
fn quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "/._-+:@,=".contains(c)) {
        return s.to_owned();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// A command's program, unquoted as a POSIX shell would (a `\` escapes only
/// a quote, a space or another `\`, so a Windows path written by hand keeps
/// its), and the rest of the line.
fn program(command: &str) -> (String, &str) {
    let s = command.trim_start();
    let mut out = String::new();
    let (mut single, mut double) = (false, false);
    let mut it = s.char_indices().peekable();
    while let Some((at, c)) = it.next() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '\\' if !single && it.peek().is_some_and(|(_, n)| matches!(*n, '\'' | '"' | ' ' | '\\')) => out.push(it.next().map_or(c, |(_, n)| n)),
            c if c.is_whitespace() && !single && !double => return (out, &s[at..]),
            c => out.push(c),
        }
    }
    (out, "")
}

/// A command that is tsumugi's: a program called tsumugi (`tsumugi`,
/// `…/tsumugi.exe`, `tsumugi-x86_64.AppImage`) given `notify`.
fn ours(command: &str) -> bool {
    let (program, rest) = program(command);
    let name = program.rsplit(['/', '\\']).next().unwrap_or_default();
    name.to_ascii_lowercase().starts_with("tsumugi") && rest.trim_start().starts_with("notify")
}

/// Whether the program of a hook's command is there to run.
fn runs(program: &str) -> bool {
    if program.contains(['/', '\\']) { Path::new(program).is_file() } else { crate::facts::locate(program) }
}

/// Whether the settings run `tsumugi notify` on both events already.
pub fn present(text: &str) -> bool {
    let Ok(v) = Json::parse(text) else { return false };
    HOOKS.iter().all(|(event, _)| v.get("hooks").and_then(|h| h.get(event)).is_some_and(|e| holds(e, &ours)))
}

/// Whether a string in `v` is one `like` takes.
fn holds(v: &Json, like: &dyn Fn(&str) -> bool) -> bool {
    match v {
        Json::String(s) => like(s),
        Json::Array(a) => a.iter().any(|x| holds(x, like)),
        Json::Object(f) => f.iter().any(|(_, x)| holds(x, like)),
        _ => false,
    }
}

/// Put `command` in place of tsumugi's commands in `v`; whether there were
/// any.
fn rewrite(v: &mut Json, command: &str) -> bool {
    match v {
        Json::String(s) if ours(s) => {
            *s = command.to_owned();
            true
        }
        Json::Array(a) => a.iter_mut().fold(false, |any, x| rewrite(x, command) | any),
        Json::Object(f) => f.iter_mut().fold(false, |any, (_, x)| rewrite(x, command) | any),
        _ => false,
    }
}

/// The settings with this tsumugi's hooks in: tsumugi's commands there
/// made `commands()`, the missing ones added; the rest as it was. An empty
/// file is an empty object.
pub fn add(text: &str) -> Result<String, String> {
    add_these(text, commands())
}

fn add_these(text: &str, commands: &[String; 2]) -> Result<String, String> {
    let mut v = if text.trim().is_empty() { Json::Object(Vec::new()) } else { Json::parse(text).map_err(|e| format!("Claude Code's settings.json does not read: {e}"))? };
    let Json::Object(top) = &mut v else { return Err("Claude Code's settings.json is not an object".into()) };
    let hooks = entry(top, "hooks", Json::Object(Vec::new()));
    let Json::Object(events) = hooks else { return Err("`hooks` in Claude Code's settings.json is not an object".into()) };
    for ((event, _), command) in HOOKS.iter().zip(commands) {
        let list = entry(events, event, Json::Array(Vec::new()));
        if rewrite(list, command) {
            continue;
        }
        let Json::Array(list) = list else { return Err(format!("`hooks.{event}` is not a list")) };
        let one = Json::Object(vec![("type".into(), Json::String("command".into())), ("command".into(), Json::String(command.clone()))]);
        list.push(Json::Object(vec![("hooks".into(), Json::Array(vec![one]))]));
    }
    Ok(v.pretty())
}

/// The settings with tsumugi's hooks taken out, and an event left empty by
/// that taken out too; the rest as it was.
pub fn remove(text: &str) -> Result<String, String> {
    let mut v = Json::parse(text).map_err(|e| format!("Claude Code's settings.json does not read: {e}"))?;
    let Json::Object(top) = &mut v else { return Err("Claude Code's settings.json is not an object".into()) };
    if let Some((_, Json::Object(events))) = top.iter_mut().find(|(k, _)| k == "hooks") {
        for (_, list) in events.iter_mut() {
            if let Json::Array(list) = list {
                list.retain(|x| !holds(x, &ours));
            }
        }
        events.retain(|(_, list)| !matches!(list, Json::Array(a) if a.is_empty()));
    }
    top.retain(|(k, v)| !(k == "hooks" && matches!(v, Json::Object(e) if e.is_empty())));
    Ok(v.pretty())
}

/// The settings with tsumugi's hooks pointed at `commands` when one of them
/// runs a program that is not there (`runs`); `None` when they are all
/// fine, or there are none. One that runs is left: another tsumugi of
/// one's own (a build being worked on) is not taken over by starting it.
fn repair(text: &str, commands: &[String; 2], runs: &dyn Fn(&str) -> bool) -> Option<String> {
    let v = Json::parse(text).ok()?;
    let broken = |s: &str| ours(s) && !commands.iter().any(|c| c == s) && !runs(&program(s).0);
    if !v.get("hooks").is_some_and(|h| holds(h, &broken)) {
        return None;
    }
    add_these(text, commands).ok()
}

/// Take them out of the file on disk (a thread's work), the old file kept
/// beside it first.
pub fn uninstall() -> Result<PathBuf, String> {
    let path = settings_path().ok_or("no home folder")?;
    let old = crate::files::read_or_empty(&path)?;
    let new = remove(&old)?;
    crate::files::write_atomic(&path.with_extension("json.tsumugi-backup"), &old).map_err(|e| format!("the backup: {e}"))?;
    crate::files::write_atomic(&path, new)?;
    Ok(path)
}

/// The value at `key`, put there as `empty` when there is none.
fn entry<'a>(fields: &'a mut Vec<(String, Json)>, key: &str, empty: Json) -> &'a mut Json {
    let at = match fields.iter().position(|(k, _)| k == key) {
        Some(at) => at,
        None => {
            fields.push((key.into(), empty));
            fields.len() - 1
        }
    };
    &mut fields[at].1
}

/// Add them to the file on disk (a thread's work): the old file kept
/// beside it first.
pub fn install() -> Result<PathBuf, String> {
    let path = settings_path().ok_or("no home folder")?;
    // A file that is there but does not read is an error, not an empty one
    // to write over (the source review, 2026-10-07).
    let old = crate::files::read_or_empty(&path)?;
    let new = add(&old)?;
    if !old.is_empty() {
        crate::files::write_atomic(&path.with_extension("json.tsumugi-backup"), &old).map_err(|e| format!("the backup: {e}"))?;
    }
    crate::files::write_atomic(&path, new)?;
    Ok(path)
}

/// Whether the hooks are in the file on disk (a thread's work).
pub fn installed() -> bool {
    settings_path().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| present(&t))
}

/// Point tsumugi's hooks in the file on disk at this tsumugi when one of
/// them runs a program that is not there (a thread's work, as the window
/// starts), the old file kept beside it first; the file when it was.
pub fn repair_on_disk() -> Result<Option<PathBuf>, String> {
    let Some(path) = settings_path() else { return Ok(None) };
    let Ok(old) = std::fs::read_to_string(&path) else { return Ok(None) };
    let Some(new) = repair(&old, commands(), &runs) else { return Ok(None) };
    crate::files::write_atomic(&path.with_extension("json.tsumugi-backup"), &old).map_err(|e| format!("the backup: {e}"))?;
    crate::files::write_atomic(&path, new)?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn these(program: &str) -> [String; 2] {
        HOOKS.map(|(_, args)| format!("{program} {args}"))
    }

    #[test]
    fn the_hooks_go_in_beside_what_is_there() {
        let mine = r#"{"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}}"#;
        let ours = these("/opt/tsumugi/tsumugi");
        assert!(!present(mine));
        let out = add_these(mine, &ours).unwrap();
        assert!(present(&out), "{out}");
        let v = Json::parse(&out).unwrap();
        assert_eq!(v.get("model").and_then(Json::string), Some("opus"), "the rest kept");
        let stop = v.get("hooks").and_then(|h| h.get("Stop")).and_then(Json::array).unwrap();
        assert_eq!(stop.len(), 2, "beside the one there");
        assert!(out.contains("\"/opt/tsumugi/tsumugi notify --state done\""), "{out}");
        assert_eq!(add_these(&out, &ours).unwrap(), out, "twice is once");
        assert!(present(&add_these("", &ours).unwrap()), "no file: one with only them");
        assert!(add_these("[1]", &ours).is_err());
        assert!(add_these("{not json", &ours).is_err());
    }

    #[test]
    fn removing_takes_out_only_tsumugis() {
        let mine = r#"{"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}}"#;
        let ours = these("/opt/tsumugi/tsumugi");
        let back = remove(&add_these(mine, &ours).unwrap()).unwrap();
        assert!(!present(&back));
        let v = Json::parse(&back).unwrap();
        assert_eq!(v.get("hooks").and_then(|h| h.get("Stop")).and_then(Json::array).map(Vec::len), Some(1), "{back}");
        assert!(v.get("hooks").and_then(|h| h.get("Notification")).is_none(), "an emptied event goes: {back}");
        let only = remove(&add_these("", &ours).unwrap()).unwrap();
        assert!(Json::parse(&only).unwrap().get("hooks").is_none(), "{only}");
    }

    #[test]
    fn a_path_is_one_word_for_the_shell() {
        assert_eq!(quote("/opt/tsumugi/tsumugi"), "/opt/tsumugi/tsumugi");
        assert_eq!(quote("C:/Users/yu/AppData/Local/tsumugi/tsumugi.exe"), "C:/Users/yu/AppData/Local/tsumugi/tsumugi.exe");
        assert_eq!(quote("C:/Users/ゆう/tsumugi.exe"), "C:/Users/ゆう/tsumugi.exe", "letters of any language as they are");
        assert_eq!(quote("C:/Program Files/tsumugi/tsumugi.exe"), "'C:/Program Files/tsumugi/tsumugi.exe'");
        assert_eq!(quote("/home/o'neil/tsumugi"), r"'/home/o'\''neil/tsumugi'");
        for path in ["C:/Program Files/tsumugi/tsumugi.exe", "/home/o'neil/tsumugi", "/opt/tsumugi/tsumugi"] {
            assert_eq!(program(&format!("{} notify --stdin", quote(path))), (path.to_owned(), " notify --stdin"), "read back as it was");
        }
    }

    #[test]
    fn tsumugis_commands_are_known_however_written() {
        for mine in ["tsumugi notify --stdin", "tsumugi notify", "/opt/tsumugi/tsumugi notify --state done", "'C:/Program Files/tsumugi/tsumugi.exe' notify --stdin", r#""C:\Program Files\tsumugi\TSUMUGI.EXE" notify --stdin"#, r"C:\tools\tsumugi.exe notify --stdin", "/home/me/tsumugi-x86_64.AppImage notify --stdin"] {
            assert!(ours(mine), "{mine}");
        }
        for not in ["say done", "tsumugi ls", "notify tsumugi", "/usr/bin/notify-send tsumugi notify", ""] {
            assert!(!ours(not), "{not}");
        }
        assert_eq!(program(r"C:\tools\tsumugi.exe notify").0, r"C:\tools\tsumugi.exe", "a Windows path by hand keeps its \\");
    }

    /// A bare `tsumugi` that the shell does not find (the "command not
    /// found" on every reply) is pointed at this tsumugi; one that runs, or
    /// already this one, is left.
    #[test]
    fn a_hook_whose_program_is_gone_is_pointed_here() {
        let ours = these("/opt/tsumugi/tsumugi");
        let old = r#"{"model": "opus", "hooks": {"Notification": [{"hooks": [{"type": "command", "command": "tsumugi notify --stdin"}]}], "Stop": [{"hooks": [{"type": "command", "command": "say done"}, {"type": "command", "command": "tsumugi notify --state done"}]}]}}"#;
        let fixed = repair(old, &ours, &|_| false).expect("not on the PATH");
        assert!(present(&fixed), "{fixed}");
        assert!(!fixed.contains("\"tsumugi notify"), "{fixed}");
        let v = Json::parse(&fixed).unwrap();
        assert_eq!(v.get("model").and_then(Json::string), Some("opus"), "the rest kept");
        let stop = v.get("hooks").and_then(|h| h.get("Stop")).and_then(Json::array).unwrap();
        assert_eq!(stop.len(), 1, "in its place, not beside it: {fixed}");
        assert!(fixed.contains("\"say done\"") && fixed.contains("\"/opt/tsumugi/tsumugi notify --state done\""), "{fixed}");
        assert_eq!(repair(&fixed, &ours, &|_| false), None, "this one's own are left");
        assert_eq!(repair(old, &ours, &|p| p == "tsumugi"), None, "on the PATH: left");
        let other = add_these("", &these("/home/me/dev/tsumugi")).unwrap();
        assert_eq!(repair(&other, &ours, &|p| p == "/home/me/dev/tsumugi"), None, "another tsumugi that runs: left");
        assert!(repair(&other, &ours, &|_| false).is_some(), "one since moved: pointed here");
        assert_eq!(repair(r#"{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}}"#, &ours, &|_| false), None, "none of tsumugi's: left");
        assert_eq!(repair("{not json", &ours, &|_| false), None);
    }
}
