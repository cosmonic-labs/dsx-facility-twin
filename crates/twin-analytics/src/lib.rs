//! Forecasting with augurs: MSTL (daily seasonality) with an AutoETS trend, plus the MAD
//! peer-outlier detector. Pure functions, so they test natively and run unchanged in Wasm.
use augurs::ets::AutoETS;
use augurs::mstl::MSTLModel;
use augurs::outlier::{MADDetector, OutlierDetector};
use augurs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dir {
    High,
    Low,
}

#[derive(Debug, Clone)]
pub struct Forecast {
    /// Steps ahead at which the point forecast first crosses the threshold.
    pub cross_step: Option<usize>,
    pub point: Vec<f64>,
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
}

/// Seasonal period in samples (5-minute steps → one day).
pub const DAY: usize = 288;

/// Cheap triage before fitting: is this point close enough to its threshold, or moving
/// toward it fast enough, that a forecast could matter within `horizon` steps?
pub fn worth_fitting(y: &[f64], threshold: f64, dir: Dir, horizon: usize) -> bool {
    if y.len() < 24 {
        return false;
    }
    let n = y.len();
    let last = y[n - 1];
    let (lo, hi) = y[n.saturating_sub(DAY)..].iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let range = (hi - lo).max(1e-9);
    let gap = match dir {
        Dir::High => threshold - last,
        Dir::Low => last - threshold,
    };
    if gap <= 0.0 {
        return false; // already past: that's a condition, not a prediction
    }
    // slope over the last hour (12 samples), per step
    let k = 12.min(n - 1);
    let slope = (y[n - 1] - y[n - 1 - k]) / k as f64;
    let toward = match dir {
        Dir::High => slope,
        Dir::Low => -slope,
    };
    // within one daily swing of the threshold, or trending there inside the horizon
    gap < range * 1.2 || toward * horizon as f64 > gap * 0.5
}

/// Fit MSTL(daily) + AutoETS trend and forecast `horizon` steps.
/// Clip past incidents out of the history so the seasonal fit does not learn them.
/// Samples older than the last `RECENT` steps are held to a band around the typical value for
/// their time of day (median across days of the samples within an hour either side, or the overall median with under two days of data);
/// the band is 5·MAD of the residuals. The recent window is left untouched so a developing
/// problem still shows.
const RECENT: usize = 24;
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}
pub fn clean_history(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n <= RECENT + 12 {
        return y.to_vec();
    }
    let typical: Vec<f64> = if n >= DAY * 2 {
        // phase measured back from the newest sample, so "same time yesterday" lines up
        // Typical value for a time of day: the median, over every day, of the samples within an
        // hour either side. One bad afternoon is a minority of that pool and cannot move it.
        const W: usize = 12;
        let mut by_phase: Vec<Vec<f64>> = vec![vec![]; DAY];
        for (i, v) in y.iter().enumerate() {
            by_phase[(n - 1 - i) % DAY].push(*v);
        }
        let med: Vec<f64> = (0..DAY)
            .map(|ph| {
                let mut pool: Vec<f64> = (0..=2 * W).flat_map(|k| by_phase[(ph + DAY + k - W) % DAY].iter().copied()).collect();
                median(&mut pool)
            })
            .collect();
        (0..n).map(|i| med[(n - 1 - i) % DAY]).collect()
    } else {
        let m = median(&mut y.to_vec());
        vec![m; n]
    };
    let mut dev: Vec<f64> = y.iter().zip(&typical).map(|(v, t)| (v - t).abs()).collect();
    let band = 5.0 * 1.4826 * median(&mut dev) + 1e-9;
    y.iter().zip(&typical).enumerate().map(|(i, (v, t))| if i + RECENT < n { v.clamp(t - band, t + band) } else { *v }).collect()
}

pub fn forecast(y: &[f64], horizon: usize, threshold: f64, dir: Dir) -> Result<Forecast, String> {
    let cleaned = clean_history(y);
    let y = cleaned.as_slice();
    let periods = if y.len() >= DAY * 2 { vec![DAY] } else { vec![] };
    let trend = AutoETS::non_seasonal().into_trend_model();
    let fit = MSTLModel::new(periods, trend).fit(y).map_err(|e| e.to_string())?;
    let fc = fit.predict(horizon, 0.8).map_err(|e| e.to_string())?;
    // ETS damps trends the plant really has (bearing wear, a leak). Take the worse of the
    // augurs forecast and the last hour's least-squares trend, in the direction of the limit.
    let n = y.len();
    let mut k = 12.min(n);
    // A level shift (a breaker trips and the other cord picks up the rack) is a step, not a
    // trend: when one jump explains most of the last hour's change, fit only what came after it.
    {
        let w = &y[n - k..];
        let total = (w[k - 1] - w[0]).abs();
        if let Some((j, d)) = w.windows(2).map(|p| (p[1] - p[0]).abs()).enumerate().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)) {
            if total > 1e-9 && d > 0.6 * total {
                k = k - (j + 1);
            }
        }
    }
    let xs: Vec<f64> = (0..k).map(|i| i as f64).collect();
    let ys = &y[n - k.max(1)..];
    let (mx, my) = (xs.iter().sum::<f64>() / k.max(1) as f64, ys.iter().sum::<f64>() / ys.len() as f64);
    let sxx: f64 = if k >= 3 { xs.iter().map(|x| (x - mx).powi(2)).sum() } else { 0.0 };
    let slope = if sxx > 0.0 { xs.iter().zip(ys).map(|(x, v)| (x - mx) * (v - my)).sum::<f64>() / sxx } else { 0.0 };
    let last = y[n - 1];
    // A recovery ramp (a rack powering back up after a trip) is not a trend: if the last hour
    // started from far below the day's typical level, trust augurs alone.
    let mut sorted: Vec<f64> = y[n.saturating_sub(DAY)..].to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let recovering = ys.iter().cloned().fold(f64::MAX, f64::min) < 0.2 * median.abs() && median.abs() > 1e-6;
    let slope = if recovering { 0.0 } else { slope };
    let point: Vec<f64> = fc
        .point
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let lin = last + slope * (i + 1) as f64;
            match dir {
                Dir::High => p.max(lin),
                Dir::Low => p.min(lin),
            }
        })
        .collect();
    let (lower, upper) = match &fc.intervals {
        Some(iv) => {
            let off: Vec<f64> = point.iter().zip(&fc.point).map(|(a, b)| a - b).collect();
            (
                iv.lower.iter().zip(&off).map(|(l, o)| l + o).collect(),
                iv.upper.iter().zip(&off).map(|(u, o)| u + o).collect(),
            )
        }
        None => (point.clone(), point.clone()),
    };
    let cross_step = point.iter().position(|v| match dir {
        Dir::High => *v >= threshold,
        Dir::Low => *v <= threshold,
    });
    Ok(Forecast { cross_step: cross_step.map(|s| s + 1), point, lower, upper })
}

/// Which series disagree with their peers over the recent window (augurs MAD detector).
/// `series` are aligned, same length. Returns indices of outlying series.
pub fn peer_outliers(series: &[Vec<f64>], sensitivity: f64) -> Vec<usize> {
    if series.len() < 4 || series.iter().any(|s| s.len() < 6) {
        return vec![];
    }
    let refs: Vec<&[f64]> = series.iter().map(|s| s.as_slice()).collect();
    let Ok(det) = MADDetector::with_sensitivity(sensitivity) else { return vec![] };
    let Ok(pre) = det.preprocess(&refs) else { return vec![] };
    let Ok(out) = det.detect(&pre) else { return vec![] };
    out.outlying_series.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yesterdays_heat_wave_is_not_today() {
        // Facility water with a 4 degree daily swing, and a +7 degree heat wave at this time yesterday.
        let n = DAY * 3;
        let y: Vec<f64> = (0..n)
            .map(|i| {
                let from_end = n - i;
                let base = 26.0 + 4.0 * (std::f64::consts::TAU * (i as f64) / DAY as f64).sin() + ((i * 31) % 7) as f64 * 0.02;
                if (DAY - 24..DAY + 12).contains(&from_end) { base + 7.0 } else { base }
            })
            .collect();
        let peak_ahead = (1..=24).map(|h| 26.0 + 4.0 * (std::f64::consts::TAU * ((n - 1 + h) as f64) / DAY as f64).sin()).fold(f64::MIN, f64::max);
        let f = forecast(&y, 24, peak_ahead + 2.5, Dir::High).unwrap();
        assert!(f.cross_step.is_none(), "crossed at {:?}: {:?}", f.cross_step, &f.point[..8]);
    }

    #[test]
    fn a_step_is_not_a_trend() {
        // A tap at 30% of rating picks up its partner's load and settles at 60%.
        let n = DAY * 2;
        let y: Vec<f64> = (0..n).map(|i| if i + 6 >= n { 150.0 } else { 75.0 } + ((i * 31) % 7) as f64 * 0.3).collect();
        let f = forecast(&y, 24, 237.5, Dir::High).unwrap();
        assert!(f.cross_step.is_none(), "step forecast crossed at {:?}: {:?}", f.cross_step, &f.point[..8]);
    }

    #[test]
    fn yesterdays_incident_is_not_seasonal() {
        // Two days of flat vibration with a two-hour wear-and-trip excursion one day ago.
        let n = DAY * 3;
        let y: Vec<f64> = (0..n)
            .map(|i| {
                let from_end = n - i;
                let base = 2.0 + 0.05 * ((i * 7919) % 13) as f64 / 13.0;
                if (DAY - 4..DAY + 20).contains(&from_end) { base + 7.0 * ((DAY + 20 - from_end) as f64 / 24.0) } else { base }
            })
            .collect();
        let f = forecast(&y, 24, 4.5, Dir::High).unwrap();
        assert!(f.cross_step.is_none(), "forecast crossed at {:?}: {:?}", f.cross_step, &f.point[..6]);
        // Without cleaning, MSTL folds the excursion into the daily pattern and the forecast
        // collapses toward zero after it, which would read as flow or level "falling below".
        let f = forecast(&y, 24, 1.0, Dir::Low).unwrap();
        assert!(f.cross_step.is_none(), "forecast fell below at {:?}: {:?}", f.cross_step, &f.point[..8]);
    }

    fn day_cycle(n: usize, base: f64, amp: f64) -> Vec<f64> {
        (0..n).map(|i| base + amp * (std::f64::consts::TAU * (i as f64) / DAY as f64).sin()).collect()
    }

    #[test]
    fn predicts_seasonal_crossing() {
        // two days of history with a 5-unit daily swing, threshold just above yesterday's peak
        let mut y = day_cycle(DAY * 2 + 30, 40.0, 5.0);
        // a slow upward drift today
        let n = y.len();
        for (i, v) in y.iter_mut().enumerate().skip(n - 60) {
            *v += (i - (n - 60)) as f64 * 0.05;
        }
        let t = 47.0;
        assert!(worth_fitting(&y, t, Dir::High, 24));
        let t0 = std::time::Instant::now();
        let f = forecast(&y, 36, t, Dir::High).unwrap();
        eprintln!("fit {:?} cross {:?} first {:.2} last {:.2}", t0.elapsed(), f.cross_step, f.point[0], f.point[35]);
        assert!(f.cross_step.is_some());
    }

    #[test]
    fn recovery_ramp_is_not_a_trend() {
        let mut y = day_cycle(DAY * 2, 90.0, 10.0);
        let n = y.len();
        for (i, v) in y.iter_mut().enumerate().skip(n - 12) {
            *v = (i - (n - 12)) as f64 * 8.0; // 0 → 88 over an hour
        }
        let f = forecast(&y, 24, 237.0, Dir::High).unwrap();
        assert!(f.cross_step.is_none(), "{:?}", &f.point[..5]);
    }

    #[test]
    fn steady_point_is_not_worth_fitting() {
        let y = day_cycle(DAY * 2, 30.0, 1.0);
        assert!(!worth_fitting(&y, 47.0, Dir::High, 24));
    }

    #[test]
    fn finds_peer_outlier() {
        let mut s: Vec<Vec<f64>> = (0..8).map(|k| (0..12).map(|i| 35.0 + (i as f64) * 0.05 + k as f64 * 0.1).collect()).collect();
        for v in s[5].iter_mut() {
            *v += 6.0;
        }
        let o = peer_outliers(&s, 0.5);
        assert_eq!(o, vec![5]);
    }
}
