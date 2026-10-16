//! "Always allow" from the waiting list: the Bash command a session asks
//! to run, put on Claude Code's list of commands it runs without asking
//! (`permissions.allow`), in the project's `.claude/settings.local.json` --
//! the one kept on this machine and out of git. Only the exact command: a
//! wider rule is for a person to write by hand.

use std::path::{Path, PathBuf};

use crate::answer;
use crate::json::Json;

/// The rule that allows the command a session asks about, from the lines
/// above its menu (`answer::asking`). Claude Code 2.1.301 shows "Bash
/// command", perhaps a tip and the description, the command between two
/// dashed lines, then the question; older ones "Bash command", the
/// command, perhaps its description, the question. `None` for any other
/// question, a command of several lines, or one with more said about it
/// between the command and the question: a reason Claude Code asks (its
/// own rule that asks wins over one that allows) or a warning.
pub fn rule(asking: &[String]) -> Option<String> {
    let at = asking.iter().position(|l| is_title(l))?;
    let rest: Vec<&str> = asking[at + 1..].iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let command = match rest.iter().position(|l| *l == answer::FRAME) {
        // One line between the dashed lines, and only the question after.
        Some(open) => match rest[open + 1..] {
            [command, close, question] if close == answer::FRAME && question.ends_with('?') => command,
            _ => return None,
        },
        None => {
            // The command, maybe its description, then the question.
            let (question, before) = rest.split_last()?;
            if !question.ends_with('?') || before.is_empty() || before.len() > 2 {
                return None;
            }
            // A second line is the description only when it reads as one
            // (it starts with a capital); otherwise the command may have
            // wrapped on screen, and half of it would be allowed (the
            // source review, 2026-10-07).
            if before.len() == 2 && !before[1].starts_with(|c: char| c.is_uppercase()) {
                return None;
            }
            before[0]
        }
    };
    // `*` (and `:*`) make a rule a pattern that allows more than this one
    // command: those are for a person to write.
    (!command.is_empty() && !command.contains(['(', ')', '*'])).then(|| format!("Bash({command})"))
}

/// Claude Code's title over a Bash command it asks to run, perhaps with who
/// asks beside it (" · from the Explore agent") and its place among the
/// questions waiting ("1 of 2"). Not one run elsewhere: "(unsandboxed)",
/// "(runs on …)", "from a remote cloud agent".
fn is_title(line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("Bash command") else { return false };
    let words: Vec<&str> = rest.split_whitespace().collect();
    let place = |w: &[&str]| matches!(w, [a, "of", b] if a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok());
    let rest = match words.len().checked_sub(3) {
        Some(n) if place(&words[n..]) => &words[..n],
        _ => &words[..],
    };
    match rest {
        [] => true,
        ["·", "from", by @ ..] => !by.is_empty() && by.join(" ") != "a remote cloud agent",
        _ => false,
    }
}

/// Where the project's own settings for this machine are.
pub fn file(project: &Path) -> PathBuf {
    project.join(".claude").join("settings.local.json")
}

/// The settings with `rule` added to `permissions.allow`, when it is not
/// there yet; the rest as it was.
pub fn add(text: &str, rule: &str) -> Result<String, String> {
    let mut v = if text.trim().is_empty() {
        Json::Object(Vec::new())
    } else {
        Json::parse(text).map_err(|e| format!("settings.local.json does not read: {e}"))?
    };
    let Json::Object(top) = &mut v else { return Err("settings.local.json is not an object".into()) };
    let Json::Object(perms) = entry(top, "permissions", Json::Object(Vec::new())) else {
        return Err("`permissions` is not an object".into());
    };
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
        assert_eq!(
            rule(&lines(&["Bash command", "npm run test", "Run the tests", "Do you want to proceed?"])).as_deref(),
            Some("Bash(npm run test)")
        );
        assert_eq!(rule(&lines(&["Bash command", "cargo build", "Do you want to proceed?"])).as_deref(), Some("Bash(cargo build)"));
        assert_eq!(rule(&lines(&["Edit file", "src/main.rs", "Do you want to make this edit?"])), None, "only Bash");
        assert_eq!(rule(&lines(&["Bash command", "a", "b", "c", "Do you want to proceed?"])), None, "several lines");
        assert_eq!(rule(&lines(&["Bash command", "echo $(rm -rf /)", "Do you want to proceed?"])), None, "no brackets in a rule");
        assert_eq!(rule(&lines(&["Bash command", "ls"])), None, "no question");
        assert_eq!(rule(&lines(&["Bash command", "rm -rf build/*", "Do you want to proceed?"])), None, "a * is a pattern");
        assert_eq!(
            rule(&lines(&["Bash command", "cargo test --workspace --all-features --", "-q", "Do you want to proceed?"])),
            None,
            "a command wrapped on screen"
        );
    }

    #[test]
    fn claude_code_2_1_301s_layout_becomes_a_rule() {
        // What `answer::asking` reads of it (#167).
        let tip = "Tip: auto mode handles these prompts for you — choose \"switch to auto mode\" below";
        let f = answer::FRAME;
        let asked = |head: &[&str], tail: &[&str]| {
            let mut v = lines(head);
            v.extend(lines(tail));
            rule(&v)
        };
        let ask = |title: &str| asked(&[title, tip, "Create an empty file", f, "touch tsumugi-a.txt", f], &["Do you want to proceed?"]);
        assert_eq!(ask("Bash command").as_deref(), Some("Bash(touch tsumugi-a.txt)"));
        assert_eq!(ask("Bash command                     1 of 2").as_deref(), Some("Bash(touch tsumugi-a.txt)"), "a place in the queue");
        assert_eq!(ask("Bash command · from the Explore agent").as_deref(), Some("Bash(touch tsumugi-a.txt)"));
        assert_eq!(ask("Bash command · from the Explore agent  2 of 3").as_deref(), Some("Bash(touch tsumugi-a.txt)"));
        assert_eq!(ask("Bash command (unsandboxed)"), None, "run outside the sandbox");
        assert_eq!(ask("Bash command (runs on build-box)"), None);
        assert_eq!(ask("Bash command · from a remote cloud agent"), None);
        assert_eq!(ask("Bash commands"), None);
        assert_eq!(
            asked(&["Bash command", f, "cargo build", f], &["Do you want to proceed?"]).as_deref(),
            Some("Bash(cargo build)"),
            "no tip, no description"
        );
        assert_eq!(asked(&["Bash command", f, "cargo test --workspace --all-features --", "-q", f], &["Do you want to proceed?"]), None);
        assert_eq!(
            asked(
                &["Bash command", f, "rm -rf target", f],
                &["Permission rule Bash(rm:*) requires confirmation for this command.", "Do you want to proceed?"]
            ),
            None,
            "an ask rule wins over an allow rule"
        );
        assert_eq!(asked(&["Bash command", f, "rm -rf build/*", f], &["Do you want to proceed?"]), None, "a * is a pattern");
        assert_eq!(asked(&["Bash command", f, "ls", f], &[]), None, "no question");
        assert_eq!(asked(&["Edit file", f, "fn main() {}", f], &["Do you want to make this edit?"]), None, "only Bash");
    }

    #[test]
    fn a_rule_is_added_once_beside_the_rest() {
        let text = "{\n  \"permissions\": {\n    \"allow\": [\"Bash(ls)\"],\n    \"deny\": []\n  },\n  \"model\": \"x\"\n}";
        let out = add(text, "Bash(npm run test)").unwrap();
        let v = Json::parse(&out).unwrap();
        let allow: Vec<&str> =
            v.get("permissions").and_then(|p| p.get("allow")).and_then(Json::array).unwrap().iter().filter_map(Json::string).collect();
        assert_eq!(allow, ["Bash(ls)", "Bash(npm run test)"]);
        assert_eq!(v.get("model").and_then(Json::string), Some("x"));
        assert_eq!(add(&out, "Bash(npm run test)").unwrap(), out, "not twice");
        assert!(add("", "Bash(ls)").unwrap().contains("\"Bash(ls)\""));
        assert!(add("[1]", "Bash(ls)").is_err());
    }
}
