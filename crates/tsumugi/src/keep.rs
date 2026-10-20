//! The files tsumugi keeps between runs beside the state, all TOML
//! (ito's common-spec): read with `ito_common::kept::read`, written by one
//! `kept::Writer` per file (a thread of its own, the newest save wins). A
//! file an older build kept in another form is read once and written as TOML;
//! the old file stays.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ito_common::kept::{self, Writer};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// A file of the state folder, by name.
pub fn file(name: &str) -> Option<PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name(name))
}

/// What `name` keeps; when it is not there, what `old` makes of the older
/// form, written as `name` at once so that this is done once.
pub fn load<T: Serialize + DeserializeOwned + Default>(name: &str, old: impl FnOnce() -> Option<T>) -> T {
    match file(name) {
        Some(path) => load_at(&path, old),
        None => T::default(),
    }
}

fn load_at<T: Serialize + DeserializeOwned + Default>(path: &Path, old: impl FnOnce() -> Option<T>) -> T {
    if let Some(v) = kept::read(path) {
        return v;
    }
    if path.exists() {
        return T::default();
    }
    let Some(v) = old() else { return T::default() };
    if let Ok(text) = toml::to_string(&v) {
        let _ = ito_common::write_atomic(path, text);
    }
    v
}

fn writer(name: &'static str) -> Option<Writer> {
    static WRITERS: OnceLock<Mutex<HashMap<&'static str, Writer>>> = OnceLock::new();
    let path = file(name)?;
    let mut writers = WRITERS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    Some(writers.entry(name).or_insert_with(|| Writer::new(path, true)).clone())
}

/// `value` written to `name`, on a thread of its own.
pub fn save(name: &'static str, value: &impl Serialize) {
    if let (Some(w), Ok(text)) = (writer(name), toml::to_string(value)) {
        w.save(text);
    }
}

/// `value` written to `name` before this returns, for a file that is read
/// back at once.
pub fn save_now(name: &'static str, value: &impl Serialize) {
    if let (Some(w), Ok(text)) = (writer(name), toml::to_string(value)) {
        let _ = w.save_now(text);
    }
}

/// An older file's text, when there is one.
pub fn old_text(name: &str) -> Option<String> {
    std::fs::read_to_string(file(name)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, serde::Deserialize, Default, Debug, PartialEq)]
    struct L {
        items: Vec<String>,
    }

    /// An older file is read once and written as TOML; after that the TOML
    /// is the one that counts, even when the older file is still there.
    #[test]
    fn an_older_file_is_taken_over_once() {
        let dir = std::env::temp_dir().join(format!("tsumugi-keep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("list.toml");
        let old = || {
            Some(L {
                items: vec![
                    "a".into(),
                    "b
c"
                    .into(),
                ],
            })
        };
        let got = load_at(&path, old);
        assert_eq!(
            got.items,
            [
                "a", "b
c"
            ]
        );
        assert!(path.is_file(), "written as TOML at once");
        assert_eq!(load_at(&path, || Some(L { items: vec!["other".into()] })), got, "the TOML wins over the older file");
        std::fs::write(&path, "items = [").unwrap();
        assert_eq!(load_at(&path, old), L::default(), "a file that does not read is not replaced by the older one");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
