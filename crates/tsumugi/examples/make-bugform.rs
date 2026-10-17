//! Write `.github/ISSUE_TEMPLATE/bug_report.yml` from ito's shared form
//! (`ito_bugreport::template`), so every uchmk app's report reads alike.
//!
//!     cargo run --example make-bugform [-- --check]
//!
//! `--check` writes nothing and exits 1 when the file differs (what CI runs).
//! The same text is compared by a test below.

use ito_bugreport::Field;

const OUT: &str = "../../.github/ISSUE_TEMPLATE/bug_report.yml";

/// The app's own fields, placed before the backtrace. None to begin with; add
/// one (and fill its id in `ito_bugreport::url`'s `extra`) when the app knows
/// something a report needs.
const EXTRA: &[Field] = &[];

fn form() -> String {
    ito_bugreport::template("tsumugi", EXTRA)
}

fn main() {
    let want = form();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(OUT);
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    if std::env::args().any(|a| a == "--check") {
        if have != want {
            eprintln!("{} is out of date: run `cargo run --example make-bugform`", path.display());
            std::process::exit(1);
        }
        return;
    }
    if have != want {
        std::fs::write(&path, want).expect("write bug_report.yml");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The form on disk is the shared one, so a change in ito shows up here.
    #[test]
    fn the_form_is_the_shared_one() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(OUT);
        let have = std::fs::read_to_string(path).expect("bug_report.yml");
        assert_eq!(have.replace("\r\n", "\n"), form());
    }

    /// GitHub drops a parameter the form has no field for; the ids the F12 key
    /// fills (`version`, `os`, `context`) are in the form.
    #[test]
    fn the_ids_the_url_fills_are_in_the_form() {
        let ids = ito_bugreport::field_ids(&form());
        for id in ["version", "os", "context"] {
            assert!(ids.iter().any(|i| i == id), "{id} is not in {ids:?}");
        }
    }
}
