//! Telling once when the estimated cost passes a line (`[notify] spend_day`
//! and `spend_block`): the day's, and Claude Code's 5-hour block's. Each is
//! told once: the day's again only on the next day (by the date: a lower
//! figure, after `[prices]` changed, is no new day), the block's again only
//! in the next block.

use crate::i18n::trf;
use crate::usage::Usage;

#[derive(Default)]
pub struct Watch {
    /// The day the day's line was told on.
    day_told: Option<chrono::NaiveDate>,
    /// The start of the block told about.
    block_told: Option<i64>,
}

impl Watch {
    /// What to say now, if anything: `day` and `block` in dollars, 0 for
    /// never.
    pub fn check(&mut self, used: &Usage, day: u64, block: u64) -> Vec<String> {
        self.check_on(chrono::Local::now().date_naive(), used, day, block)
    }

    /// `check` on the day `today`.
    fn check_on(&mut self, today: chrono::NaiveDate, used: &Usage, day: u64, block: u64) -> Vec<String> {
        let mut out = Vec::new();
        if day > 0 && self.day_told != Some(today) && used.today_cost >= day as f64 {
            self.day_told = Some(today);
            out.push(trf("toast.spend_day", &[&day.to_string(), &crate::price::dollars(used.today_cost)]));
        }
        if let Some(b) = &used.block {
            if block > 0 && self.block_told != Some(b.start_ms) && b.cost >= block as f64 {
                self.block_told = Some(b.start_ms);
                out.push(trf("toast.spend_block", &[&block.to_string(), &crate::price::dollars(b.cost)]));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::{Block, Tokens};

    #[test]
    fn each_line_is_told_once() {
        let d1 = chrono::NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let d2 = d1.succ_opt().unwrap();
        let mut w = Watch::default();
        let mut used = Usage { today_cost: 5.0, ..Default::default() };
        assert!(w.check_on(d1, &used, 20, 10).is_empty());
        used.today_cost = 21.0;
        used.block = Some(Block { start_ms: 1, end_ms: 2, tokens: Tokens::default(), cost: 12.0 });
        let said = w.check_on(d1, &used, 20, 10);
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(said[0].contains("$20") && said[0].contains("$21.00"), "{said:?}");
        used.today_cost = 30.0;
        assert!(w.check_on(d1, &used, 20, 10).is_empty(), "not again the same day or block");
        // Lower prices the same day: the figure falls and climbs back, and
        // it is still the same day (the source review, 2026-10-07).
        used.today_cost = 19.0;
        assert!(w.check_on(d1, &used, 20, 10).is_empty());
        used.today_cost = 22.0;
        assert!(w.check_on(d1, &used, 20, 10).is_empty(), "not twice on one date");
        used.block = Some(Block { start_ms: 3, end_ms: 4, tokens: Tokens::default(), cost: 11.0 });
        assert_eq!(w.check_on(d2, &used, 20, 10).len(), 2, "the next day, and a new block");
        assert!(w.check_on(d2, &used, 0, 0).is_empty(), "0 is never");
    }
}
