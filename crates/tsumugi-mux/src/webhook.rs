//! A session waiting while no one is at a window, sent elsewhere: an ntfy
//! topic (the phone's notification), a Slack incoming webhook, or any
//! address that takes JSON (`[notify] webhook`). Sent by the server, so a
//! window closed or minimized still tells. Through the system's `curl`, as
//! the window's update check is, so no HTTP client is built in; each on a
//! thread of its own, and a failure only goes to stderr.

use crate::proto::State;

/// `text` as a JSON string, quotes included.
fn json_string(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The `curl` arguments for one notice, before the address. The body goes
/// with `--data-raw`: with `-d`, a message starting `@` (a program's own
/// notification can say anything) has curl send the file it names (the
/// source review, 2026-10-07).
pub fn args(format: &str, state: State, title: &str, message: &str, folder: &str) -> Vec<String> {
    let json = |fields: Vec<(&str, &str)>| format!("{{{}}}", fields.iter().map(|(k, v)| format!("{}: {}", json_string(k), json_string(v))).collect::<Vec<_>>().join(", "));
    let mut out: Vec<String> = ["-sS", "-m", "10", "-X", "POST"].iter().map(|s| (*s).to_owned()).collect();
    match format {
        "slack" => {
            out.extend(["-H".into(), "Content-Type: application/json".into(), "--data-raw".into(), json(vec![("text", &format!("*{title}*\n{message}"))])]);
        }
        "json" => {
            out.extend(["-H".into(), "Content-Type: application/json".into(), "--data-raw".into(), json(vec![("title", title), ("message", message), ("folder", folder), ("state", state.word())])]);
        }
        // ntfy: the body is the message, the title a header.
        _ => {
            let (tag, priority) = match state {
                State::Error => ("x", "high"),
                State::Done => ("white_check_mark", "default"),
                _ => ("hourglass", "default"),
            };
            out.extend(["-H".into(), format!("Title: {}", title.replace(['\r', '\n'], " ")), "-H".into(), format!("Tags: {tag}"), "-H".into(), format!("Priority: {priority}"), "--data-raw".into(), message.to_owned()]);
        }
    }
    out
}

/// Send one notice to `url`, on a thread; a failure is tried once more
/// after `RETRY_AFTER` (a network that was just out).
pub fn send(url: String, format: String, state: State, title: String, message: String, folder: String) {
    let _ = std::thread::Builder::new().name("webhook".into()).spawn(move || {
        if !post(&url, &format, state, &title, &message, &folder) {
            std::thread::sleep(RETRY_AFTER);
            post(&url, &format, state, &title, &message, &folder);
        }
    });
}

const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// One try; whether it got through.
fn post(url: &str, format: &str, state: State, title: &str, message: &str, folder: &str) -> bool {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(args(format, state, title, message, folder)).arg("--url").arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    match cmd.output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!("tsumugi: the webhook failed: {}", String::from_utf8_lossy(&out.stderr).trim());
            false
        }
        Err(e) => {
            eprintln!("tsumugi: curl did not run for the webhook: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_format_says_it_its_way() {
        let ntfy = args("ntfy", State::Waiting, "filer is waiting", "Claude needs your permission", "/home/u/filer");
        assert!(ntfy.contains(&"Title: filer is waiting".to_string()) && ntfy.last().unwrap() == "Claude needs your permission");
        // A message that starts with @ is sent as it is, not as a file.
        let at = args("ntfy", State::Waiting, "t", "@/home/u/.ssh/id_rsa", "/");
        assert!(!at.iter().any(|a| a == "-d") && at[at.len() - 2] == "--data-raw", "{at:?}");
        let slack = args("slack", State::Waiting, "filer is waiting", "say \"yes\"", "/f");
        assert!(slack.last().unwrap().contains(r#""text": "*filer is waiting*\nsay \"yes\"""#), "{slack:?}");
        let json = args("json", State::Error, "t", "m", "C:\\dev\\x");
        assert!(json.last().unwrap().contains(r#""folder": "C:\\dev\\x""#) && json.last().unwrap().contains(r#""state": "error""#));
        let failed = args("ntfy", State::Error, "t", "m", "");
        assert!(failed.contains(&"Priority: high".to_string()) && failed.contains(&"Tags: x".to_string()));
        assert!(args("ntfy", State::Waiting, "a\nb", "m", "").contains(&"Title: a b".to_string()), "one header line");
    }
}
