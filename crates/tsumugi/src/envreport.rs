//! `tsumugi env`: what a bug report needs, in a form to paste -- the version,
//! the OS, where the settings and the state are, whether a server answers
//! and what is beside the exe. It starts nothing and writes nothing.

use std::path::Path;

/// The report, one `name: value` line each, the values lined up.
pub fn report() -> String {
    let settings_path = tsumugi_mux::settings::default_path();
    let settings = settings_path.as_deref().and_then(|p| tsumugi_mux::settings::load(p).ok()).unwrap_or_default();
    let facts = crate::facts::gather(Some(settings.shell.program.clone()));
    let mut rows: Vec<(&str, String)> = vec![
        ("tsumugi", format!("{} (protocol {})", env!("CARGO_PKG_VERSION"), tsumugi_mux::proto::VERSION)),
        ("os", format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
        ("exe", std::env::current_exe().map_or_else(|e| format!("({e})"), |p| p.display().to_string())),
        ("settings", with_presence(settings_path.as_deref())),
        ("state", with_presence(tsumugi_mux::state::default_path().as_deref())),
        ("server", server()),
        (
            "conpty",
            if cfg!(windows) {
                (if facts.conpty { "beside the exe" } else { "not beside the exe (the system's)" }).into()
            } else {
                "n/a".into()
            },
        ),
        ("shell", facts.shell.clone()),
        ("shells", facts.shells.iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>().join(", ")),
        (
            "shell hook",
            match facts.shell_hook {
                Some(true) => "in the profile".into(),
                Some(false) => "not in the profile".into(),
                None => "none for this shell".into(),
            },
        ),
        ("claude hooks", on_off(facts.hooks).into()),
        ("autostart", on_off(facts.autostart).into()),
    ];
    let vars = variables();
    rows.push(("variables", if vars.is_empty() { "none set".into() } else { vars.join(" ") }));
    render(&rows)
}

fn render(rows: &[(&str, String)]) -> String {
    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    rows.iter().map(|(k, v)| format!("{k:<width$}  {v}\n")).collect()
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn with_presence(path: Option<&Path>) -> String {
    match path {
        Some(p) => format!("{} ({})", p.display(), if p.is_file() { "found" } else { "not there yet" }),
        None => "(no place found)".into(),
    }
}

/// Whether a server answers at this user's address, and how many sessions it
/// holds. Asking does not start one.
fn server() -> String {
    let at = tsumugi_mux::address();
    match tsumugi_mux::Client::connect(&at, || {}).and_then(|c| c.list()) {
        Ok(list) => format!("{} (running, {} session{})", at.0.display(), list.len(), if list.len() == 1 { "" } else { "s" }),
        Err(e) => format!("{} (not answering: {e})", at.0.display()),
    }
}

/// The `TSUMUGI_*` variables that are set, but not a value that could be a
/// secret.
fn variables() -> Vec<String> {
    let mut found: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("TSUMUGI_"))
        .map(|(k, v)| {
            if ["TOKEN", "SECRET", "PASSWORD", "KEY"].iter().any(|w| k.contains(w)) {
                format!("{k}=(hidden)")
            } else {
                format!("{k}={v}")
            }
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_values_line_up() {
        let text = render(&[("os", "linux".into()), ("settings", "/a/b".into())]);
        assert_eq!(text, "os        linux\nsettings  /a/b\n");
    }

    #[test]
    fn a_secret_variable_is_not_printed() {
        // Set only here: another test reading TSUMUGI_* sees one more name.
        std::env::set_var("TSUMUGI_ENVREPORT_TEST_TOKEN", "hunter2");
        let shown = variables().join(" ");
        std::env::remove_var("TSUMUGI_ENVREPORT_TEST_TOKEN");
        assert!(shown.contains("TSUMUGI_ENVREPORT_TEST_TOKEN=(hidden)"), "{shown}");
        assert!(!shown.contains("hunter2"));
    }
}
