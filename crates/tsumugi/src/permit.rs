//! "Always allow" from the waiting list: the Bash command a session asks
//! to run, put on Claude Code's list of commands it runs without asking
//! (`permissions.allow`), in the project's `.claude/settings.local.json` --
//! the one kept on this machine and out of git. Only the exact command: a
//! wider rule is for a person to write by hand.

use std::path::{Path, PathBuf};

use crate::json::Json;

/// The rule that allows the command a session asks about, from the lines
/// above its menu (`answer::asking`): "Bash command", the command, perhaps
/// its description, the question. `None` for any other question, or a
/// command of several lines.
pub fn rule(asking: &[String]) -> Option<String> {
    let at = asking.iter().position(|l| l.trim() == "Bash command")?;
    let rest: Vec<&String> = asking[at + 1..].iter().filter(|l| !l.trim().is_empty()).collect();
    // The command, maybe its description, then the question.
    let (question, before) = rest.split_last()?;
    if !question.trim().ends_with('?') || before.is_empty() || before.len() > 2 {
        return None;
    }
    let command = before[0].trim();
    (!command.is_empty() && !command.contains(['(', ')'])).then(|| format!("Bash({command})"))
}

/// Where the project's own settings for this machine are.
pub fn file(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.local.json")
}

/// The settings with `rule` added to `permissions.allow`, when it is not
/// there yet; the rest as it was.
pub fn add(text: &str, rule: &str) -> Result<String, String> {
    let mut v = if text.trim().is_empty() { Json::Object(Vec::new()) } else { Json::parse(text).map_err(|e| format!("settings.local.json does not read: {e}"))? };
    let Json::Object(top) = &mut v else { return Err("settings.local.json is not an object".into()) };
    let Json::Object(perms) = entry(top, "permissions", Json::Object(Vec::new())) else { return Err("`permissions` is not an object".into()) };
    let Json::Array(allow) = entry(perms, "allow", Json::Array(Vec::new())) else { return Err("`permissions.allow` is not a list".into()) };
    if !allow.iter().any(|r| r.string() == Some(rule)) {
        allow.push(Json::String(rule.to_owned()));
    }
    Ok(v.pretty())
}

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

/// Add the rule to the project's file (a thread's work), the old file kept
/// beside it first as `settings.local.json.tsumugi-backup`.
pub fn allow(project: &Path, rule: &str) -> Result<PathBuf, String> {
    let path = file(project);
    let old = crate::files::read_or_empty(&path)?;
    let new = add(&old, rule)?;
    if !old.is_empty() {
        crate::files::write_atomic(&path.with_extension("json.tsumugi-backup"), &old).map_err(|e| format!("the backup: {e}"))?;
    }
    crate::files::write_atomic(&path, new)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn only_one_bash_command_becomes_a_rule() {
        assert_eq!(rule(&lines(&["Bash command", "npm run test", "Run the tests", "Do you want to proceed?"])).as_deref(), Some("Bash(npm run test)"));
        assert_eq!(rule(&lines(&["Bash command", "cargo build", "Do you want to proceed?"])).as_deref(), Some("Bash(cargo build)"));
        assert_eq!(rule(&lines(&["Edit file", "src/main.rs", "Do you want to make this edit?"])), None, "only Bash");
        assert_eq!(rule(&lines(&["Bash command", "a", "b", "c", "Do you want to proceed?"])), None, "several lines");
        assert_eq!(rule(&lines(&["Bash command", "echo $(rm -rf /)", "Do you want to proceed?"])), None, "no brackets in a rule");
        assert_eq!(rule(&lines(&["Bash command", "ls"])), None, "no question");
    }

    #[test]
    fn a_rule_is_added_once_beside_the_rest() {
        let text = "{\n  \"permissions\": {\n    \"allow\": [\"Bash(ls)\"],\n    \"deny\": []\n  },\n  \"model\": \"x\"\n}";
        let out = add(text, "Bash(npm run test)").unwrap();
        let v = Json::parse(&out).unwrap();
        let allow: Vec<&str> = v.get("permissions").and_then(|p| p.get("allow")).and_then(Json::array).unwrap().iter().filter_map(Json::string).collect();
        assert_eq!(allow, ["Bash(ls)", "Bash(npm run test)"]);
        assert_eq!(v.get("model").and_then(Json::string), Some("x"));
        assert_eq!(add(&out, "Bash(npm run test)").unwrap(), out, "not twice");
        assert!(add("", "Bash(ls)").unwrap().contains("\"Bash(ls)\""));
        assert!(add("[1]", "Bash(ls)").is_err());
    }
}
