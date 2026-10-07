//! What Claude Code's tokens cost, as an estimate: each answer's tokens at
//! its model's prices. The window's own prices are Anthropic's first-party
//! API rates as of 2026-09-25; `[prices]` in the settings changes any of
//! them or adds a model. A subscription (Pro, Max) is not billed this way:
//! the figure is what the same work would cost on the API.

use std::sync::RwLock;

use crate::usage::Tokens;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    /// US dollars per million tokens.
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

const fn p(input: f64, output: f64, cache_read: f64) -> Price {
    Price { input, output, cache_read }
}

/// By the start of a model's id; the longest that fits is taken.
const OWN: [(&str, Price); 13] = [
    ("claude-fable-5-1", p(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", p(10.0, 50.0, 0.25)),
    ("claude-fable-5", p(10.0, 50.0, 1.0)),
    ("claude-opus-5-5", p(4.0, 20.0, 0.20)),
    ("claude-opus-5", p(5.0, 25.0, 0.50)),
    ("claude-opus-4-8", p(5.0, 25.0, 0.50)),
    ("claude-opus-4-7", p(5.0, 25.0, 0.50)),
    ("claude-opus-4-6", p(5.0, 25.0, 0.50)),
    ("claude-sonnet-5-5", p(2.0, 10.0, 0.20)),
    ("claude-sonnet-5", p(2.0, 10.0, 0.20)),
    ("claude-sonnet-4-6", p(3.0, 15.0, 0.30)),
    ("claude-sonnet-4-5", p(3.0, 15.0, 0.30)),
    ("claude-haiku-4-5", p(1.0, 5.0, 0.10)),
];

/// `[prices]` from the settings.
static SET: RwLock<Vec<(String, Price)>> = RwLock::new(Vec::new());

/// Take the settings' prices (a cache read a tenth of the input unless given).
pub fn set(prices: &std::collections::BTreeMap<String, tsumugi_mux::settings::Price>) {
    let list = prices.iter().map(|(m, s)| (m.clone(), Price { input: s.input, output: s.output, cache_read: s.cache_read.unwrap_or(s.input / 10.0) })).collect();
    if let Ok(mut l) = SET.write() {
        *l = list;
    }
}

/// The prices of `model`: the settings' first, then the window's own; the
/// longest start that fits wins. `None` for a model not known.
pub fn of(model: &str) -> Option<Price> {
    let set = SET.read().map(|l| l.clone()).unwrap_or_default();
    let own = OWN.iter().map(|(m, p)| (m.to_string(), *p));
    set.into_iter().chain(own).filter(|(m, _)| model.starts_with(m.as_str())).max_by_key(|(m, _)| m.len()).map(|(_, p)| p)
}

/// What these tokens cost at `price`: cache writes at 1.25 times the input,
/// the hour-long cache's at 2 times.
pub fn cost(price: Price, t: &Tokens) -> f64 {
    let short_writes = t.cache_write.saturating_sub(t.cache_write_1h) as f64;
    let dollars = t.input as f64 * price.input + short_writes * price.input * 1.25 + t.cache_write_1h as f64 * price.input * 2.0 + t.cache_read as f64 * price.cache_read + t.output as f64 * price.output;
    dollars / 1e6
}

/// `$0.42`, `$12.30`, `$1,204`, `<$0.01`.
pub fn dollars(x: f64) -> String {
    if x <= 0.0 {
        "$0".into()
    } else if x < 0.01 {
        "<$0.01".into()
    } else if x < 1000.0 {
        format!("${x:.2}")
    } else {
        let whole = x.round() as u64;
        let s = whole.to_string();
        let mut out = String::new();
        for (k, c) in s.chars().enumerate() {
            if k > 0 && (s.len() - k).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
        }
        format!("${out}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_are_found_by_the_longest_start() {
        assert_eq!(of("claude-opus-5-5").map(|p| p.input), Some(4.0));
        assert_eq!(of("claude-opus-5-20260101").map(|p| p.input), Some(5.0), "not Opus 5.5's");
        assert_eq!(of("claude-fable-5-1").map(|p| p.cache_read), Some(0.25));
        assert_eq!(of("gpt-x"), None);
        let t = Tokens { input: 1_000_000, cache_write: 2_000_000, cache_write_1h: 1_000_000, cache_read: 10_000_000, output: 1_000_000 };
        // 4 + 4 × 1.25 + 4 × 2 + 10 × 0.2 + 20
        assert!((cost(of("claude-opus-5-5").unwrap(), &t) - 39.0).abs() < 1e-9);
        assert_eq!((dollars(0.004), dollars(0.42), dollars(12.3), dollars(1204.4)), ("<$0.01".into(), "$0.42".into(), "$12.30".into(), "$1,204".into()));
    }
}
