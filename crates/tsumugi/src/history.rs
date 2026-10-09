//! The sessions that ended, newest first: what each was, where, how many
//! tokens its conversation took, and the last lines on its screen (the
//! server sends them as it lets the session go). Kept beside the state as
//! `closed-sessions.json`, so the list outlives the window and the server.

use std::path::PathBuf;

use crate::json::Json;

/// How many ended sessions are kept.
pub const KEPT: usize = 30;

/// One ended session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Closed {
    pub title: String,
    pub command: String,
    pub cwd: PathBuf,
    /// Claude Code ran in it, and the conversation's id (to resume it).
    pub claude: bool,
    pub conversation: String,
    /// Its state's word when it ended (`done`, `error`).
    pub state: String,
    /// Tokens its conversation used, when known.
    pub tokens: u64,
    /// What they would cost on the API, in US dollars (`price.rs`).
    pub cost: f64,
    /// When it ended, in Unix milliseconds.
    pub ended_ms: u64,
    /// The last lines on its screen.
    pub last: Vec<String>,
}

/// The file beside the state.
pub fn file() -> Option<PathBuf> {
    tsumugi_mux::state::default_path().map(|p| p.with_file_name("closed-sessions.json"))
}

/// The list written out, newest first.
pub fn to_json(list: &[Closed]) -> String {
    let s = |v: &str| Json::String(v.to_owned());
    let n = |v: u64| Json::Number(v as f64);
    Json::Array(
        list.iter()
            .map(|c| {
                Json::Object(vec![
                    ("title".into(), s(&c.title)),
                    ("command".into(), s(&c.command)),
                    ("cwd".into(), s(&c.cwd.to_string_lossy())),
                    ("claude".into(), Json::Bool(c.claude)),
                    ("conversation".into(), s(&c.conversation)),
                    ("state".into(), s(&c.state)),
                    ("tokens".into(), n(c.tokens)),
                    ("cost".into(), Json::Number(c.cost)),
                    ("ended_ms".into(), n(c.ended_ms)),
                    ("last".into(), Json::Array(c.last.iter().map(|l| s(l)).collect())),
                ])
            })
            .collect(),
    )
    .pretty()
}

/// The list read back; what cannot be read is left out.
pub fn from_json(text: &str) -> Vec<Closed> {
    let Ok(json) = Json::parse(text) else { return Vec::new() };
    let Some(items) = json.array() else { return Vec::new() };
    let num = |j: &Json, k: &str| match j.get(k) {
        Some(Json::Number(n)) if *n >= 0.0 => *n as u64,
        _ => 0,
    };
    let text = |j: &Json, k: &str| j.get(k).and_then(Json::string).unwrap_or_default().to_owned();
    items
        .iter()
        .filter(|j| j.get("cwd").and_then(Json::string).is_some())
        .map(|j| Closed {
            title: text(j, "title"),
            command: text(j, "command"),
            cwd: PathBuf::from(text(j, "cwd")),
            claude: matches!(j.get("claude"), Some(Json::Bool(true))),
            conversation: text(j, "conversation"),
            state: text(j, "state"),
            tokens: num(j, "tokens"),
            cost: match j.get("cost") {
                Some(Json::Number(n)) if *n >= 0.0 => *n,
                _ => 0.0,
            },
            ended_ms: num(j, "ended_ms"),
            last: j.get("last").and_then(Json::array).map(|a| a.iter().filter_map(Json::string).map(str::to_owned).collect()).unwrap_or_default(),
        })
        .take(KEPT)
        .collect()
}

/// The list kept, or none.
pub fn load() -> Vec<Closed> {
    file().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| from_json(&t)).unwrap_or_default()
}

/// Write the list on a thread.
pub fn save(list: Vec<Closed>) {
    let _ = std::thread::Builder::new().name("closed-sessions".into()).spawn(move || {
        let Some(p) = file() else { return };
        let _ = crate::files::write_atomic(&p, to_json(&list));
    });
}

/// Put an ended session at the top, the oldest beyond [`KEPT`] dropped.
pub fn push(list: &mut Vec<Closed>, c: Closed) {
    list.insert(0, c);
    list.truncate(KEPT);
}

/// Put back sessions taken off the list (Undo), each where its end puts
/// it: the list is newest first, and sessions may have ended since.
pub fn restore(list: &mut Vec<Closed>, taken: Vec<Closed>) {
    list.extend(taken);
    list.sort_by_key(|c| std::cmp::Reverse(c.ended_ms));
    list.truncate(KEPT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_comes_back_as_it_was_written() {
        let one = Closed {
            title: "fix \"the\" bug".into(),
            command: "claude".into(),
            cwd: PathBuf::from("/home/u/dev/filer"),
            claude: true,
            conversation: "abc-1".into(),
            state: "done".into(),
            tokens: 123_456,
            cost: 1.25,
            ended_ms: 1_791_000_000_123,
            last: vec!["● Done.".into(), "".into(), "> \\ end".into()],
        };
        let two = Closed { cwd: PathBuf::from("C:\\dev\\x"), ..Closed::default() };
        let list = vec![one, two];
        assert_eq!(from_json(&to_json(&list)), list);
        assert!(from_json("not json").is_empty());
        let mut many = Vec::new();
        for k in 0..KEPT + 3 {
            push(&mut many, Closed { tokens: k as u64, ..Closed::default() });
        }
        assert_eq!(many.len(), KEPT);
        assert_eq!(many[0].tokens, (KEPT + 2) as u64, "the newest first");
    }

    #[test]
    fn undo_puts_them_back_in_their_places() {
        let at = |ms| Closed { ended_ms: ms, ..Closed::default() };
        // 30 and 10 taken off; 40 ended since.
        let mut list = vec![at(40), at(20)];
        restore(&mut list, vec![at(30), at(10)]);
        assert_eq!(list.iter().map(|c| c.ended_ms).collect::<Vec<_>>(), [40, 30, 20, 10]);
    }
}
