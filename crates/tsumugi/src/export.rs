//! "Export or import settings" (Settings, Advanced): one TOML file holding
//! `common.toml`, the common themes and tsumugi's own settings files, to
//! carry to another machine. The format and the work are ito's
//! (`ito_common::bundle`); this keeps the list of tsumugi's files, the
//! reading of an export made before the bundle (a text with a block per
//! file under `=== <path> ===`), and the `keymap.toml` that would stand
//! over an export's `[keys]`.

use std::path::{Path, PathBuf};

use ito_common::bundle::{self, Bundle, Restored};

const OLD_HEAD: &str = "# tsumugi settings export";

/// Whether `name` (its path in the settings folder) is one of tsumugi's
/// settings files: the fixed ones, and the themes in `themes/`.
pub fn keep(name: &str) -> bool {
    let theme = name.strip_prefix("themes/").is_some_and(|n| n.ends_with(".toml") && !n.contains(['/', '\\']) && !n.starts_with('.'));
    matches!(name, "config.toml" | "keymap.toml" | "profiles.toml" | "prompts.toml" | "layouts.toml" | "theme.toml") || theme
}

/// The files in a text an export made before the bundle wrote. Only names
/// of the settings folder's own kinds come out: nothing can be written
/// outside it. One from before v0.87.0 names the settings `settings.toml`:
/// they come out as `config.toml`.
pub fn unpack(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.split_inclusive('\n');
    if !lines.next().is_some_and(|l| l.starts_with(OLD_HEAD)) {
        return Err("not a tsumugi settings export".into());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for l in lines {
        let t = l.trim_end_matches(['\r', '\n']);
        if let Some(name) = t.strip_prefix("=== ").and_then(|r| r.strip_suffix(" ===")) {
            let name = if name == "settings.toml" { "config.toml" } else { name };
            if !keep(name) {
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

/// ito's base folder (where `common.toml` is) and the settings folder.
fn folders() -> Result<(PathBuf, PathBuf), String> {
    let app = tsumugi_mux::settings::default_path().and_then(|p| p.parent().map(Path::to_path_buf)).ok_or("no settings folder")?;
    let base = ito_i18n::base_dir().ok_or("no settings folder")?;
    Ok((base, app))
}

/// Write the settings to `file` (a thread's work): how many files went in.
pub fn export(file: &Path) -> Result<usize, String> {
    let (base, app) = folders()?;
    bundle::export(file, &base, &app, "tsumugi", env!("CARGO_PKG_VERSION"), &keep)
}

/// Read an export (a bundle, or an older text) back over the settings (a
/// thread's work).
pub fn import(file: &Path) -> Result<Restored, String> {
    let (base, app) = folders()?;
    import_in(file, &base, &app)
}

fn import_in(file: &Path, base: &Path, app: &Path) -> Result<Restored, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let b = if text.starts_with(OLD_HEAD) {
        let files = unpack(&text)?.into_iter().map(|(n, t)| (format!("app/{n}"), t)).collect();
        Bundle { format: bundle::FORMAT, app: "tsumugi".into(), version: String::new(), files }
    } else {
        bundle::parse(&text)?
    };
    let done = bundle::restore(&b, base, app, "tsumugi", &keep)?;
    // An export with the settings but no keymap.toml (one from before it) puts
    // the keymap.toml there aside, or its keys would stand over the export's `[keys]`.
    let keymap = app.join("keymap.toml");
    if b.files.contains_key("app/config.toml") && !b.files.contains_key("app/keymap.toml") && keymap.is_file() {
        std::fs::rename(&keymap, app.join("keymap.toml.bak")).map_err(|e| format!("{}: {e}", keymap.display()))?;
    }
    Ok(done)
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
    if w.is_empty() {
        "session".into()
    } else {
        w
    }
}

/// Where a recording of `name` goes: `tsumugi` in Videos (Movies on
/// macOS), as `<date>-<time>-<name>.cast`. The mux server makes the folder.
pub fn cast_path(name: &str) -> Option<PathBuf> {
    let videos = tsumugi_mux::settings::home()?.join(if cfg!(target_os = "macos") { "Movies" } else { "Videos" });
    Some(videos.join("tsumugi").join(format!("{}-{}.cast", chrono::Local::now().format("%Y%m%d-%H%M%S"), file_word(name))))
}

/// Where a work log of `name` goes: Downloads, as
/// `tsumugi-<name>-<date>-<time>.txt` (the name of [`save_output`]'s file).
/// The mux server makes the folder, and gives the name `-2` when a file
/// of it is there.
pub fn log_path(name: &str) -> Option<PathBuf> {
    let downloads = tsumugi_mux::settings::home()?.join("Downloads");
    Some(downloads.join(format!("tsumugi-{}-{}.txt", file_word(name), chrono::Local::now().format("%Y%m%d-%H%M%S"))))
}

/// Where an export goes: Downloads when there is one, else the home folder.
pub fn place() -> Option<PathBuf> {
    let home = tsumugi_mux::settings::home()?;
    let downloads = home.join("Downloads");
    Some(if downloads.is_dir() { downloads } else { home })
}

/// The file the Backup card starts with: in [`place`], named by the day.
pub fn default_file() -> &'static Path {
    static FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    FILE.get_or_init(|| place().unwrap_or_default().join(format!("tsumugi-settings-{}.toml", chrono::Local::now().format("%Y%m%d"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_export_is_read_and_nothing_leaves_the_folder() {
        let head = OLD_HEAD;
        let back =
            unpack(&format!("{head} 0.100.0\n=== config.toml ===\ntheme = \"dark\"\n=== themes/Mine.toml ===\nname = \"Mine\"")).unwrap();
        assert_eq!(back[0], ("config.toml".to_string(), "theme = \"dark\"\n".to_string()));
        assert_eq!(back[1].0, "themes/Mine.toml");
        assert!(unpack("hello").is_err());
        assert!(unpack(&format!("{head} 0\n=== ../evil.toml ===\nx\n")).is_err(), "nothing outside the folder");
        assert!(unpack(&format!("{head} 0\n=== themes/../../x.toml ===\nx\n")).is_err());
        let old = unpack(&format!("{head} 0.86.0\n=== settings.toml ===\nx = 1\n")).unwrap();
        assert_eq!(old, vec![("config.toml".to_string(), "x = 1\n".to_string())], "an even older export's settings");
    }

    #[test]
    fn an_output_file_is_named_safely() {
        assert_eq!(file_word("claude · fix/the bug"), "claude---fix-the-bug");
        assert_eq!(file_word("../.."), "session");
        assert_eq!(file_word("日本語"), "日本語");
    }

    #[test]
    fn importing_keeps_the_old_file() {
        let root = std::env::temp_dir().join(format!("tsumugi-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (base, dir) = (root.join("uchmk"), root.join("uchmk").join("tsumugi"));
        std::fs::create_dir_all(dir.join("themes")).unwrap();
        std::fs::create_dir_all(base.join("themes")).unwrap();
        std::fs::write(dir.join("config.toml"), "old = 1\n").unwrap();
        std::fs::write(dir.join("themes/A.toml"), "a = 1\n").unwrap();
        std::fs::write(dir.join("keymap.toml"), "[[window.keymap]]\n").unwrap();
        std::fs::write(dir.join("notes.toml"), "x = 1\n").unwrap();
        let file = root.join("out.toml");
        let b = bundle::collect(&base, &dir, "tsumugi", "1", &keep).unwrap();
        assert_eq!(b.files.keys().map(String::as_str).collect::<Vec<_>>(), ["app/config.toml", "app/keymap.toml", "app/themes/A.toml"]);
        std::fs::write(&file, bundle::to_text(&b).unwrap()).unwrap();
        std::fs::write(dir.join("config.toml"), "newer = 2\n").unwrap();
        let done = import_in(&file, &base, &dir).unwrap();
        assert_eq!(done.written, ["app/config.toml"]);
        assert_eq!(std::fs::read_to_string(dir.join("config.toml")).unwrap(), "old = 1\n");
        let kept = std::fs::read_dir(&dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with("config.toml.bak-"));
        assert_eq!(std::fs::read_to_string(kept.expect("the one there is kept").path()).unwrap(), "newer = 2\n");
        assert!(dir.join("keymap.toml").exists(), "a bundle with the keymap keeps it");
        // An export from before keymap.toml: the one here is put aside.
        let older = root.join("older.txt");
        std::fs::write(&older, format!("{OLD_HEAD} 0.100.0\n=== config.toml ===\n[keys]\nhelp = \"F9\"\n")).unwrap();
        assert_eq!(import_in(&older, &base, &dir).unwrap().written, ["app/config.toml"]);
        assert!(!dir.join("keymap.toml").exists());
        assert_eq!(std::fs::read_to_string(dir.join("keymap.toml.bak")).unwrap(), "[[window.keymap]]\n");
        let _ = std::fs::remove_dir_all(&root);
    }
}
