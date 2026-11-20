//! The knobs scenarios turn. Rebuilt from the active runs on every tick.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct Faults {
    // grid and utility
    pub util_status: BTreeMap<String, i64>, // side -> 0 lost, 1 normal, 2 sag
    pub util_voltage_dev: f64,
    pub grid_freq_dev: f64,
    pub dr_event: i64, // 0 none, 1 announced, 2 active
    pub flex_cut_frac: f64,
    pub gen_fail: BTreeSet<String>,
    // power
    pub ups_alarm: BTreeSet<String>,
    pub ups_fault: BTreeSet<String>,
    pub ups_bypass: BTreeSet<String>, // side
    pub xfmr_extra: BTreeMap<String, f64>,
    pub xfmr_trip: BTreeSet<String>,
    pub busway_fault: BTreeSet<String>,
    pub busway_hot: BTreeMap<String, f64>,
    pub tap_trip: BTreeSet<String>,
    pub tap_overload: BTreeMap<String, f64>,
    pub bess_extra_temp: f64,
    pub bess_isolated: bool,
    pub rack_psu_failed: BTreeMap<String, u32>,
    // cooling
    pub wetbulb_extra: f64,
    pub dc_fault: BTreeSet<String>,
    pub fwp_trip: BTreeSet<String>,
    pub fwp_vib: BTreeMap<String, f64>,
    pub ch_alarm: BTreeSet<String>,
    pub ch_trip: BTreeSet<String>,
    pub ch_head: BTreeMap<String, f64>,
    pub crah_fault: BTreeSet<String>,
    pub cdu_wear: BTreeMap<String, f64>,
    pub cdu_vib: BTreeMap<String, f64>,
    pub cdu_leak_rate: BTreeMap<String, f64>,
    pub cdu_leak_detect: BTreeSet<String>,
    pub cdu_off: BTreeSet<String>,
    pub rack_seep: BTreeMap<String, f64>,
    pub rack_leak: BTreeSet<String>,
    pub rack_isolated: BTreeSet<String>,
    // network and compute
    pub switch_down: BTreeSet<String>,
    pub link_errors: BTreeMap<String, f64>,
    pub nvl_failed: BTreeMap<String, u32>,
    pub nvl_errors: BTreeMap<String, f64>,
    pub tray_health: BTreeMap<String, i64>,
    // data
    pub sensor_offset: BTreeMap<String, f64>,
    pub sensor_stuck: BTreeSet<String>,
    pub comms_lost: BTreeSet<String>,
}
