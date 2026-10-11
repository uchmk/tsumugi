//! Where a new session runs: this machine, a WSL distribution, or a host
//! from `~/.ssh/config`. The new-session dialog offers the ones found; a
//! profile keeps the choice as a word (`wsl:Ubuntu`, `ssh:box`,
//! `mux:box`).

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Where {
    #[default]
    Here,
    /// A WSL distribution, by its name.
    Wsl(String),
    /// A host as `~/.ssh/config` names it: `ssh` in a pane here.
    Ssh(String),
    /// The same host's own tsumugi server, over ssh (`machine.rs`): the
    /// session lives there and stays when the line drops.
    Mux(String),
}

impl Where {
    /// As a profile keeps it: empty for this machine.
    pub fn word(&self) -> String {
        match self {
            Where::Here => String::new(),
            Where::Wsl(d) => format!("wsl:{d}"),
            Where::Ssh(h) => format!("ssh:{h}"),
            Where::Mux(h) => format!("mux:{h}"),
        }
    }

    pub fn from_word(w: &str) -> Self {
        let w = w.trim();
        let after = |p: &str| w.strip_prefix(p).map(str::trim).filter(|r| !r.is_empty()).map(str::to_owned);
        match (after("wsl:"), after("ssh:"), after("mux:")) {
            (Some(d), _, _) => Where::Wsl(d),
            (_, Some(h), _) => Where::Ssh(h),
            (_, _, Some(h)) => Where::Mux(h),
            _ => Where::Here,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Where::Here => crate::i18n::tr("sidebar.this_machine").into(),
            Where::Wsl(d) => crate::i18n::trf("newsession.wsl", &[d]),
            Where::Ssh(h) => crate::i18n::trf("newsession.ssh", &[h]),
            Where::Mux(h) => crate::i18n::trf("newsession.mux", &[h]),
        }
    }

    /// Each SSH host twice: in a pane here, and kept on its own server.
    pub fn with_kept(places: &[Where]) -> Vec<Where> {
        let mut out = Vec::new();
        for p in places {
            out.push(p.clone());
            if let Where::Ssh(h) = p {
                out.push(Where::Mux(h.clone()));
            }
        }
        out
    }

    /// The program the session runs in place of the shell; `None` for the
    /// shell of the settings. WSL starts in the folder (it reads the
    /// Windows folder it is started in); SSH in the login's home folder.
    pub fn shell(&self) -> Option<(String, Vec<String>)> {
        match self {
            Where::Here | Where::Mux(_) => None,
            Where::Wsl(d) => Some(("wsl.exe".into(), vec!["-d".into(), d.clone()])),
            Where::Ssh(h) => Some(("ssh".into(), vec![h.clone()])),
        }
    }
}

/// The hosts `~/.ssh/config` names one by one: those of its `Host` lines
/// that are not patterns (`*`, `?`, `!`).
pub fn ssh_hosts(config: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in config.lines() {
        let line = line.trim();
        let (key, rest) = match line.find(|c: char| c.is_whitespace() || c == '=') {
            Some(k) => (&line[..k], line[k..].trim_start_matches(|c: char| c.is_whitespace() || c == '=')),
            None => continue,
        };
        if !key.eq_ignore_ascii_case("host") {
            continue;
        }
        for name in rest.split_whitespace() {
            let name = name.trim_matches('"');
            if !name.is_empty() && !name.contains(['*', '?', '!']) && !out.iter().any(|h| h == name) {
                out.push(name.to_owned());
            }
        }
    }
    out
}

/// The distributions `wsl.exe -l -q` lists (in UTF-16), less Docker
/// Desktop's own.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn wsl_distros(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let text = String::from_utf16_lossy(&units);
    text.trim_start_matches('\u{feff}')
        .lines()
        .map(|l| l.trim_matches(|c: char| c.is_whitespace() || c == '\0'))
        .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("docker-desktop") && !l.eq_ignore_ascii_case("docker-desktop-data"))
        .map(str::to_owned)
        .collect()
}

/// Look for the places on a thread (`wsl.exe` can take a moment to
/// answer): this machine first, then WSL's, then SSH's. Into `into`, then a
/// repaint.
pub fn find(into: Arc<Mutex<Vec<Where>>>, ctx: eframe::egui::Context) {
    let _ = std::thread::Builder::new().name("remote-places".into()).spawn(move || {
        let mut found = Vec::new();
        found.extend(wsl_list().into_iter().map(Where::Wsl));
        if let Some(text) = tsumugi_mux::settings::home().and_then(|h| std::fs::read_to_string(h.join(".ssh").join("config")).ok()) {
            found.extend(ssh_hosts(&text).into_iter().map(Where::Ssh));
        }
        if !found.is_empty() {
            found.insert(0, Where::Here);
        }
        if let Ok(mut places) = into.lock() {
            *places = found;
        }
        ctx.request_repaint();
    });
}

#[cfg(windows)]
fn wsl_list() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("wsl.exe").args(["-l", "-q"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).creation_flags(CREATE_NO_WINDOW).output();
    match out {
        Ok(o) if o.status.success() => wsl_distros(&o.stdout),
        _ => Vec::new(),
    }
}

#[cfg(not(windows))]
fn wsl_list() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_keeps_as_a_word() {
        for w in [Where::Here, Where::Wsl("Ubuntu-24.04".into()), Where::Ssh("box".into()), Where::Mux("box".into())] {
            assert_eq!(Where::from_word(&w.word()), w);
        }
        assert_eq!(Where::Mux("box".into()).shell(), None, "the server there runs its own shell");
        assert_eq!(Where::with_kept(&[Where::Wsl("D".into()), Where::Ssh("box".into())]), vec![Where::Wsl("D".into()), Where::Ssh("box".into()), Where::Mux("box".into())]);
        assert_eq!(Where::from_word("ssh:"), Where::Here);
        assert_eq!(Where::from_word("nonsense"), Where::Here);
        assert_eq!(Where::Ssh("box".into()).shell(), Some(("ssh".into(), vec!["box".into()])));
        assert_eq!(Where::Wsl("Debian".into()).shell(), Some(("wsl.exe".into(), vec!["-d".into(), "Debian".into()])));
        assert_eq!(Where::Here.shell(), None);
    }

    #[test]
    fn only_named_hosts_are_offered() {
        let config = "# mine\nHost box build\n  HostName 10.0.0.2\nhost=pi\nHost *.corp !bad gpu?\nMatch host x\n  Host  \"quoted\"\nHost box\n";
        assert_eq!(ssh_hosts(config), ["box", "build", "pi", "quoted"]);
    }

    #[test]
    fn wsl_names_come_out_of_utf16() {
        let text = "\u{feff}Ubuntu\r\ndocker-desktop\r\n\r\nDebian\r\n";
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(wsl_distros(&bytes), ["Ubuntu", "Debian"]);
    }
}
