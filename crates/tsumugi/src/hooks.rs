//! Claude Code's hooks for tsumugi (v1-scope 2, and the step left from 5:
//! asking once whether to add them): `Notification` runs `tsumugi notify
//! --stdin`, `Stop` runs `tsumugi notify --state done`, in Claude Code's
//! `settings.json`. Added beside whatever hooks are there, the file kept
//! first as `settings.json.tsumugi-backup`.

use std::path::PathBuf;

use crate::json::Json;

/// The two hooks: Claude Code's event and the command it runs.
pub const HOOKS: [(&str, &str); 2] = [("Notification", "tsumugi notify --stdin"), ("Stop", "tsumugi notify --state done")];

/// Claude Code's own settings (`CLAUDE_CONFIG_DIR` moves them).
pub fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| tsumugi_mux::settings::home().map(|h| h.join(".claude")))?;
    Some(base.join("settings.json"))
}

/// Whether the settings run `tsumugi notify` on both events already.
pub fn present(text: &str) -> bool {
    let Ok(v) = Json::parse(text) else { return false };
    HOOKS.iter().all(|(event, _)| v.get("hooks").and_then(|h| h.get(event)).is_some_and(|e| mentions(e, "tsumugi notify")))
}

fn mentions(v: &Json, needle: &str) -> bool {
    match v {
        Json::String(s) => s.contains(needle),
        Json::Array(a) => a.iter().any(|x| mentions(x, needle)),
        Json::Object(f) => f.iter().any(|(_, x)| mentions(x, needle)),
        _ => false,
    }
}

/// The settings with the hooks added where they are missing; the rest as
/// it was. An empty file is an empty object.
pub fn add(text: &str) -> Result<String, String> {
    let mut v = if text.trim().is_empty() { Json::Object(Vec::new()) } else { Json::parse(text).map_err(|e| format!("Claude Code's settings.json does not read: {e}"))? };
    let Json::Object(top) = &mut v else { return Err("Claude Code's settings.json is not an object".into()) };
    let hooks = entry(top, "hooks", Json::Object(Vec::new()));
    let Json::Object(events) = hooks else { return Err("`hooks` in Claude Code's settings.json is not an object".into()) };
    for (event, command) in HOOKS {
        let list = entry(events, event, Json::Array(Vec::new()));
        if mentions(list, "tsumugi notify") {
            continue;
        }
        let Json::Array(list) = list else { return Err(format!("`hooks.{event}` is not a list")) };
        let one = Json::Object(vec![("type".into(), Json::String("command".into())), ("command".into(), Json::String(command.into()))]);
        list.push(Json::Object(vec![("hooks".into(), Json::Array(vec![one]))]));
    }
    Ok(v.pretty())
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
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = add(&old)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if !old.is_empty() {
        std::fs::write(path.with_extension("json.tsumugi-backup"), &old).map_err(|e| format!("the backup: {e}"))?;
    }
    std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Whether the hooks are in the file on disk (a thread's work).
pub fn installed() -> bool {
    settings_path().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| present(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hooks_go_in_beside_what_is_there() {
        let mine = r#"{"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "say done"}]}]}}"#;
        assert!(!present(mine));
        let out = add(mine).unwrap();
        assert!(present(&out), "{out}");
        let v = Json::parse(&out).unwrap();
        assert_eq!(v.get("model").and_then(Json::string), Some("opus"), "the rest kept");
        let stop = v.get("hooks").and_then(|h| h.get("Stop")).and_then(Json::array).unwrap();
        assert_eq!(stop.len(), 2, "beside the one there");
        assert_eq!(add(&out).unwrap(), out, "twice is once");
        assert!(present(&add("").unwrap()), "no file: one with only them");
        assert!(add("[1]").is_err());
        assert!(add("{not json").is_err());
    }
}
