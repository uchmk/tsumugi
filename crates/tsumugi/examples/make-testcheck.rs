//! TESTING-CHECKS.md from TESTING.md: one tickable line per row of the
//! sections' tables, the ticks already there kept (`[x]`, and `[~]` for a
//! look judged from a screenshot). TESTING.md stays the source of truth.
//!
//! cargo run -p tsumugi --example make-testcheck            write it
//! cargo run -p tsumugi --example make-testcheck -- --check fail when out of date
//! cargo run -p tsumugi --example make-testcheck -- --stats how far each section has got

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;

const SOURCE: &str = "TESTING.md";
const OUT: &str = "TESTING-CHECKS.md";

const HEAD: &str = "# 実機チェックリスト

`TESTING.md` から `cargo run -p tsumugi --example make-testcheck` で生成している。**正は TESTING.md** で、
このファイルはその表を 1 行ずつ印を付けられる形に並べたもの。食い違ったら TESTING.md を信じること。

**チェック（`[x]` と `[~]`）だけは手で書いてよく、生成し直しても残る。**それ以外を書き換えても次の生成で消える。
`[x]` は実機で確かめた印、`[~]` は見た目の行を画面の画像で判断した印（済みには数えない。持ち主が同じ画像を見て `[x]` にする）。
**件数はこのファイルに書かない**（印を付けた PR が並ぶと必ずぶつかるため）。`-- --stats` で出る。

付ける前に「失敗していたら、画面かディスクの何が違ったはずか」を自問すること。キーは [TESTING-KEYS.md](TESTING-KEYS.md)。
";

struct Row {
    id: String,
    text: String,
}

struct Section {
    title: String,
    rows: Vec<Row>,
}

fn is_id(s: &str) -> bool {
    let mut parts = s.splitn(2, '.');
    let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return false };
    !a.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().next().is_some_and(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The sections (`## N. Title`) and their rows (`| id | Do | Expect |`).
fn read(source: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    // Under a numbered section; a heading without a number ends it.
    let mut inside = false;
    for line in source.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            inside = title.chars().next().is_some_and(|c| c.is_ascii_digit());
            if inside {
                out.push(Section { title: title.to_owned(), rows: Vec::new() });
            }
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split(" | ").map(str::trim).collect();
        if let (true, Some(section), [id, act, expect]) = (inside, out.last_mut(), cells.as_slice()) {
            if is_id(id) {
                section.rows.push(Row { id: (*id).to_owned(), text: format!("{act} → {expect}") });
            }
        }
    }
    out
}

/// The marks in the list as it is: id to `x` or `~`.
fn marks(old: &str) -> HashMap<String, char> {
    old.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("- [")?;
            let mark = rest.chars().next()?;
            let id = rest.get(3..)?.trim_start().strip_prefix("**")?.split("**").next()?;
            (mark == 'x' || mark == '~').then(|| (id.to_owned(), mark))
        })
        .collect()
}

fn write(sections: &[Section], marks: &HashMap<String, char>) -> String {
    let mut out = String::from(HEAD);
    for s in sections {
        out.push_str(&format!("\n## {}\n\n", s.title));
        for r in &s.rows {
            let mark = marks.get(&r.id).copied().unwrap_or(' ');
            out.push_str(&format!("- [{mark}] **{}** {}\n", r.id, r.text));
        }
    }
    out
}

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let arg = std::env::args().nth(1).unwrap_or_default();
    let Ok(source) = std::fs::read_to_string(root.join(SOURCE)) else {
        eprintln!("{SOURCE} not found");
        return ExitCode::FAILURE;
    };
    let old = std::fs::read_to_string(root.join(OUT)).unwrap_or_default();
    let sections = read(&source);
    let kept = marks(&old);
    let new = write(&sections, &kept);
    match arg.as_str() {
        "--check" => {
            if new == old {
                ExitCode::SUCCESS
            } else {
                eprintln!("{OUT} is out of date: cargo run -p tsumugi --example make-testcheck");
                ExitCode::FAILURE
            }
        }
        "--stats" => {
            let (mut done, mut looked, mut all) = (0, 0, 0);
            for s in &sections {
                let d = s.rows.iter().filter(|r| kept.get(&r.id) == Some(&'x')).count();
                let l = s.rows.iter().filter(|r| kept.get(&r.id) == Some(&'~')).count();
                println!("{d:>3} / {:<3} {}{}", s.rows.len(), s.title, if l > 0 { format!(" ({l} [~])") } else { String::new() });
                (done, looked, all) = (done + d, looked + l, all + s.rows.len());
            }
            println!("{done} / {all} checked, {looked} judged from a screenshot");
            ExitCode::SUCCESS
        }
        _ => match std::fs::write(root.join(OUT), new) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{OUT}: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_read_and_ticks_kept() {
        let src = "# T\n\n## 1. One\n\n| # | Do | Expect |\n| --- | --- | --- |\n| 1.1 | Press `a` | It works |\n| 1.2a | b | c |\n\n## How to\n| 9.9 | not | a section |\n";
        let s = read(src);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["1.1", "1.2a"]);
        let first = write(&s, &HashMap::new());
        assert!(first.contains("- [ ] **1.1** Press `a` → It works"));
        let ticked = first.replace("- [ ] **1.2a**", "- [~] **1.2a**").replace("- [ ] **1.1**", "- [x] **1.1**");
        assert_eq!(write(&s, &marks(&ticked)), ticked, "ticks survive");
    }
}
