//! Rollup storage format: one KV value per point per (simulated) day, 5-minute buckets.
//!
//! `ts.<point>.<day>` holds 288 slots. Each slot keeps a running mean so several samples
//! landing in one bucket (a slow simulation speed) average instead of overwrite.
use serde::{Deserialize, Serialize};

pub const STEP_S: i64 = 300;
pub const SLOTS: usize = (86_400 / STEP_S) as usize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub day: i64,
    /// Bucket means; `null` = no sample yet.
    pub v: Vec<Option<f32>>,
    /// Samples per bucket (for the running mean).
    pub n: Vec<u16>,
    /// Time of the newest sample. A sample older than this means the clock went back (the
    /// simulator restarted, or a site replays history): everything in the chunk is from another
    /// timeline and is dropped. Chunks written before this field existed count as stale.
    #[serde(default = "stale")]
    pub last: i64,
}

fn stale() -> i64 {
    i64::MAX
}

pub fn day_of(t_ms: i64) -> i64 {
    t_ms.div_euclid(86_400_000)
}

pub fn slot_of(t_ms: i64) -> usize {
    (t_ms.rem_euclid(86_400_000) / (STEP_S * 1000)) as usize
}

impl Chunk {
    pub fn new(day: i64) -> Self {
        Chunk { day, v: vec![None; SLOTS], n: vec![0; SLOTS], last: i64::MIN }
    }
    pub fn add(&mut self, t_ms: i64, value: f64) {
        if t_ms < self.last.saturating_sub(STEP_S * 1000) {
            *self = Chunk::new(self.day);
        }
        let s = slot_of(t_ms);
        // Nothing can be newer than the newest sample: clear anything after it.
        for k in s + 1..SLOTS {
            self.v[k] = None;
            self.n[k] = 0;
        }
        self.last = self.last.max(t_ms);
        let n = self.n[s] as f64;
        let prev = self.v[s].map(f64::from).unwrap_or(0.0);
        self.v[s] = Some(((prev * n + value) / (n + 1.0)) as f32);
        self.n[s] = self.n[s].saturating_add(1);
    }
}

/// Concatenate chunks (oldest first) into a regular series ending at `end_ms`,
/// filling gaps by carrying the last value forward. Returns (start_ms, values).
pub fn assemble(chunks: &[Chunk], end_ms: i64, max_points: usize) -> (i64, Vec<f64>) {
    let mut out: Vec<(i64, f64)> = Vec::new();
    let mut sorted: Vec<&Chunk> = chunks.iter().collect();
    sorted.sort_by_key(|c| c.day);
    for c in sorted {
        for (i, v) in c.v.iter().enumerate() {
            let t = c.day * 86_400_000 + i as i64 * STEP_S * 1000;
            if t > end_ms {
                break;
            }
            if let Some(v) = v {
                out.push((t, *v as f64));
            }
        }
    }
    if out.is_empty() {
        return (end_ms, vec![]);
    }
    // Regularize from the first sample to end_ms.
    let start = out[0].0;
    let n = (((end_ms - start) / (STEP_S * 1000)) + 1).max(1) as usize;
    let mut series = Vec::with_capacity(n);
    let mut j = 0;
    let mut last = out[0].1;
    for k in 0..n {
        let t = start + k as i64 * STEP_S * 1000;
        while j < out.len() && out[j].0 <= t {
            last = out[j].1;
            j += 1;
        }
        series.push(last);
    }
    if series.len() > max_points {
        let cut = series.len() - max_points;
        return (start + cut as i64 * STEP_S * 1000, series.split_off(cut));
    }
    (start, series)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_that_goes_back_drops_the_old_timeline() {
        let day = 20_000;
        let t0 = day * 86_400_000;
        let mut c = Chunk::new(day);
        for i in 0..200 {
            c.add(t0 + i * 300_000, 5.0); // an earlier run got to slot 199
        }
        c.add(t0 + 20 * 300_000, 1.0); // restarted simulator is back at slot 20
        assert_eq!(c.v[20], Some(1.0));
        assert!(c.v[..20].iter().all(|v| v.is_none()) && c.v[21..].iter().all(|v| v.is_none()));
        // A chunk from before `last` existed is treated as stale.
        let old: Chunk = serde_json::from_str(&serde_json::to_string(&serde_json::json!({"day": day, "v": vec![Some(5.0); SLOTS], "n": vec![1; SLOTS]})).unwrap()).unwrap();
        let mut old = old;
        old.add(t0 + 300_000, 2.0);
        assert_eq!(old.v.iter().filter(|v| v.is_some()).count(), 1);
        // Same timeline: slots fill forward and average within a slot.
        let mut c = Chunk::new(day);
        c.add(t0, 1.0);
        c.add(t0 + 10_000, 3.0);
        c.add(t0 + 300_000, 4.0);
        assert_eq!((c.v[0], c.v[1]), (Some(2.0), Some(4.0)));
    }
}
