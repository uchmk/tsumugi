//! Moving the settings and the state from where tsumugi kept them before
//! v0.87.0 (`<config>/tsumugi/settings.toml`, `<local data>/tsumugi/`) to
//! where every uchmk app keeps its own (`<config>/uchmk/tsumugi/config.toml`,
//! `ito_common::state_dir`). Once a run, the first time either path is asked
//! for; a folder named by an environment variable is never moved.

use std::path::PathBuf;

/// The state's files, by name (and anything named `state…`, such as the
/// saved tabs' temporary file): told apart from the settings where both
/// were in one folder (macOS).
const STATE_NAMES: [&str; 9] = ["state", "closed-sessions.json", "always-restore", "view", "sort", "closed-folders", "history", "worktrees", "pane-logs"];

/// Copies of the running program (`spawn`): an older server may still run
/// from there, so it is left where it is.
const LEFT: &str = "server";

fn is_state(name: &str) -> bool {
    name.starts_with("state") || STATE_NAMES.contains(&name)
}

/// Move the old folders' files, once a run.
pub fn move_old_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let set = |k: &str| ito_common::env_dir(k).is_some();
        let config = if set("TSUMUGI_SETTINGS") || set("TSUMUGI_CONFIG_HOME") { None } else { ito_common::app_dir("tsumugi") };
        let state = if set("TSUMUGI_STATE") || set("TSUMUGI_STATE_HOME") { None } else { ito_common::state_dir("tsumugi") };
        let (old_config, old_state) = old_dirs();
        let old = (old_config.filter(|_| config.is_some()), old_state.filter(|_| state.is_some()));
        for e in move_old(old, (config, state)) {
            eprintln!("tsumugi: {e}");
        }
    });
}

/// Where the settings and the state were before v0.87.0.
fn old_dirs() -> (Option<PathBuf>, Option<PathBuf>) {
    let var = ito_common::env_dir;
    if cfg!(windows) {
        (var("APPDATA").map(|d| d.join("tsumugi")), var("LOCALAPPDATA").map(|d| d.join("tsumugi")))
    } else if cfg!(target_os = "macos") {
        let dir = var("HOME").map(|h| h.join("Library").join("Application Support").join("tsumugi"));
        (dir.clone(), dir)
    } else {
        let home = var("HOME");
        let config = var("XDG_CONFIG_HOME").or_else(|| home.as_ref().map(|h| h.join(".config")));
        let state = var("XDG_STATE_HOME").or_else(|| home.as_ref().map(|h| h.join(".local").join("state")));
        (config.map(|d| d.join("tsumugi")), state.map(|d| d.join("tsumugi")))
    }
}

/// Move what is in the old settings and state folders (`old`) into the new
/// ones (`new`), each file or folder by itself, `settings.toml` becoming
/// `config.toml`; nothing already in the new place is replaced. An old
/// folder left empty is removed. What could not be moved, as warnings.
pub fn move_old(old: (Option<PathBuf>, Option<PathBuf>), new: (Option<PathBuf>, Option<PathBuf>)) -> Vec<String> {
    let shared = old.0.is_some() && old.0 == old.1;
    let mut warnings = Vec::new();
    let mut step = |from: &Option<PathBuf>, to: &Option<PathBuf>, keep: &dyn Fn(&str) -> bool| {
        let (Some(from), Some(to)) = (from, to) else { return };
        if from == to {
            return;
        }
        let Ok(entries) = std::fs::read_dir(from) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == LEFT || !keep(&name) {
                continue;
            }
            let target = if name == "settings.toml" { "config.toml" } else { name.as_str() };
            if let Err(err) = ito_common::move_old(&e.path(), &to.join(target)) {
                warnings.push(err);
            }
        }
    };
    step(&old.1, &new.1, &|n| !shared || is_state(n));
    step(&old.0, &new.0, &|n| !shared || !is_state(n));
    for dir in [&old.0, &old.1].into_iter().flatten() {
        let _ = std::fs::remove_dir(dir);
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn has(dir: &Path, name: &str) -> bool {
        dir.join(name).exists()
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tsumugi-places-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), name).unwrap();
    }

    #[test]
    fn separate_old_folders_move_with_the_settings_renamed() {
        let t = temp("separate");
        let (old_c, old_s) = (t.join("old-config"), t.join("old-state"));
        touch(&old_c, "settings.toml");
        touch(&old_c, "prompts.toml");
        touch(&old_s, "state");
        touch(&old_s, "history");
        touch(&old_s.join(LEFT), "tsumugi.exe");
        let (new_c, new_s) = (t.join("uchmk").join("tsumugi"), t.join("state").join("tsumugi"));
        touch(&new_c, "keep.toml");
        let warnings = move_old((Some(old_c.clone()), Some(old_s.clone())), (Some(new_c.clone()), Some(new_s.clone())));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(has(&new_c, "config.toml") && !has(&new_c, "settings.toml") && has(&new_c, "prompts.toml") && has(&new_c, "keep.toml"));
        assert!(has(&new_s, "state") && has(&new_s, "history") && !has(&new_s, LEFT));
        assert!(!old_c.exists(), "an emptied old folder is removed");
        assert!(has(&old_s, LEFT), "the server's copies stay");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn a_shared_old_folder_is_split_and_nothing_new_is_replaced() {
        let t = temp("shared");
        let old = t.join("old");
        touch(&old, "settings.toml");
        touch(&old, "layouts.toml");
        touch(&old, "state");
        touch(&old, "state.tmp");
        touch(&old, "closed-sessions.json");
        let (new_c, new_s) = (t.join("uchmk").join("tsumugi"), t.join("uchmk").join("tsumugi").join("state"));
        std::fs::create_dir_all(&new_c).unwrap();
        std::fs::write(new_c.join("config.toml"), "new").unwrap();
        let warnings = move_old((Some(old.clone()), Some(old.clone())), (Some(new_c.clone()), Some(new_s.clone())));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(std::fs::read_to_string(new_c.join("config.toml")).unwrap(), "new", "the new settings are kept");
        assert!(has(&old, "settings.toml"), "and the old ones left where they were");
        assert!(has(&new_c, "layouts.toml") && !has(&new_c, "closed-sessions.json"));
        assert!(has(&new_s, "state") && has(&new_s, "state.tmp") && has(&new_s, "closed-sessions.json") && !has(&new_s, "layouts.toml"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
