//! "Export or import settings" (Settings, Advanced): one text file holding
//! the settings, the profiles, the saved prompts, `theme.toml` and the themes in `themes/`, to
//! carry to another machine. Each file is a block under a line
//! `=== <its path in the settings folder> ===`; importing writes each back,
//! the one there kept beside it as `<name>.bak` first.

use std::path::{Path, PathBuf};

const HEAD: &str = "# tsumugi settings export";

/// The files of the settings folder that go in, by their path in it.
fn files(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = ["settings.toml", "profiles.toml", "prompts.toml", "layouts.toml", "theme.toml"].iter().filter(|n| dir.join(n).is_file()).map(|n| n.to_string()).collect();
    if let Ok(entries) = std::fs::read_dir(dir.join("themes")) {
        let mut themes: Vec<String> = entries.flatten().filter_map(|e| e.file_name().to_str().map(str::to_owned)).filter(|n| n.ends_with(".toml")).map(|n| format!("themes/{n}")).collect();
        themes.sort();
        out.extend(themes);
    }
    out
}

/// The files as one text.
pub fn pack(files: &[(String, String)]) -> String {
    let mut s = format!("{HEAD} {}\n", env!("CARGO_PKG_VERSION"));
    for (name, text) in files {
        s.push_str(&format!("=== {name} ===\n"));
        s.push_str(text);
        if !text.ends_with('\n') {
            s.push('\n');
        }
    }
    s
}

/// The files in a text `pack` made. Only names of the settings folder's
/// own kinds come out: nothing can be written outside it.
pub fn unpack(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.split_inclusive('\n');
    if !lines.next().is_some_and(|l| l.starts_with(HEAD)) {
        return Err("not a tsumugi settings export".into());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for l in lines {
        let t = l.trim_end_matches(['\r', '\n']);
        if let Some(name) = t.strip_prefix("=== ").and_then(|r| r.strip_suffix(" ===")) {
            if !allowed(name) {
                return Err(format!("`{name}` is not a settings file"));
            }
            out.push((name.to_string(), String::new()));
        } else if let Some((_, body)) = out.last_mut() {
            body.push_str(l);
        }
    }
    if out.is_empty() {
        return Err("the export holds no files".into());
    }
    Ok(out)
}

fn allowed(name: &str) -> bool {
    let theme = name.strip_prefix("themes/").is_some_and(|n| n.ends_with(".toml") && !n.contains(['/', '\\']) && !n.starts_with('.'));
    matches!(name, "settings.toml" | "profiles.toml" | "prompts.toml" | "layouts.toml" | "theme.toml") || theme
}

/// Write the export into `to` (a folder): its file's path (a thread's work).
pub fn export(dir: &Path, to: &Path) -> Result<PathBuf, String> {
    let read: Vec<(String, String)> = files(dir).into_iter().filter_map(|n| std::fs::read_to_string(dir.join(&n)).ok().map(|t| (n, t))).collect();
    let file = to.join(format!("tsumugi-settings-{}.txt", chrono::Local::now().format("%Y%m%d")));
    std::fs::write(&file, pack(&read)).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

/// Read an export back into the settings folder (a thread's work): how many
/// files were written.
pub fn import(dir: &Path, from: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let files = unpack(&text)?;
    for (name, body) in &files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        if path.is_file() {
            let mut bak = path.clone().into_os_string();
            bak.push(".bak");
            std::fs::copy(&path, bak).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        std::fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(files.len())
}

/// A session's output, written as `tsumugi-<name>-<date>-<time>.txt` in
/// `to` (a thread's work): the file's path.
pub fn save_output(to: &Path, name: &str, text: &str) -> Result<PathBuf, String> {
    let file = to.join(format!("tsumugi-{}-{}.txt", file_word(name), chrono::Local::now().format("%Y%m%d-%H%M%S")));
    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(file)
}

/// A name made safe for a file's: letters, digits, `-` and `_` kept, the
/// rest a dash, at most 40.
fn file_word(name: &str) -> String {
    let w: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).take(40).collect();
    let w = w.trim_matches('-').to_owned();
    if w.is_empty() { "session".into() } else { w }
}

/// Where an export goes: Downloads when there is one, else the home folder.
pub fn place() -> Option<PathBuf> {
    let home = tsumugi_mux::settings::home()?;
    let downloads = home.join("Downloads");
    Some(if downloads.is_dir() { downloads } else { home })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_export_comes_back_as_it_went() {
        let files = vec![("settings.toml".to_string(), "theme = \"dark\"\n".to_string()), ("themes/Mine.toml".to_string(), "name = \"Mine\"".to_string())];
        let text = pack(&files);
        let back = unpack(&text).unwrap();
        assert_eq!(back[0], files[0]);
        assert_eq!(back[1], ("themes/Mine.toml".to_string(), "name = \"Mine\"\n".to_string()));
        assert!(unpack("hello").is_err());
        assert!(unpack(&format!("{HEAD} 0\n=== ../evil.toml ===\nx\n")).is_err(), "nothing outside the folder");
        assert!(unpack(&format!("{HEAD} 0\n=== themes/../../x.toml ===\nx\n")).is_err());
    }

    #[test]
    fn an_output_file_is_named_safely() {
        assert_eq!(file_word("claude · fix/the bug"), "claude---fix-the-bug");
        assert_eq!(file_word("../.."), "session");
        assert_eq!(file_word("日本語"), "日本語");
    }

    #[test]
    fn importing_keeps_the_old_file() {
        let dir = std::env::temp_dir().join(format!("tsumugi-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        std::fs::write(dir.join("settings.toml"), "old = 1\n").unwrap();
        std::fs::write(dir.join("themes/A.toml"), "a = 1\n").unwrap();
        let file = export(&dir, &dir).unwrap();
        std::fs::write(dir.join("settings.toml"), "newer = 2\n").unwrap();
        assert_eq!(import(&dir, &file).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(dir.join("settings.toml")).unwrap(), "old = 1\n");
        assert_eq!(std::fs::read_to_string(dir.join("settings.toml.bak")).unwrap(), "newer = 2\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
