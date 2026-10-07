//! Telling once when the estimated cost passes a line (`[notify] spend_day`
//! and `spend_block`): the day's, and Claude Code's 5-hour block's. Each is
//! told once: the day's again only after the day's figure starts over, the
//! block's again only in the next block.

use crate::usage::Usage;

#[derive(Default)]
pub struct Watch {
    /// The day's line was told today.
    day_told: bool,
    /// The day's figure last seen: a smaller one is a new day.
    day_last: f64,
    /// The start of the block told about.
    block_told: Option<i64>,
}

impl Watch {
    /// What to say now, if anything: `day` and `block` in dollars, 0 for
    /// never.
    pub fn check(&mut self, used: &Usage, day: u64, block: u64) -> Vec<String> {
        let mut out = Vec::new();
        if used.today_cost < self.day_last {
            self.day_told = false;
        }
        self.day_last = used.today_cost;
        if day > 0 && !self.day_told && used.today_cost >= day as f64 {
            self.day_told = true;
            out.push(format!("Today's Claude Code use passed ${day}: about {} at API prices", crate::price::dollars(used.today_cost)));
        }
        if let Some(b) = &used.block {
            if block > 0 && self.block_told != Some(b.start_ms) && b.cost >= block as f64 {
                self.block_told = Some(b.start_ms);
                out.push(format!("This 5-hour block passed ${block}: about {} at API prices", crate::price::dollars(b.cost)));
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
        let mut w = Watch::default();
        let mut used = Usage { today_cost: 5.0, ..Default::default() };
        assert!(w.check(&used, 20, 10).is_empty());
        used.today_cost = 21.0;
        used.block = Some(Block { start_ms: 1, end_ms: 2, tokens: Tokens::default(), cost: 12.0 });
        let said = w.check(&used, 20, 10);
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(said[0].contains("$20") && said[0].contains("$21.00"), "{said:?}");
        used.today_cost = 30.0;
        assert!(w.check(&used, 20, 10).is_empty(), "not again the same day or block");
        used.today_cost = 25.0;
        used.block = Some(Block { start_ms: 3, end_ms: 4, tokens: Tokens::default(), cost: 11.0 });
        assert_eq!(w.check(&used, 20, 10).len(), 2, "a new day (the figure fell) and a new block");
        assert!(w.check(&used, 0, 0).is_empty(), "0 is never");
    }
}
