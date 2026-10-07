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
    /// Of those, written to the hour-long cache (priced higher).
    pub cache_write_1h: u64,
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
        self.cache_write_1h += o.cache_write_1h;
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
    /// By conversation id: how many prompts were typed, and how long from
    /// its first line to its last (the selected card's numbers).
    pub talks: HashMap<String, Talk>,
    /// Claude Code's five-hour window now, if one is open.
    pub block: Option<Block>,
    /// What today's tokens would cost on the API, in US dollars (`price.rs`),
    /// the models with no price left out.
    pub today_cost: f64,
    /// The same, by conversation.
    pub costs: HashMap<String, f64>,
}

/// Claude Code's usage window: it opens at the hour of the first answer
/// after the last one closed, and lasts five hours. Worked out from the
/// transcripts' answers, so an estimate: the limit itself is the plan's,
/// and is not written anywhere tsumugi can read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Block {
    pub start_ms: i64,
    pub end_ms: i64,
    pub tokens: Tokens,
    /// What they would cost on the API (`price.rs`).
    pub cost: f64,
}

/// How long a window lasts.
const BLOCK_MS: i64 = 5 * 3600 * 1000;

/// The window open at `now`, from the answers' times, tokens and costs
/// (any order).
pub fn block(answers: &[(i64, Tokens, f64)], now: i64) -> Option<Block> {
    let mut sorted: Vec<&(i64, Tokens, f64)> = answers.iter().collect();
    sorted.sort_by_key(|(t, ..)| *t);
    let mut open: Option<Block> = None;
    for (t, tokens, cost) in sorted {
        let mut b = match open {
            Some(b) if *t < b.end_ms => b,
            _ => {
                let start = t - t.rem_euclid(3600 * 1000);
                Block { start_ms: start, end_ms: start + BLOCK_MS, tokens: Tokens::default(), cost: 0.0 }
            }
        };
        b.tokens.add(*tokens);
        b.cost += cost;
        open = Some(b);
    }
    open.filter(|b| now < b.end_ms)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Talk {
    pub prompts: u32,
    pub ms: u64,
}

/// A line that is a prompt someone typed: the user's, its content words,
/// not a tool's result (a list) or a command's output.
fn is_prompt(text: &str) -> bool {
    text.contains("\"type\":\"user\"") && text.contains("\"role\":\"user\",\"content\":\"") && !text.contains("\"isMeta\":true") && !text.contains("<command-") && !text.contains("<local-command")
}

/// A line's time, in Unix milliseconds.
fn stamp(text: &str) -> Option<i64> {
    let when = string(text, "\"timestamp\":\"")?;
    Some(DateTime::parse_from_rfc3339(&when).ok()?.timestamp_millis())
}

/// `12.3k`, `1.2M`: a count said short.
pub fn short(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// An answer: its message id, the local day it came, its time in Unix
/// milliseconds, its model and its tokens.
struct Answer {
    id: String,
    day: NaiveDate,
    ms: i64,
    model: String,
    tokens: Tokens,
}

/// An answer's line; `None` for any other line. Read as JSON, so the
/// numbers are the message's own `usage` and not the first of the name
/// anywhere on the line: a tool's result can hold a helper's `usage`, and
/// `iterations` repeats `output_tokens` before the total (the source
/// review, 2026-10-07). Only Claude Code's `assistant` lines are answers.
fn line(text: &str) -> Option<Answer> {
    if !text.contains("\"usage\"") || !text.contains("\"assistant\"") {
        return None;
    }
    let v = crate::json::Json::parse(text).ok()?;
    if v.get("type").and_then(crate::json::Json::string) != Some("assistant") {
        return None;
    }
    let message = v.get("message")?;
    let usage = message.get("usage")?;
    let n = |o: Option<&crate::json::Json>, key: &str| match o.and_then(|o| o.get(key)) {
        Some(crate::json::Json::Number(x)) if *x >= 0.0 => *x as u64,
        _ => 0,
    };
    let tokens = Tokens {
        input: n(Some(usage), "input_tokens"),
        cache_write: n(Some(usage), "cache_creation_input_tokens"),
        cache_write_1h: n(usage.get("cache_creation"), "ephemeral_1h_input_tokens"),
        cache_read: n(Some(usage), "cache_read_input_tokens"),
        output: n(Some(usage), "output_tokens"),
    };
    let text_of = |o: Option<&crate::json::Json>| o.and_then(crate::json::Json::string).map(str::to_owned);
    let id = text_of(message.get("id")).filter(|i| i.starts_with("msg_")).unwrap_or_default();
    let at = DateTime::parse_from_rfc3339(&text_of(v.get("timestamp"))?).ok()?;
    let model = text_of(message.get("model")).or_else(|| text_of(v.get("model"))).unwrap_or_default();
    Some(Answer { id, day: at.with_timezone(&Local).date_naive(), ms: at.timestamp_millis(), model, tokens })
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
    prompts: u32,
    /// The first and the last line's time.
    span: Option<(i64, i64)>,
    /// The answers of the last day, by time and model: the five-hour window.
    recent: Vec<(i64, String, Tokens)>,
    /// By model: all of it, and each day's (what they cost).
    models: HashMap<String, Tokens>,
    day_models: HashMap<(NaiveDate, String), Tokens>,
}

impl File {
    fn take(&mut self, text: &str) {
        for l in text.lines() {
            if is_prompt(l) {
                self.prompts += 1;
            }
            if let Some(t) = stamp(l) {
                self.span = Some(self.span.map_or((t, t), |(a, b)| (a.min(t), b.max(t))));
            }
            let Some(a) = line(l) else { continue };
            if !a.id.is_empty() && !self.seen.insert(a.id) {
                continue;
            }
            self.total.add(a.tokens);
            self.days.entry(a.day).or_default().add(a.tokens);
            self.models.entry(a.model.clone()).or_default().add(a.tokens);
            self.day_models.entry((a.day, a.model.clone())).or_default().add(a.tokens);
            self.recent.push((a.ms, a.model, a.tokens));
        }
        let day_ago = chrono::Utc::now().timestamp_millis() - 24 * 3600 * 1000;
        self.recent.retain(|(ms, ..)| *ms > day_ago);
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
    /// Shared, not copied: the window asks for it several times a frame.
    latest: Arc<Mutex<Arc<Usage>>>,
}

impl Watcher {
    /// Read now and every 20 seconds after; `wake` when something changed.
    pub fn start(wake: impl Fn() + Send + 'static) -> Self {
        let latest = Arc::new(Mutex::new(Arc::new(Usage::default())));
        let out = latest.clone();
        let _ = std::thread::Builder::new().name("usage".into()).spawn(move || {
            let mut files: HashMap<PathBuf, (File, Option<String>)> = HashMap::new();
            loop {
                if let Some(dir) = projects() {
                    let now = transcripts(&dir);
                    // Those gone past the week are let go, not kept for ever.
                    files.retain(|p, _| now.iter().any(|(q, _)| q == p));
                    for (path, id) in now {
                        files.entry(path.clone()).or_insert_with(|| (File::default(), id)).0.read_more(&path);
                    }
                }
                let today = Local::now().date_naive();
                let mut u = Usage::default();
                let price = |model: &str, t: &Tokens| crate::price::of(model).map_or(0.0, |p| crate::price::cost(p, t));
                let answers: Vec<(i64, Tokens, f64)> = files.values().flat_map(|(f, _)| f.recent.iter().map(|(ms, m, t)| (*ms, *t, price(m, t)))).collect();
                u.block = block(&answers, chrono::Utc::now().timestamp_millis());
                for (f, id) in files.values() {
                    if let Some(t) = f.days.get(&today) {
                        u.today.add(*t);
                    }
                    u.today_cost += f.day_models.iter().filter(|((d, _), _)| *d == today).map(|((_, m), t)| price(m, t)).sum::<f64>();
                    if let Some(id) = id {
                        u.costs.insert(id.clone(), f.models.iter().map(|(m, t)| price(m, t)).sum());
                        u.conversations.insert(id.clone(), f.total);
                        let ms = f.span.map_or(0, |(a, b)| (b - a).max(0) as u64);
                        u.talks.insert(id.clone(), Talk { prompts: f.prompts, ms });
                    }
                }
                let changed = out.lock().map(|mut l| {
                    let changed = l.today != u.today || l.conversations != u.conversations || l.talks != u.talks || l.block != u.block || l.today_cost != u.today_cost;
                    *l = Arc::new(u);
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

    pub fn get(&self) -> Arc<Usage> {
        self.latest.lock().map(|u| u.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_hour_window_opens_at_the_hour_of_its_first_answer() {
        let h = 3600 * 1000;
        let t = |n: u64| Tokens { output: n, ..Tokens::default() };
        // 09:20 and 11:00 in one window (09:00 to 14:00); 14:10 opens the
        // next at 14:00, which 15:30 is in.
        let answers = [(9 * h + 20 * 60 * 1000, t(1), 0.5), (11 * h, t(2), 0.5), (14 * h + 10 * 60 * 1000, t(4), 1.0), (15 * h + 30 * 60 * 1000, t(8), 2.0)];
        let b = block(&answers, 16 * h).unwrap();
        assert_eq!((b.start_ms, b.end_ms, b.tokens.output, b.cost), (14 * h, 19 * h, 12, 3.0));
        assert_eq!(block(&answers[..2], 12 * h).unwrap().tokens.output, 3);
        assert_eq!(block(&answers[..2], 14 * h), None, "closed at 14:00");
        assert_eq!(block(&[], 0), None);
    }

    const ANSWER: &str = r#"{"message":{"id":"msg_011Cf","role":"assistant","usage":{"cache_creation":{"ephemeral_1h_input_tokens":39581},"cache_creation_input_tokens":39581,"cache_read_input_tokens":42941,"input_tokens":2,"iterations":[{"input_tokens":2,"output_tokens":492}],"output_tokens":492}},"model":"claude-opus-5-5","sessionId":"abc","timestamp":"2026-10-06T03:57:00.000Z","type":"assistant"}"#;

    #[test]
    fn an_answers_tokens_are_read_once() {
        let a = line(ANSWER).expect("an answer");
        let t = a.tokens;
        assert_eq!((a.id.as_str(), a.model.as_str()), ("msg_011Cf", "claude-opus-5-5"));
        assert_eq!(t, Tokens { input: 2, cache_write: 39581, cache_write_1h: 39581, cache_read: 42941, output: 492 });
        assert_eq!(t.total(), 40075, "the cache's reads apart");
        assert!(line(r#"{"type":"user","message":{"content":"hi"},"timestamp":"2026-10-06T03:57:00Z"}"#).is_none());
        // A tool's result carrying a helper's usage is no answer of this
        // conversation's; and the total, not the first iteration's, counts.
        assert!(line(r#"{"type":"user","toolUseResult":{"usage":{"output_tokens":9}},"message":{"content":"x"},"timestamp":"2026-10-06T03:57:00Z"}"#).is_none());
        let two = ANSWER.replace(r#""iterations":[{"input_tokens":2,"output_tokens":492}],"output_tokens":492"#, r#""iterations":[{"output_tokens":100},{"output_tokens":392}],"output_tokens":492"#);
        assert_eq!(line(&two).unwrap().tokens.output, 492);
        let mut f = File::default();
        f.take(&format!("{ANSWER}\n{ANSWER}\nnot json\n"));
        assert_eq!(f.total.output, 492, "the same message twice is one answer");
        assert_eq!(f.days.values().map(|t| t.output).sum::<u64>(), 492);
        assert_eq!(f.models.get("claude-opus-5-5").map(|t| t.output), Some(492), "by model, for its price");
    }

    #[test]
    fn prompts_are_counted_and_the_talk_timed() {
        let prompt = r#"{"type":"user","message":{"role":"user","content":"Split the pane"},"timestamp":"2026-10-06T03:00:00.000Z"}"#;
        let tool = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result"}]},"timestamp":"2026-10-06T03:10:00.000Z"}"#;
        let command = r#"{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"},"timestamp":"2026-10-06T03:20:00.000Z"}"#;
        let mut f = File::default();
        f.take(&format!("{prompt}\n{tool}\n{command}\n{ANSWER}\n"));
        assert_eq!(f.prompts, 1, "only what was typed");
        let (a, b) = f.span.unwrap();
        assert_eq!(b - a, 57 * 60 * 1000, "03:00 to the answer at 03:57");
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
