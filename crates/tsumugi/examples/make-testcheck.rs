//! TESTING-CHECKS.md from TESTING.md: one tickable line per row of the
//! sections' tables, the ticks already there kept (`[x]`, and `[~]` for a
//! look judged from a screenshot). TESTING.md stays the source of truth. The
//! generator is ito's `ito-testcheck`; this is tsumugi's configuration of it
//! (`scripts/testcheck-notes.toml` asks for English lines and for TESTING.md's
//! faults to be reported).
//!
//! cargo run -p tsumugi --example make-testcheck            write it
//! cargo run -p tsumugi --example make-testcheck -- --check fail when out of date
//! cargo run -p tsumugi --example make-testcheck -- --stats how far each section has got

use ito_testcheck::testcheck::{run, Config, Counts, Lane};

fn main() {
    // Run from the repository's root, whichever folder `cargo run` was given.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::env::set_current_dir(root).expect("repository root");
    run(&Config {
        source: "TESTING.md",
        notes: "scripts/testcheck-notes.toml",
        src_dirs: &["crates"],
        dep_crates: &[],
        release_list: None,
        command: "cargo run -p tsumugi --example make-testcheck",
        lanes: &[Lane { name: "windows", out: "TESTING-CHECKS.md", marks: &['~'], setup: false, check_release: false, preamble }],
    });
}

fn preamble(_: &Counts) -> String {
    "# Real-machine checklist

Generated from `TESTING.md` by `cargo run -p tsumugi --example make-testcheck`. **TESTING.md is the source**; this file is
its tables as one line per row that can be ticked. If they disagree, believe TESTING.md.

**Only the ticks (`[x]` and `[~]`) may be written by hand; they survive regeneration.** Anything else is overwritten.
`[x]` is a row checked on a real machine, `[~]` an appearance row judged from a screenshot (not counted as done: the owner
looks at the same picture and makes it `[x]`). **No counts are written here** (pull requests that tick would always conflict):
`-- --stats` prints them.

Before ticking, ask: if this had failed, what would be different on the screen or on the disk? Keys are in [TESTING-KEYS.md](TESTING-KEYS.md).
"
    .to_owned()
}
