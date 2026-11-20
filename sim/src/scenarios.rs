//! Failure scenarios. Each one is a short story told in stages (warn → fail → escalate →
//! restore) that turns the physics knobs in `Faults`. Stages are failure-specific: a pump
//! wears, a leak seeps before the detector trips, a grid event is announced before it caps.
use crate::faults::Faults;
use serde::{Deserialize, Serialize};
use twin_model::topo::{AssetKind, Topology};

#[derive(Debug, Clone, Serialize)]
pub struct StageDef {
    pub key: &'static str,
    pub name: &'static str,
    /// warn | fail | escalate | mitigate | restore
    pub kind: &'static str,
    /// Simulated minutes; ramps reach 100% at the end of the stage.
    pub minutes: f64,
    pub detail: &'static str,
    /// What the twin should show (for the presenter and for QA).
    pub expect: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioDef {
    pub id: &'static str,
    pub name: &'static str,
    pub domain: &'static str,
    pub summary: &'static str,
    pub target_kinds: Vec<AssetKind>,
    pub default_target: &'static str,
    pub stages: Vec<StageDef>,
}

const fn st(key: &'static str, name: &'static str, kind: &'static str, minutes: f64, detail: &'static str, expect: &'static str) -> StageDef {
    StageDef { key, name, kind, minutes, detail, expect }
}
const RESTORE: StageDef = st("restore", "Restore", "restore", 20.0, "Repair complete; the plant recovers on its own time constants.", "States clear back to green as readings recover.");

pub fn catalog() -> Vec<ScenarioDef> {
    use AssetKind::*;
    vec![
        // ---------------------------------------------------------------- cooling
        ScenarioDef { id: "cdu-pump-wear", name: "CDU pump bearing wear", domain: "cooling",
            summary: "A CDU pump bearing wears out: vibration climbs, flow falls, then the pump trips and its partner carries the row alone.",
            target_kinds: vec![Cdu], default_target: "CDU-P1-R2-A",
            stages: vec![
                st("bearing-wear", "Bearing wear", "warn", 45.0, "Vibration rises from 2 to 5 mm/s; maximum flow drops 15%.", "CDU yellow (vibration watch). augurs forecasts the trend."),
                st("flow-loss", "Flow loss", "escalate", 30.0, "Wear accelerates; the CDU delivers less than half its rated flow. The partner CDU ramps to its limit.", "CDU orange. Row racks yellow: on trend for the afternoon peak."),
                st("pump-trip", "Pump trip", "fail", 45.0, "The CDU trips offline. One CDU carries the whole row.", "CDU red. Racks lose cooling redundancy; at peak load they warm into degraded and throttle."),
                RESTORE ] },
        ScenarioDef { id: "rack-qd-leak", name: "Rack quick-disconnect leak", domain: "cooling",
            summary: "A rack manifold quick-disconnect seeps, then lets go. Leak detection isolates the rack and it shuts down.",
            target_kinds: vec![Rack], default_target: "P1-R1-03",
            stages: vec![
                st("seep", "Seep at quick-disconnect", "warn", 30.0, "Rack differential pressure decays; the row CDU reservoir starts dropping (make-up water).", "Reservoir forecast and rack dP point to a leak before the detector trips."),
                st("leak-detected", "Leak detected, rack isolated", "fail", 40.0, "Leak rope wet. Leak response closes the rack valves: no liquid flow, GPUs overheat and the rack shuts down.", "Rack red, its 18 trays red, customer red."),
                RESTORE ] },
        ScenarioDef { id: "cdu-leak", name: "CDU secondary loop leak", domain: "cooling",
            summary: "A CDU secondary loop leaks: the reservoir falls until the CDU isolates itself.",
            target_kinds: vec![Cdu], default_target: "CDU-P2-R1-B",
            stages: vec![
                st("reservoir-dropping", "Reservoir dropping", "warn", 30.0, "Reservoir level falls 0.6% per minute.", "augurs predicts the level crossing its degraded limit; CDU yellow."),
                st("cdu-isolated", "Leak detected, CDU isolated", "fail", 45.0, "Leak sensor trips and the CDU isolates. Its partner carries the row.", "CDU red; row racks redundancy lost, degraded at peak."),
                RESTORE ] },
        ScenarioDef { id: "chiller-trip", name: "Trim chiller trip", domain: "cooling",
            summary: "The lead trim chiller's head pressure climbs and it trips; then the standby trips on overload. Air-side cooling is lost.",
            target_kinds: vec![Chiller], default_target: "CH-1",
            stages: vec![
                st("high-head", "High head pressure", "warn", 30.0, "Condenser head pressure climbs 250 kPa; alarm at the end.", "Chiller yellow."),
                st("chiller-trips", "Chiller trips", "fail", 30.0, "The lead chiller trips; the standby starts.", "Chiller red; CRAHs on a single chiller (redundancy lost)."),
                st("standby-trips", "Standby chiller trips", "escalate", 40.0, "The standby trips too. Chilled water warms; supply air and rack inlet temperatures climb.", "CRAHs out; racks ride through on room air, then inlet air goes degraded."),
                RESTORE ] },
        ScenarioDef { id: "heat-wave", name: "Heat wave and dry cooler failure", domain: "cooling",
            summary: "Wet-bulb rises 7 °C over an afternoon; then a dry-cooler fan fails and the plant runs short of heat rejection.",
            target_kinds: vec![CoolingTower], default_target: "DC-2",
            stages: vec![
                st("heat-building", "Heat building", "warn", 60.0, "Ambient wet-bulb rises 7 °C. Dry-cooler capacity derates; facility water warms.", "augurs forecasts facility water crossing its limit: site-wide yellow."),
                st("fan-fault", "Dry cooler fan fault", "fail", 45.0, "A dry cooler drops out during the heat. Two units carry the hall.", "Dry cooler red; facility water warm; CDUs and racks drift toward degraded."),
                st("second-fan-fault", "Second dry cooler fault", "escalate", 30.0, "A second dry cooler fails. Heat rejection is below load.", "Facility water degraded; racks across the hall throttle (orange)."),
                RESTORE ] },
        ScenarioDef { id: "fw-pump-trip", name: "Facility water pump trip", domain: "cooling",
            summary: "The lead facility pump trips and the standby takes over; then the standby's drive faults and facility water stops.",
            target_kinds: vec![FacilityPump], default_target: "FWP-1",
            stages: vec![
                st("vibration", "Pump vibration rising", "warn", 30.0, "Lead pump vibration rises to 6 mm/s.", "Pump yellow; augurs trend."),
                st("lead-trips", "Lead pump trips", "fail", 30.0, "Standby pump takes over.", "Pump red; facility water on one pump (redundancy lost)."),
                st("standby-fault", "Standby drive fault", "escalate", 20.0, "No facility water flow. Every CDU loses heat rejection.", "Facility water red: the widest blast radius in the hall."),
                RESTORE ] },
        ScenarioDef { id: "crah-failure", name: "CRAH failures", domain: "cooling",
            summary: "Two CRAHs fail; air-side capacity drops below the residual heat load at peak.",
            target_kinds: vec![Crah], default_target: "CRAH-2",
            stages: vec![
                st("crah-down", "CRAH fan failure", "fail", 30.0, "One CRAH stops. Three of four carry the room (N+1 used up).", "CRAH red; racks: air cooling redundancy lost."),
                st("second-crah", "Second CRAH failure", "escalate", 45.0, "A second CRAH stops. Supply air warms.", "Racks ride through on room air; inlet air climbs toward watch."),
                RESTORE ] },
        // ---------------------------------------------------------------- power
        ScenarioDef { id: "utility-outage", name: "Utility outage (generators work)", domain: "power",
            summary: "Utility sags, then drops. UPS batteries carry the hall for seconds while generators start; ATSs transfer.",
            target_kinds: vec![UtilityFeed, Grid], default_target: "UTIL-A",
            stages: vec![
                st("sag", "Utility sag", "warn", 5.0, "Voltage dips 9% and frequency wanders.", "Utility yellow."),
                st("utility-lost", "Utility lost", "fail", 10.0, "Both feeds lost. UPS on battery; generators start and ATSs transfer.", "Utility red. Brief on-battery yellow, then green on generator."),
                st("on-generators", "Running on generators", "mitigate", 60.0, "The hall runs on generator power; fuel burns.", "Utility red; everything downstream green on the generator path (redundancy lost)."),
                st("restore", "Utility restored", "restore", 15.0, "Utility returns; ATSs retransfer and generators cool down.", "All green.") ] },
        ScenarioDef { id: "gen-fail-to-start", name: "Utility outage, generator fails to start", domain: "power",
            summary: "Utility drops and one generator fails to start. That side's UPS batteries count down to zero.",
            target_kinds: vec![Generator], default_target: "GEN-A",
            stages: vec![
                st("utility-lost", "Utility lost, generator fails", "fail", 20.0, "Both feeds lost; this generator fails to start. Its side runs on UPS battery.", "UPS modules yellow with a runtime ETA; racks yellow: on trend to lose that side."),
                st("battery-depleted", "Batteries depleted", "escalate", 30.0, "The side's UPS output turns off. Racks run on their other cord.", "UPS and side A distribution red; racks green but redundancy lost."),
                st("restore", "Utility restored", "restore", 15.0, "Utility returns; batteries recharge.", "All green.") ] },
        ScenarioDef { id: "ups-module-failure", name: "UPS module failure", domain: "power",
            summary: "A UPS module alarms and fails; a second fails and the side drops to static bypass.",
            target_kinds: vec![Ups], default_target: "UPS-B2",
            stages: vec![
                st("module-alarm", "Module alarm", "warn", 15.0, "Capacitor / fan alarm on the module.", "UPS yellow."),
                st("module-failed", "Module failed", "fail", 30.0, "The module drops out; the side runs N (no spare module).", "UPS red; switchboard redundancy lost."),
                st("static-bypass", "Second module fails: static bypass", "escalate", 30.0, "The side transfers to static bypass: unconditioned utility power.", "UPS side orange; racks stay up on the other side (redundancy lost)."),
                RESTORE ] },
        ScenarioDef { id: "tap-overload-trip", name: "Busway tap overload and trip", domain: "power",
            summary: "A tenant's load climbs on one tap-off until its breaker trips; the rack rides on its other cord.",
            target_kinds: vec![TapOff], default_target: "P2-R2-02-TAP-A",
            stages: vec![
                st("overload", "Tap overload", "warn", 30.0, "Current climbs past 80% of the breaker rating.", "Tap yellow, then orange; augurs forecasts the trip."),
                st("breaker-trips", "Breaker trips", "fail", 30.0, "The tap breaker opens.", "Tap red; rack redundancy lost."),
                st("restore", "Breaker reset", "restore", 10.0, "Electrician resets the breaker.", "Green.") ] },
        ScenarioDef { id: "busway-fault", name: "Busway fault", domain: "power",
            summary: "A row busway de-energizes: every rack in the row loses one cord.",
            target_kinds: vec![Busway], default_target: "BW-P1-R1-B",
            stages: vec![
                st("de-energized", "Busway de-energized", "fail", 45.0, "The busway trips upstream.", "Busway and its 8 taps red; 8 racks redundancy lost."),
                RESTORE ] },
        ScenarioDef { id: "transformer-overheat", name: "Transformer overheat and trip", domain: "power",
            summary: "A transformer's winding temperature climbs until it trips; that side's UPS runs on battery.",
            target_kinds: vec![Transformer], default_target: "XFMR-B",
            stages: vec![
                st("winding-hot", "Winding temperature rising", "warn", 45.0, "Cooling fan failure: winding temperature climbs 70 °C.", "augurs forecasts the degraded limit; transformer yellow then orange."),
                st("trips", "Transformer trips", "fail", 40.0, "Protection opens. The side's UPS modules run on battery until empty.", "UPS yellow with runtime ETA, then side B red; racks redundancy lost."),
                RESTORE ] },
        ScenarioDef { id: "grid-flex-event", name: "DSX Flex grid event", domain: "grid",
            summary: "The grid operator calls a demand response event. BESS discharges and spot capacity is capped first.",
            target_kinds: vec![Grid], default_target: "GRID",
            stages: vec![
                st("announced", "Event announced", "warn", 15.0, "DSX Flex receives a load target from the grid operator.", "Grid yellow."),
                st("cap-active", "Power cap active", "fail", 45.0, "Site sheds 18% of IT load: BESS covers part, spot racks capped first.", "Spot racks orange (capped); premium untouched."),
                st("extended", "Event extended", "escalate", 30.0, "The operator deepens the cut to 30%. Standard-tier racks are capped too.", "More customers orange; revenue impact climbs."),
                st("restore", "Event ends", "restore", 15.0, "Caps lift; BESS recharges.", "Green.") ] },
        ScenarioDef { id: "bess-thermal", name: "BESS thermal event", domain: "power",
            summary: "Battery cell temperature climbs; the BESS isolates and Flex capacity is lost.",
            target_kinds: vec![Bess], default_target: "BESS-1",
            stages: vec![
                st("cell-temp", "Cell temperature rising", "warn", 30.0, "Cell temperature climbs 22 °C.", "BESS yellow, augurs forecast."),
                st("isolated", "BESS isolated", "fail", 30.0, "Battery management isolates the system.", "BESS red; no Flex reserve."),
                RESTORE ] },
        ScenarioDef { id: "power-shelf-failure", name: "Rack power shelf failures", domain: "power",
            summary: "Power shelves fail inside a rack; with two gone the rack caps its power.",
            target_kinds: vec![Rack], default_target: "P1-R2-02",
            stages: vec![
                st("psu-fault", "Power shelf fault (1 of 6)", "warn", 30.0, "One shelf fails; N+N redundancy reduced.", "Rack yellow."),
                st("second-psu", "Second power shelf fault", "fail", 30.0, "Two shelves gone: the rack caps to 75%.", "Rack orange (capped)."),
                RESTORE ] },
        // ---------------------------------------------------------------- network and compute
        ScenarioDef { id: "leaf-switch-failure", name: "Leaf switch failure", domain: "network",
            summary: "A leaf switch accumulates CRC errors and fails; then its partner fails and the row loses fabric.",
            target_kinds: vec![LeafSwitch], default_target: "LEAF-P2-R2-A",
            stages: vec![
                st("crc-errors", "CRC errors rising", "warn", 20.0, "Link errors climb on the leaf.", "Leaf yellow."),
                st("leaf-down", "Leaf down", "fail", 30.0, "The leaf drops; trays fail over to the partner.", "Leaf red; 144 trays redundancy lost."),
                st("partner-down", "Partner leaf down", "escalate", 20.0, "Both leafs in the row are down.", "Row trays red: the customer's jobs lose the fabric."),
                RESTORE ] },
        ScenarioDef { id: "nvlink-tray-failure", name: "NVSwitch tray failure", domain: "compute",
            summary: "An NVSwitch tray inside a rack fails; the NVL72 domain runs with reduced bandwidth.",
            target_kinds: vec![Rack], default_target: "P1-R1-05",
            stages: vec![
                st("nvlink-errors", "NVLink errors", "warn", 20.0, "Error counters climb.", "NVLink fabric yellow."),
                st("tray-failed", "NVSwitch tray failed", "fail", 40.0, "One of nine switch trays is down.", "Fabric orange; all 18 compute trays degraded."),
                RESTORE ] },
        ScenarioDef { id: "gpu-xid", name: "GPU XID and tray offline", domain: "compute",
            summary: "A compute tray reports ECC and thermal warnings, then falls off the bus (XID 79).",
            target_kinds: vec![ComputeTray], default_target: "P1-R1-02-T07",
            stages: vec![
                st("gpu-warning", "ECC / thermal warning", "warn", 20.0, "GPU health warnings.", "Tray yellow."),
                st("xid", "Tray offline (XID 79)", "fail", 40.0, "The tray's GPUs drop off the bus.", "Tray red; 4 GPUs of one customer out."),
                RESTORE ] },
        // ---------------------------------------------------------------- data
        ScenarioDef { id: "sensor-stuck", name: "Drifting, then stuck sensor", domain: "data",
            summary: "A rack return-temperature sensor drifts high, then freezes. The equipment is fine: the twin should say so.",
            target_kinds: vec![Rack], default_target: "P2-R1-04",
            stages: vec![
                st("drift", "Sensor drifting", "warn", 40.0, "The return-temperature sensor reads up to 6 °C high.", "augurs peer check: unusual versus its row. Not an equipment fault."),
                st("stuck", "Sensor stuck", "fail", 30.0, "The sensor freezes at its last value.", "Flat line, peers keep moving."),
                st("restore", "Sensor replaced", "restore", 10.0, "Technician replaces the sensor.", "Green.") ] },
        ScenarioDef { id: "bms-comms-loss", name: "BMS gateway comms loss", domain: "data",
            summary: "A row's BMS gateway starts dropping, then goes dark. The twin marks the row as unknown instead of guessing.",
            target_kinds: vec![Row], default_target: "P2-R2",
            stages: vec![
                st("retries", "Gateway retries", "warn", 15.0, "The CDUs in the row report bad quality.", "CDUs grey (no data)."),
                st("comms-lost", "Comms lost", "fail", 30.0, "Every point in the row reports bad quality.", "Row grey: no data, not a failure."),
                RESTORE ] },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: u64,
    pub scenario: String,
    pub target: String,
    pub target_name: String,
    pub stage: usize,
    pub stage_started_ms: i64,
    pub started_ms: i64,
    pub auto: bool,
    pub done: bool,
    #[serde(default)]
    pub ended_ms: Option<i64>,
}

impl Run {
    pub fn progress(&self, def: &ScenarioDef, now_ms: i64) -> f64 {
        let m = def.stages[self.stage].minutes.max(0.1);
        (((now_ms - self.stage_started_ms) as f64 / 60_000.0) / m).clamp(0.0, 1.0)
    }
}

fn row_of(id: &str) -> String {
    // P1-R2-03 -> P1-R2 ; CDU-P1-R2-A -> P1-R2
    let s = id.trim_start_matches("CDU-").trim_start_matches("LEAF-").trim_start_matches("BW-");
    let parts: Vec<&str> = s.split('-').collect();
    parts.iter().take(2).cloned().collect::<Vec<_>>().join("-")
}

fn partner(id: &str) -> String {
    if let Some(x) = id.strip_suffix("-A") {
        format!("{x}-B")
    } else if let Some(x) = id.strip_suffix("-B") {
        format!("{x}-A")
    } else {
        id.to_string()
    }
}

/// Apply one run's current stage to the fault set.
pub fn apply(run: &Run, def: &ScenarioDef, now_ms: i64, topo: &Topology, f: &mut Faults) {
    if run.done {
        return;
    }
    let key = def.stages[run.stage].key;
    if key == "restore" {
        return;
    }
    let p = run.progress(def, now_ms);
    let t = run.target.clone();
    let side_of = |x: &str| x.chars().last().map(|c| c.to_string()).unwrap_or("A".into());
    let both = |f: &mut Faults, v: i64| {
        f.util_status.insert("A".into(), v);
        f.util_status.insert("B".into(), v);
    };
    match (def.id, key) {
        ("cdu-pump-wear", "bearing-wear") => {
            f.cdu_vib.insert(t.clone(), 1.6 * p);
            f.cdu_wear.insert(t, 0.15 * p);
        }
        ("cdu-pump-wear", "flow-loss") => {
            f.cdu_vib.insert(t.clone(), 1.6 + 2.0 * p);
            f.cdu_wear.insert(t, 0.15 + 0.45 * p);
        }
        ("cdu-pump-wear", "pump-trip") => {
            f.cdu_off.insert(t);
        }
        ("rack-qd-leak", "seep") => {
            f.rack_seep.insert(t.clone(), p);
            f.cdu_leak_rate.insert(format!("CDU-{}-A", row_of(&t)), 0.5);
        }
        ("rack-qd-leak", "leak-detected") => {
            f.rack_seep.insert(t.clone(), 1.0);
            f.rack_leak.insert(t.clone());
            f.rack_isolated.insert(t.clone());
            f.cdu_leak_rate.insert(format!("CDU-{}-A", row_of(&t)), 0.3);
        }
        ("cdu-leak", "reservoir-dropping") => {
            f.cdu_leak_rate.insert(t, 0.6);
        }
        ("cdu-leak", "cdu-isolated") => {
            f.cdu_leak_rate.insert(t.clone(), 0.6);
            f.cdu_leak_detect.insert(t.clone());
            f.cdu_off.insert(t);
        }
        ("chiller-trip", "high-head") => {
            f.ch_head.insert(t.clone(), 250.0 * p);
            if p > 0.6 {
                f.ch_alarm.insert(t);
            }
        }
        ("chiller-trip", "chiller-trips") => {
            f.ch_trip.insert(t);
        }
        ("chiller-trip", "standby-trips") => {
            for a in topo.assets.iter().filter(|a| a.kind == AssetKind::Chiller) {
                f.ch_trip.insert(a.id.clone());
            }
        }
        ("heat-wave", "heat-building") => f.wetbulb_extra = 7.0 * p,
        ("heat-wave", "fan-fault") => {
            f.wetbulb_extra = 7.0;
            f.dc_fault.insert(t);
        }
        ("heat-wave", "second-fan-fault") => {
            f.wetbulb_extra = 7.0;
            f.dc_fault.insert(t.clone());
            let other = topo.assets.iter().filter(|a| a.kind == AssetKind::CoolingTower && a.id != t).map(|a| a.id.clone()).last();
            if let Some(o) = other {
                f.dc_fault.insert(o);
            }
        }
        ("fw-pump-trip", "vibration") => {
            f.fwp_vib.insert(t, 4.0 * p);
        }
        ("fw-pump-trip", "lead-trips") => {
            f.fwp_trip.insert(t);
        }
        ("fw-pump-trip", "standby-fault") => {
            for a in topo.assets.iter().filter(|a| a.kind == AssetKind::FacilityPump) {
                f.fwp_trip.insert(a.id.clone());
            }
        }
        ("crah-failure", "crah-down") => {
            f.crah_fault.insert(t);
        }
        ("crah-failure", "second-crah") => {
            f.crah_fault.insert(t.clone());
            let other = topo.assets.iter().filter(|a| a.kind == AssetKind::Crah && a.id != t).map(|a| a.id.clone()).next();
            if let Some(o) = other {
                f.crah_fault.insert(o);
            }
        }
        ("utility-outage", "sag") => {
            both(f, 2);
            f.grid_freq_dev = 0.15;
        }
        ("utility-outage", "utility-lost") | ("utility-outage", "on-generators") => {
            both(f, 0);
            f.grid_freq_dev = 0.0;
        }
        ("gen-fail-to-start", _) => {
            both(f, 0);
            f.gen_fail.insert(side_of(&t));
        }
        ("ups-module-failure", "module-alarm") => {
            f.ups_alarm.insert(t);
        }
        ("ups-module-failure", "module-failed") => {
            f.ups_fault.insert(t);
        }
        ("ups-module-failure", "static-bypass") => {
            let side = t.chars().nth(4).map(|c| c.to_string()).unwrap_or("A".into());
            f.ups_fault.insert(t.clone());
            f.ups_bypass.insert(side);
        }
        ("tap-overload-trip", "overload") => {
            // fraction of the breaker rating: passes watch (80%) mid-stage, degraded (95%) near the end
            f.tap_overload.insert(t, 0.62 + 0.38 * p);
        }
        ("tap-overload-trip", "breaker-trips") => {
            f.tap_trip.insert(t);
        }
        ("busway-fault", "de-energized") => {
            f.busway_fault.insert(t);
        }
        ("transformer-overheat", "winding-hot") => {
            f.xfmr_extra.insert(side_of(&t), 85.0 * p);
        }
        ("transformer-overheat", "trips") => {
            f.xfmr_extra.insert(side_of(&t), 85.0);
            f.xfmr_trip.insert(side_of(&t));
        }
        ("grid-flex-event", "announced") => f.dr_event = 1,
        ("grid-flex-event", "cap-active") => {
            f.dr_event = 2;
            f.flex_cut_frac = 0.18;
        }
        ("grid-flex-event", "extended") => {
            f.dr_event = 2;
            f.flex_cut_frac = 0.30;
        }
        ("bess-thermal", "cell-temp") => f.bess_extra_temp = 27.0 * p,
        ("bess-thermal", "isolated") => {
            f.bess_extra_temp = 25.0;
            f.bess_isolated = true;
        }
        ("power-shelf-failure", "psu-fault") => {
            f.rack_psu_failed.insert(t, 1);
        }
        ("power-shelf-failure", "second-psu") => {
            f.rack_psu_failed.insert(t, 2);
        }
        ("leaf-switch-failure", "crc-errors") => {
            f.link_errors.insert(t, 320.0 * p);
        }
        ("leaf-switch-failure", "leaf-down") => {
            f.switch_down.insert(t);
        }
        ("leaf-switch-failure", "partner-down") => {
            f.switch_down.insert(partner(&t));
            f.switch_down.insert(t);
        }
        ("nvlink-tray-failure", "nvlink-errors") => {
            f.nvl_errors.insert(format!("{t}-NVL"), 60.0 * p);
        }
        ("nvlink-tray-failure", "tray-failed") => {
            f.nvl_errors.insert(format!("{t}-NVL"), 80.0);
            f.nvl_failed.insert(format!("{t}-NVL"), 1);
        }
        ("gpu-xid", "gpu-warning") => {
            f.tray_health.insert(t, 1);
        }
        ("gpu-xid", "xid") => {
            f.tray_health.insert(t, 3);
        }
        ("sensor-stuck", "drift") => {
            f.sensor_offset.insert(format!("{t}.liquid_return_temp"), 6.0 * p);
        }
        ("sensor-stuck", "stuck") => {
            f.sensor_offset.insert(format!("{t}.liquid_return_temp"), 6.0);
            f.sensor_stuck.insert(format!("{t}.liquid_return_temp"));
        }
        ("bms-comms-loss", "retries") => {
            for a in topo.assets.iter().filter(|a| a.kind == AssetKind::Cdu && a.parent.as_deref() == Some(t.as_str())) {
                f.comms_lost.insert(a.id.clone());
            }
        }
        ("bms-comms-loss", "comms-lost") => {
            for a in &topo.assets {
                let in_row = a.parent.as_deref() == Some(t.as_str())
                    || a.parent.as_deref().map(|p| p.starts_with(&format!("{t}-"))).unwrap_or(false);
                if in_row && a.kind != AssetKind::ComputeTray && a.kind != AssetKind::LeafSwitch {
                    f.comms_lost.insert(a.id.clone());
                }
            }
        }
        _ => {}
    }
}
