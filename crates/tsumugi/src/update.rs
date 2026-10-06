//! "Check for updates" (Settings, General): once a start, ask GitHub for the
//! latest release and say so when it is newer than this build. Through the
//! system's `curl` (in Windows since 10, and on macOS and Linux), so no HTTP
//! client is built in; a machine without it, or offline, just says nothing.

use std::sync::mpsc::Receiver;

const LATEST: &str = "https://api.github.com/repos/uchmk/tsumugi/releases/latest";

/// Ask on a thread of its own; the answer is the newer version, if any.
pub fn check(wake: impl Fn() + Send + 'static) -> Receiver<Option<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new().name("update-check".into()).spawn(move || {
        let _ = tx.send(fetch().and_then(|body| newer(&body, env!("CARGO_PKG_VERSION"))));
        wake();
    });
    rx
}

fn fetch() -> Option<String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sfL", "--max-time", "10", "-H", "Accept: application/vnd.github+json", "-A", "tsumugi", LATEST]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.stdin(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The release's version when it is newer than `ours`.
fn newer(body: &str, ours: &str) -> Option<String> {
    let json = crate::json::Json::parse(body).ok()?;
    let tag = json.get("tag_name")?.string()?;
    let theirs = tag.trim_start_matches('v');
    (parts(theirs)? > parts(ours)?).then(|| theirs.to_string())
}

fn parts(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split(['.', '-', '+']).map(|p| p.parse::<u64>());
    Some((it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_newer_release_is_said() {
        assert_eq!(newer(r#"{"tag_name":"v0.42.0","name":"x"}"#, "0.41.3").as_deref(), Some("0.42.0"));
        assert_eq!(newer(r#"{"tag_name":"v0.41.3"}"#, "0.41.3"), None);
        assert_eq!(newer(r#"{"tag_name":"v0.9.10"}"#, "0.10.0"), None);
        assert_eq!(newer(r#"{"message":"Not Found"}"#, "0.1.0"), None);
        assert_eq!(newer("not json", "0.1.0"), None);
    }
}
