//! Answering a question from the sidebar: when a session waits on a menu of
//! numbered choices -- Claude Code's "Do you want to proceed? 1. Yes 2. …
//! 3. No" -- its card shows them as buttons, and a click types the number.
//! Only what is on the session's screen is offered, so nothing is typed
//! into a session that is not asking.

/// One numbered choice on the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// What to type: the choice's number.
    pub key: char,
    pub text: String,
}

/// The last menu of numbered choices on the screen's last lines: two or
/// more lines `1. …`, `2. …` in order, a pointer (`❯`) and a box's sides
/// aside. Empty when there is none.
pub fn choices(lines: &[String]) -> Vec<Choice> {
    let mut found: Vec<Choice> = Vec::new();
    let mut best = Vec::new();
    for line in lines.iter().rev().take(20).collect::<Vec<_>>().into_iter().rev() {
        match choice(line) {
            Some(c) if c.key == next(&found) => found.push(c),
            Some(c) if c.key == '1' => {
                keep(&mut best, &mut found);
                found.push(c);
            }
            // A choice's text run over onto the next line.
            None if !found.is_empty() && is_continuation(line) => {}
            _ => keep(&mut best, &mut found),
        }
    }
    keep(&mut best, &mut found);
    best
}

/// A run of two or more is a menu; the last one wins.
fn keep(best: &mut Vec<Choice>, found: &mut Vec<Choice>) {
    if found.len() >= 2 {
        *best = std::mem::take(found);
    } else {
        found.clear();
    }
}

fn next(found: &[Choice]) -> char {
    char::from_digit(found.len() as u32 + 1, 10).unwrap_or('?')
}

fn strip_box(line: &str) -> &str {
    line.trim().trim_start_matches(['│', '┃', '|']).trim_end_matches(['│', '┃', '|']).trim()
}

fn is_continuation(line: &str) -> bool {
    let s = strip_box(line);
    !s.is_empty() && line.starts_with([' ', '│']) && !s.starts_with(|c: char| c.is_ascii_digit())
}

fn choice(line: &str) -> Option<Choice> {
    let s = strip_box(line).trim_start_matches(['❯', '>', '›', '▶']).trim_start();
    let mut chars = s.chars();
    let key = chars.next().filter(|c| ('1'..='9').contains(c))?;
    let rest = chars.as_str().strip_prefix('.')?.trim();
    (!rest.is_empty()).then(|| Choice { key, text: rest.to_owned() })
}

/// A short name for a choice's button: up to its first comma or bracket,
/// cut to a few words.
pub fn short(text: &str) -> String {
    let head = text.split([',', '(', '—']).next().unwrap_or(text).trim();
    let words: Vec<&str> = head.split_whitespace().take(3).collect();
    let s = words.join(" ");
    if s.chars().count() > 18 { format!("{}…", s.chars().take(17).collect::<String>()) } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn claude_codes_question_is_found() {
        let screen = lines(
            "╭──────────────────────────────╮\n\
             │ Bash command                 │\n\
             │   rm -rf target              │\n\
             │ Do you want to proceed?      │\n\
             │ ❯ 1. Yes                     │\n\
             │   2. Yes, and don't ask again for rm commands in /home/u/dev │\n\
             │   3. No, and tell Claude what to do differently (esc) │\n\
             ╰──────────────────────────────╯",
        );
        let c = choices(&screen);
        assert_eq!(c.iter().map(|c| c.key).collect::<String>(), "123");
        assert_eq!(c[0].text, "Yes");
        assert_eq!(short(&c[1].text), "Yes");
        assert_eq!(short(&c[2].text), "No");
        assert_eq!(short("Don't ask again for this whole long thing"), "Don't ask again");
    }

    #[test]
    fn a_list_in_the_output_that_is_not_last_or_not_in_order_is_not_a_menu() {
        assert!(choices(&lines("1. only one\nroot@vm:~#")).is_empty(), "one line is no menu");
        assert!(choices(&lines("2. two\n3. three")).is_empty(), "not from 1");
        let c = choices(&lines("1. old\n2. list\nsome output\n1. Yes\n2. No"));
        assert_eq!(c.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Yes", "No"], "the last menu");
        assert!(choices(&lines("1. a\n3. c")).is_empty(), "a gap");
    }
}
