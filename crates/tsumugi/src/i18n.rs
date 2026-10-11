//! The words of tsumugi's own menus, looked up by key so another language is
//! one more table: [`tr`] (or [`trf`] with arguments). English is built in
//! (`src/lang/en.toml`); another language is a file beside it (`ja.toml`),
//! listed in [`AVAILABLE`], whose missing keys fall back to English.
//!
//! Which language: `language` in the common.toml all of uchmk's apps share,
//! else the OS's (`auto`). The table, the choice and the checks live in
//! `ito-i18n`, shared with yagura and kura.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use ito_i18n::Table;

/// The languages with a table, English first.
pub const AVAILABLE: &[(&str, &str)] = &[("en", include_str!("lang/en.toml")), ("ja", include_str!("lang/ja.toml"))];

/// One table per language in [`AVAILABLE`], built on first use and kept, so
/// [`tr`] can hand out `&'static str` and the language can still change
/// while tsumugi runs (the settings screen, common.toml being edited).
static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
/// Which of [`TABLES`] is in force.
static CHOSEN: AtomicUsize = AtomicUsize::new(0);

/// Use the language common.toml's `language` comes to (`auto` or unset:
/// the OS's) from the next lookup on.
pub fn follow(common: &str) {
    set_language(&resolve_language(Some(common), ito_i18n::os_language().as_deref()));
}

/// Use `code`'s table from the next lookup on; a code without a table is
/// English.
pub fn set_language(code: &str) {
    let i = AVAILABLE.iter().position(|(l, _)| *l == code).unwrap_or(0);
    CHOSEN.store(i, Ordering::Relaxed);
}

/// The code of the language in force (`en`, `ja`).
pub fn current_language() -> &'static str {
    AVAILABLE[CHOSEN.load(Ordering::Relaxed).min(AVAILABLE.len() - 1)].0
}

fn tables() -> &'static Vec<Table> {
    TABLES.get_or_init(|| {
        AVAILABLE
            .iter()
            .map(|(l, _)| {
                let (t, errors) = Table::new(AVAILABLE, l);
                for e in errors {
                    eprintln!("tsumugi: {e}");
                }
                t
            })
            .collect()
    })
}

fn get() -> &'static Table {
    let all = tables();
    &all[CHOSEN.load(Ordering::Relaxed).min(all.len() - 1)]
}

/// The English text for `key` whatever the language in force: the
/// settings' search finds a row by its English words too.
pub fn tr_en(key: &str) -> &str {
    tables()[0].tr(key)
}

/// The text for `key` in the chosen language; the key itself when no table
/// has it, so a missing entry shows up on screen instead of a blank.
pub fn tr(key: &str) -> &str {
    get().tr(key)
}

/// The `[keydesc]` translation of an English key text (keys.rs's names
/// and the help's lines), so keys.rs stays English for TESTING-KEYS.md; the
/// text itself when the language has none.
pub fn tr_desc(desc: &str) -> &str {
    get().tr_desc(desc)
}

/// [`tr`] with `{0}`, `{1}` … replaced by `args`.
pub fn trf(key: &str, args: &[&str]) -> String {
    get().trf(key, args)
}

/// The language to use: common.toml's unless it is `auto`, else the OS's
/// (`ja-JP` → `ja`); anything without a table is English.
fn resolve_language(common: Option<&str>, os: Option<&str>) -> String {
    let codes: Vec<&str> = AVAILABLE.iter().map(|(l, _)| *l).collect();
    ito_i18n::resolve(None, common, os, &codes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ito_i18n::check;

    #[test]
    fn a_missing_key_reads_as_itself() {
        assert_eq!(tr("no.such.key"), "no.such.key");
        assert_eq!(Table::new(AVAILABLE, "en").0.tr("tab.pin"), "Pin to top");
        assert_eq!(Table::new(AVAILABLE, "ja").0.trf("tag.show", &["#api"]), "#api だけを出す");
    }

    #[test]
    fn common_toml_beats_the_os_unless_it_says_auto() {
        assert_eq!(resolve_language(Some("en"), Some("ja-JP")), "en");
        assert_eq!(resolve_language(Some("ja"), Some("en-US")), "ja");
        assert_eq!(resolve_language(Some("auto"), Some("ja-JP")), "ja");
        assert_eq!(resolve_language(None, Some("en-US")), "en");
        assert_eq!(resolve_language(Some("xx"), None), "en");
    }

    /// Every translation has each English key, no key English lacks (a typo
    /// would never be read), and the same `{0}` … to fill in.
    #[test]
    fn translations_match_english_key_for_key() {
        let bad = check::translations(AVAILABLE);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every `tr("…")` in the source has an English entry, so no key ever
    /// shows on screen as itself.
    #[test]
    fn every_key_in_the_source_has_english() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let missing = check::source_keys(&src, &Table::new(AVAILABLE, "en").0, &["i18n.rs"]);
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }
}
