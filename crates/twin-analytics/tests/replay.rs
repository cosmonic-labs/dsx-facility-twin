//! Replays real twin history captured from the lab during scenario QA.
use twin_analytics::{forecast, Dir, DAY};

fn load(name: &str) -> Vec<f64> {
    let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(p).unwrap().lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// Facility water supply: the facility-pump-trip scenario drove it to 46 °C yesterday. Today,
/// at the same time of day, the twin forecast the whole hall degraded within 80 minutes.
#[test]
fn facility_water_does_not_replay_yesterdays_pump_trip() {
    let y = load("fw_supply_day_after_pump_trip.csv");
    let peak = y.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
    // cut the history so yesterday's peak falls 0..2 h ahead of "now" (same time of day)
    for ahead in [2usize, 8, 16, 24] {
        let end = peak + DAY - ahead;
        let yy = &y[..end.min(y.len())];
        let f = forecast(yy, 24, 32.0, Dir::High).unwrap();
        assert!(f.cross_step.is_none(), "ahead {ahead}: crossed at {:?}: {:?}", f.cross_step, f.point.iter().map(|v| v.round()).collect::<Vec<_>>());
    }
}
