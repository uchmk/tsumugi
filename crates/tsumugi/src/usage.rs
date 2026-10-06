//! How many tokens Claude Code has used (v1-scope 1d, and the idea's 5):
//! today's, and each conversation's, read from the transcripts it writes
//! under `~/.claude/projects/` (`CLAUDE_CONFIG_DIR` moves them). Each
//! answer's line carries its `usage`; one answer can be several lines with
//! the same message id, counted once. Read on a thread, from where the last
//! reading stopped.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, NaiveDate};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tokens {
    pub input: u64,
    /// Written to the prompt cache.
    pub cache_write: u64,
    /// Read from it: cheap, and many, so kept apart from the total.
    pub cache_read: u64,
    pub output: u64,
}

impl Tokens {
    /// What was sent and made, without the cache's reads.
    pub fn total(&self) -> u64 {
        self.input + self.cache_write + self.output
    }

    fn add(&mut self, o: Tokens) {
        self.input += o.input;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
        self.output += o.output;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Usage {
    /// Today, local time, across every conversation.
    pub today: Tokens,
    /// By conversation id (the transcript's name), all of it.
    pub conversations: HashMap<String, Tokens>,
}

/// `12.3k`, `1.2M`: a count said short.
pub fn short(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// An answer's line: its message id, the local day it came, its tokens.
/// `None` for any other line.
fn line(text: &str) -> Option<(String, NaiveDate, Tokens)> {
    let at = text.find("\"usage\":{")?;
    let usage = &text[at..];
    let n = |key: &str| number(usage, key).unwrap_or(0);
    let tokens = Tokens { input: n("input_tokens"), cache_write: n("cache_creation_input_tokens"), cache_read: n("cache_read_input_tokens"), output: n("output_tokens") };
    let id = string(text, "\"id\":\"msg_").map(|s| format!("msg_{s}")).unwrap_or_default();
    let when = string(text, "\"timestamp\":\"")?;
    let day = DateTime::parse_from_rfc3339(&when).ok()?.with_timezone(&Local).date_naive();
    Some((id, day, tokens))
}

/// The number after `"key":` in `json`, the first one.
fn number(json: &str, key: &str) -> Option<u64> {
    let at = json.find(&format!("\"{key}\":"))? + key.len() + 3;
    let digits: String = json[at..].trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// What follows `start` up to the next quote.
fn string(json: &str, start: &str) -> Option<String> {
    let at = json.find(start)? + start.len();
    Some(json[at..].split('"').next()?.to_owned())
}

/// One transcript as read so far.
#[derive(Default)]
struct File {
    /// Where the next reading starts: after the last whole line.
    offset: u64,
    seen: HashSet<String>,
    total: Tokens,
    days: HashMap<NaiveDate, Tokens>,
}

impl File {
    fn take(&mut self, text: &str) {
        for l in text.lines() {
            let Some((id, day, t)) = line(l) else { continue };
            if !id.is_empty() && !self.seen.insert(id) {
                continue;
            }
            self.total.add(t);
            self.days.entry(day).or_default().add(t);
        }
    }

    /// Read what was added since the last time.
    fn read_more(&mut self, path: &Path) {
        let Ok(mut f) = std::fs::File::open(path) else { return };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            // Rewritten: from the start again.
            *self = File::default();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if f.read_to_end(&mut bytes).is_err() {
            return;
        }
        // Only whole lines; a line being written is read next time.
        let Some(end) = bytes.iter().rposition(|b| *b == b'\n') else { return };
        self.take(&String::from_utf8_lossy(&bytes[..=end]));
        self.offset += end as u64 + 1;
    }
}

fn projects() -> Option<PathBuf> {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| tsumugi_mux::settings::home().map(|h| h.join(".claude")))?;
    Some(base.join("projects"))
}

/// The transcripts written to in the last week: a conversation's own in a
/// project's folder (its name is the conversation's id), and its helpers'
/// one level further down (counted in the day's total only).
fn transcripts(dir: &Path) -> Vec<(PathBuf, Option<String>)> {
    let week = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
    let recent = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t > week);
    let jsonl = |p: &Path| p.extension().is_some_and(|e| e == "jsonl");
    let mut out = Vec::new();
    let Ok(projects) = std::fs::read_dir(dir) else { return out };
    for project in projects.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let Ok(entries) = std::fs::read_dir(&project) else { continue };
        for p in entries.flatten().map(|e| e.path()) {
            if p.is_dir() {
                let inner = std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path());
                let deeper = inner.flat_map(|q| if q.is_dir() { std::fs::read_dir(&q).into_iter().flatten().flatten().map(|e| e.path()).collect() } else { vec![q] });
                out.extend(deeper.filter(|q| jsonl(q) && recent(q)).map(|q| (q, None)));
            } else if jsonl(&p) && recent(&p) {
                let id = p.file_stem().map(|s| s.to_string_lossy().into_owned());
                out.push((p, id));
            }
        }
    }
    out
}

pub struct Watcher {
    latest: Arc<Mutex<Usage>>,
}

impl Watcher {
    /// Read now and every 20 seconds after; `wake` when something changed.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let latest = Arc::new(Mutex::new(Usage::default()));
        let out = latest.clone();
        let _ = std::thread::Builder::new().name("usage".into()).spawn(move || {
            let mut files: HashMap<PathBuf, (File, Option<String>)> = HashMap::new();
            loop {
                if let Some(dir) = projects() {
                    for (path, id) in transcripts(&dir) {
                        files.entry(path.clone()).or_insert_with(|| (File::default(), id)).0.read_more(&path);
                    }
                }
                let today = Local::now().date_naive();
                let mut u = Usage::default();
                for (f, id) in files.values() {
                    if let Some(t) = f.days.get(&today) {
                        u.today.add(*t);
                    }
                    if let Some(id) = id {
                        u.conversations.insert(id.clone(), f.total);
                    }
                }
                let changed = out.lock().map(|mut l| {
                    let changed = l.today != u.today || l.conversations != u.conversations;
                    *l = u;
                    changed
                });
                if changed.unwrap_or(false) {
                    wake();
                }
                std::thread::sleep(Duration::from_secs(20));
            }
        });
        Self { latest }
    }

    pub fn get(&self) -> Usage {
        self.latest.lock().map(|u| u.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"{"message":{"id":"msg_011Cf","role":"assistant","usage":{"cache_creation":{"ephemeral_1h_input_tokens":39581},"cache_creation_input_tokens":39581,"cache_read_input_tokens":42941,"input_tokens":2,"iterations":[{"input_tokens":2,"output_tokens":492}],"output_tokens":492}},"sessionId":"abc","timestamp":"2026-10-06T03:57:00.000Z","type":"assistant"}"#;

    #[test]
    fn an_answers_tokens_are_read_once() {
        let (id, _, t) = line(ANSWER).expect("an answer");
        assert_eq!(id, "msg_011Cf");
        assert_eq!(t, Tokens { input: 2, cache_write: 39581, cache_read: 42941, output: 492 });
        assert_eq!(t.total(), 40075, "the cache's reads apart");
        assert!(line(r#"{"type":"user","message":{"content":"hi"},"timestamp":"2026-10-06T03:57:00Z"}"#).is_none());
        let mut f = File::default();
        f.take(&format!("{ANSWER}\n{ANSWER}\nnot json\n"));
        assert_eq!(f.total.output, 492, "the same message twice is one answer");
        assert_eq!(f.days.values().map(|t| t.output).sum::<u64>(), 492);
    }

    #[test]
    fn counts_are_said_short() {
        assert_eq!(short(950), "950");
        assert_eq!(short(12_345), "12.3k");
        assert_eq!(short(1_250_000), "1.2M");
    }

    #[test]
    fn a_transcript_is_read_on_from_where_it_stopped() {
        let dir = std::env::temp_dir().join(format!("tsumugi-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("conv.jsonl");
        std::fs::write(&path, format!("{ANSWER}\n")).unwrap();
        let mut f = File::default();
        f.read_more(&path);
        assert_eq!(f.total.output, 492);
        let second = ANSWER.replace("msg_011Cf", "msg_2");
        // Half a line: left for next time.
        std::fs::write(&path, format!("{ANSWER}\n{}", &second[..40])).unwrap();
        f.read_more(&path);
        assert_eq!(f.total.output, 492);
        std::fs::write(&path, format!("{ANSWER}\n{second}\n")).unwrap();
        f.read_more(&path);
        assert_eq!(f.total.output, 984);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
