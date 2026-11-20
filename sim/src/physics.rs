//! A small, coupled physical model of the hall: workload → power → heat → cooling → limits.
//!
//! It is not CFD and not a power-flow solver. It is just enough physics that failures
//! propagate the way they do in a real AI factory: a weak CDU pump warms the row in the
//! afternoon peak, a heat wave eats dry-cooler capacity, a UPS on battery counts down.
use crate::faults::Faults;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::{BTreeMap, BTreeSet};
use twin_model::topo::{AssetKind, Tier, Topology};

const V_LL: f64 = 415.0;
const PF: f64 = 0.95;

fn amps(kw: f64) -> f64 {
    kw * 1000.0 / (3f64.sqrt() * V_LL * PF)
}
fn lag(cur: f64, target: f64, dt_s: f64, tau_min: f64) -> f64 {
    let a = 1.0 - (-dt_s / (tau_min * 60.0)).exp();
    cur + (target - cur) * a
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gen {
    Standby,
    Starting(u32),
    Running,
    FailToStart,
}

#[derive(Clone, Debug)]
struct RackStatic {
    id: String,
    row: String,
    taps: [String; 2],
    rated_kw: f64,
    design_flow: f64,
    design_dp: f64,
    psus: f64,
    tier: Option<Tier>,
}

#[derive(Clone, Debug, Default)]
struct RackDyn {
    noise: f64,
    ret: f64,
    throttle: f64,
    shutdown: bool,
    p_kw: f64,
    /// Power before any cap: what the rack would draw.
    p_raw: f64,
    flow: f64,
    supply: f64,
    cap: f64,
}

/// Structure derived once from the topology.
#[derive(Clone)]
struct Layout {
    racks: Vec<RackStatic>,
    rows: Vec<String>,
    cdus_by_row: BTreeMap<String, Vec<String>>,
    busways: BTreeMap<String, (String, String)>, // id -> (row, side)
    leafs: Vec<String>,
    spines: Vec<String>,
    ups: BTreeMap<String, Vec<String>>, // side -> modules
    ups_need: usize,
    attrs: BTreeMap<String, BTreeMap<String, f64>>,
    dcs: Vec<String>,
    fwps: Vec<String>,
    chillers: Vec<String>,
    crahs: Vec<String>,
    trays: Vec<(String, String)>, // tray, rack
    sides: Vec<String>,
}

#[derive(Clone)]
pub struct World {
    pub t_ms: i64,
    rng: SmallRng,
    l: Layout,
    racks: Vec<RackDyn>,
    pub gens: BTreeMap<String, (Gen, f64)>,
    soc: BTreeMap<String, f64>,
    winding: BTreeMap<String, f64>,
    fws_t: f64,
    chw_t: f64,
    air_t: f64,
    cdu_ts: BTreeMap<String, f64>,
    reservoir: BTreeMap<String, f64>,
    bess_soc: f64,
    bess_temp: f64,
    fuel: BTreeMap<String, f64>,
    stuck: BTreeMap<String, f64>,
    last_it_kw: f64,
    /// point id → (value, quality)
    pub readings: BTreeMap<String, (f64, i64)>,
}

impl World {
    pub fn new(topo: &Topology, t_ms: i64, seed: u64) -> Self {
        let mut tier_of: BTreeMap<String, Tier> = BTreeMap::new();
        for c in &topo.customers {
            for a in &c.allocations {
                let rack = topo
                    .asset(&a.asset)
                    .and_then(|x| if x.kind == AssetKind::ComputeTray { x.parent.clone() } else { Some(x.id.clone()) })
                    .unwrap_or_default();
                let e = tier_of.entry(rack).or_insert(c.tier);
                if c.tier < *e {
                    *e = c.tier;
                }
            }
        }
        let mut l = Layout {
            racks: vec![],
            rows: vec![],
            cdus_by_row: BTreeMap::new(),
            busways: BTreeMap::new(),
            leafs: vec![],
            spines: vec![],
            ups: BTreeMap::new(),
            ups_need: 2,
            attrs: BTreeMap::new(),
            dcs: vec![],
            fwps: vec![],
            chillers: vec![],
            crahs: vec![],
            trays: vec![],
            sides: vec!["A".into(), "B".into()],
        };
        for a in &topo.assets {
            l.attrs.insert(a.id.clone(), a.attrs.clone());
            let side = a.tags.iter().find_map(|t| t.strip_prefix("side:")).unwrap_or("A").to_string();
            match a.kind {
                AssetKind::Row => l.rows.push(a.id.clone()),
                AssetKind::Cdu => l.cdus_by_row.entry(a.parent.clone().unwrap_or_default()).or_default().push(a.id.clone()),
                AssetKind::Busway => {
                    l.busways.insert(a.id.clone(), (a.parent.clone().unwrap_or_default(), side));
                }
                AssetKind::LeafSwitch => l.leafs.push(a.id.clone()),
                AssetKind::SpineSwitch => l.spines.push(a.id.clone()),
                AssetKind::Ups => l.ups.entry(side).or_default().push(a.id.clone()),
                AssetKind::Switchboard => {
                    if let Some(r) = a.requires.first() {
                        l.ups_need = r.need as usize;
                    }
                }
                AssetKind::CoolingTower => l.dcs.push(a.id.clone()),
                AssetKind::FacilityPump => l.fwps.push(a.id.clone()),
                AssetKind::Chiller => l.chillers.push(a.id.clone()),
                AssetKind::Crah => l.crahs.push(a.id.clone()),
                AssetKind::ComputeTray => l.trays.push((a.id.clone(), a.parent.clone().unwrap_or_default())),
                AssetKind::Rack => {
                    let taps = a.requires.iter().find(|r| r.label.starts_with("power")).map(|r| r.of.clone()).unwrap_or_default();
                    l.racks.push(RackStatic {
                        id: a.id.clone(),
                        row: a.parent.clone().unwrap_or_default(),
                        taps: [taps.first().cloned().unwrap_or_default(), taps.get(1).cloned().unwrap_or_default()],
                        rated_kw: a.attr("rated_kw").unwrap_or(132.0),
                        design_flow: a.attr("design_flow_lpm").unwrap_or(190.0),
                        design_dp: a.attr("design_dp_kpa").unwrap_or(140.0),
                        psus: a.attr("psu_count").unwrap_or(6.0),
                        tier: tier_of.get(&a.id).copied(),
                    });
                }
                _ => {}
            }
        }
        let mut w = World {
            t_ms,
            rng: SmallRng::seed_from_u64(seed),
            racks: vec![RackDyn { throttle: 1.0, cap: 1.0, ret: 34.0, supply: 28.0, ..Default::default() }; l.racks.len()],
            gens: l.sides.iter().map(|s| (s.clone(), (Gen::Standby, 0.0))).collect(),
            soc: l.ups.values().flatten().map(|m| (m.clone(), 100.0)).collect(),
            winding: l.sides.iter().map(|s| (s.clone(), 80.0)).collect(),
            fws_t: 25.0,
            chw_t: 12.0,
            air_t: 19.0,
            cdu_ts: l.cdus_by_row.values().flatten().map(|c| (c.clone(), 28.0)).collect(),
            reservoir: l.cdus_by_row.values().flatten().map(|c| (c.clone(), 94.0)).collect(),
            bess_soc: 88.0,
            bess_temp: 26.0,
            fuel: l.sides.iter().map(|s| (s.clone(), 96.0)).collect(),
            stuck: BTreeMap::new(),
            last_it_kw: 3000.0,
            readings: BTreeMap::new(),
            l,
        };
        // settle lags
        for _ in 0..30 {
            w.step(120.0, &Faults::default(), false);
        }
        w
    }

    fn attr(&self, id: &str, k: &str, d: f64) -> f64 {
        self.l.attrs.get(id).and_then(|m| m.get(k)).copied().unwrap_or(d)
    }

    fn hour(&self) -> f64 {
        // Local time for the simulated site (UTC-5).
        ((self.t_ms as f64 / 3.6e6) - 5.0).rem_euclid(24.0)
    }

    fn noise(&mut self, s: f64) -> f64 {
        (self.rng.random::<f64>() - 0.5) * 2.0 * s
    }

    /// Advance by `dt_s` simulated seconds (internally sub-stepped).
    pub fn advance(&mut self, dt_s: f64, f: &Faults) {
        let n = (dt_s / 60.0).ceil().max(1.0) as usize;
        for i in 0..n {
            self.step(dt_s / n as f64, f, i + 1 == n);
        }
    }

    fn put(&mut self, asset: &str, class: &str, v: f64) {
        self.readings.insert(format!("{asset}.{class}"), (v, 1));
    }

    fn step(&mut self, dt: f64, f: &Faults, record: bool) {
        self.t_ms += (dt * 1000.0) as i64;
        let hour = self.hour();
        let day = (std::f64::consts::TAU * (hour - 9.0) / 24.0).sin(); // peaks ~15:00
        let wb = 17.0 + 4.0 * (std::f64::consts::TAU * (hour - 10.0) / 24.0).sin() + f.wetbulb_extra;
        let sides = self.l.sides.clone();

        // ---------------- utility, generators, transfer switches ----------------
        let mut source_ok: BTreeMap<String, bool> = BTreeMap::new();
        let mut ats_pos: BTreeMap<String, i64> = BTreeMap::new();
        let mut util_code: BTreeMap<String, i64> = BTreeMap::new();
        for s in &sides {
            let u = f.util_status.get(s).copied().unwrap_or(1);
            util_code.insert(s.clone(), u);
            let lost = u == 0;
            let (mut g, mut timer) = self.gens[s];
            timer += dt;
            g = match (g, lost) {
                (Gen::Standby, true) => {
                    timer = 0.0;
                    Gen::Starting(0)
                }
                (Gen::Starting(_), true) if f.gen_fail.contains(s) && timer > 10.0 => Gen::FailToStart,
                (Gen::Starting(_), true) if !f.gen_fail.contains(s) && timer > 12.0 => Gen::Running,
                (Gen::FailToStart, true) => Gen::FailToStart,
                (Gen::Running, false) if timer > 180.0 => Gen::Standby,
                (Gen::Running, false) => Gen::Running,
                (Gen::Running, true) => {
                    timer = 0.0;
                    Gen::Running
                }
                (Gen::FailToStart | Gen::Starting(_), false) => Gen::Standby,
                (x, _) => x,
            };
            self.gens.insert(s.clone(), (g, timer));
            let pos = if !lost {
                0
            } else {
                match g {
                    Gen::Running => 1,
                    Gen::Starting(_) => 3,
                    _ => 2,
                }
            };
            ats_pos.insert(s.clone(), pos);
            source_ok.insert(s.clone(), (pos == 0 || pos == 1) && !f.xfmr_trip.contains(s));
        }

        // ---------------- workload and the Flex power cap ----------------
        let it_nominal = self.last_it_kw;
        let mut flex_cap: Vec<f64> = vec![1.0; self.l.racks.len()];
        let mut bess_kw = 0.0;
        if f.dr_event == 2 {
            let shed = f.flex_cut_frac * it_nominal;
            if !f.bess_isolated && self.bess_soc > 5.0 {
                bess_kw = shed.min(self.attr("BESS-1", "power_kw", 1000.0));
            }
            let mut remaining = (shed - bess_kw).max(0.0);
            for (tier, floor) in [(Some(Tier::Spot), 0.4), (None, 0.4), (Some(Tier::Standard), 0.85)] {
                if remaining <= 0.0 {
                    break;
                }
                let idx: Vec<usize> = (0..self.l.racks.len()).filter(|i| self.l.racks[*i].tier == tier).collect();
                let avail: f64 = idx.iter().map(|i| self.racks[*i].p_raw * (1.0 - floor)).sum();
                if avail <= 0.0 {
                    continue;
                }
                let frac = (remaining / avail).min(1.0);
                for i in idx {
                    flex_cap[i] = 1.0 - (1.0 - floor) * frac;
                }
                remaining -= avail * frac;
            }
        }

        // ---------------- UPS, switchboards, busways, taps ----------------
        let mut side_up: BTreeMap<String, bool> = BTreeMap::new();
        let mut ups_mode: BTreeMap<String, i64> = BTreeMap::new();
        for s in &sides {
            let mods = self.l.ups.get(s).cloned().unwrap_or_default();
            let mut delivering = 0;
            for m in &mods {
                let soc = self.soc[m];
                let mode = if f.ups_fault.contains(m) {
                    3
                } else if f.ups_bypass.contains(s) {
                    if source_ok[s] { 2 } else { 4 }
                } else if source_ok[s] {
                    if f.ups_alarm.contains(m) { 5 } else { 0 }
                } else if soc > 0.5 {
                    1
                } else {
                    4
                };
                if matches!(mode, 0 | 1 | 2 | 5) {
                    delivering += 1;
                }
                ups_mode.insert(m.clone(), mode);
            }
            side_up.insert(s.clone(), delivering >= self.l.ups_need.min(mods.len().max(1)));
        }
        let swb_alive = |s: &str| side_up.get(s).copied().unwrap_or(false);
        let busway_alive: BTreeMap<String, bool> = self
            .l
            .busways
            .iter()
            .map(|(id, (_, s))| (id.clone(), swb_alive(s) && !f.busway_fault.contains(id)))
            .collect();
        let tap_alive = |tap: &str| -> bool {
            if f.tap_trip.contains(tap) {
                return false;
            }
            // TAP id: <rack>-TAP-<side>; busway: BW-<row>-<side>
            let side = tap.rsplit('-').next().unwrap_or("A");
            let rack_row = tap.split("-TAP-").next().unwrap_or("");
            let row = rack_row.rsplitn(2, '-').nth(1).unwrap_or("");
            busway_alive.get(&format!("BW-{row}-{side}")).copied().unwrap_or(false)
        };

        // ---------------- facility water and heat rejection ----------------
        let mech_powered = swb_alive("A") || swb_alive("B");
        let ats_powered = (ats_pos["A"] <= 1) || (ats_pos.get("B").copied().unwrap_or(2) <= 1);
        let racks_heat_liq: f64 = self.racks.iter().map(|r| r.p_kw * 0.85).sum();
        let racks_heat_air: f64 = self.racks.iter().map(|r| r.p_kw * 0.15).sum();
        let mut dc_cap = 0.0;
        let dcs = self.l.dcs.clone();
        for d in &dcs {
            let running = mech_powered && !f.dc_fault.contains(d);
            let cap = self.attr(d, "capacity_kw", 2200.0) * (1.0 - 0.035 * (wb - 18.0)).clamp(0.4, 1.15);
            if running {
                dc_cap += cap;
            }
        }
        let fwps = self.l.fwps.clone();
        let pumps_avail: Vec<&String> = fwps.iter().filter(|p| mech_powered && !f.fwp_trip.contains(*p)).collect();
        let lead_pump = pumps_avail.first().map(|p| (*p).clone());
        let fw_design = self.attr("FWS", "design_flow_lpm", 5000.0);
        let fw_flow = if lead_pump.is_some() { fw_design * 0.97 } else { 0.0 };
        let load_ratio = if dc_cap > 0.0 { racks_heat_liq / dc_cap } else { 3.0 };
        let fws_target = if fw_flow <= 0.0 { 46.0 } else { wb + 4.0 + 2.0 * load_ratio + 12.0 * (load_ratio - 1.0).max(0.0) };
        self.fws_t = lag(self.fws_t, fws_target.min(48.0), dt, if fw_flow <= 0.0 { 6.0 } else { 8.0 });
        let fw_ret = if fw_flow > 0.0 { self.fws_t + (racks_heat_liq * 60.0 / (fw_flow * 4.18)).min(15.0) } else { self.fws_t };

        // ---------------- in-row CDUs and racks ----------------
        let mut cdu_out: BTreeMap<String, (i64, f64, f64, f64)> = BTreeMap::new(); // status, flow, supply, return
        let rows = self.l.rows.clone();
        for row in &rows {
            let cdus = self.l.cdus_by_row.get(row).cloned().unwrap_or_default();
            let ridx: Vec<usize> = (0..self.l.racks.len()).filter(|i| self.l.racks[*i].row == *row).collect();
            let demand: f64 = ridx
                .iter()
                .map(|i| if f.rack_isolated.contains(&self.l.racks[*i].id) { 0.0 } else { self.l.racks[*i].design_flow })
                .sum();
            let mut maxes = vec![];
            let mut caps = 0.0;
            for c in &cdus {
                let avail = !f.cdu_off.contains(c) && mech_powered && fw_flow > 0.0;
                let wear = f.cdu_wear.get(c).copied().unwrap_or(0.0).clamp(0.0, 0.95);
                let m = if avail { self.attr(c, "design_flow_lpm", 900.0) * (1.0 - wear) } else { 0.0 };
                if avail {
                    caps += self.attr(c, "capacity_kw", 650.0) * (1.0 - 0.6 * wear);
                }
                maxes.push((c.clone(), avail, m));
            }
            let total_max: f64 = maxes.iter().map(|x| x.2).sum();
            let delivered = demand.min(total_max);
            let heat: f64 = ridx.iter().map(|i| self.racks[*i].p_kw * 0.85).sum();
            let ratio = if caps > 0.0 { heat / caps } else { 4.0 };
            let approach = if caps > 0.0 { 2.0 + 2.5 * ratio + 20.0 * (ratio - 1.0).max(0.0) } else { 30.0 };
            let ts_target = self.fws_t + approach;
            let mut row_supply = 0.0;
            for (c, avail, m) in &maxes {
                let ts = lag(self.cdu_ts[c], ts_target.min(60.0), dt, 4.0);
                self.cdu_ts.insert(c.clone(), ts);
                row_supply += ts;
                let share = if total_max > 0.0 { delivered * m / total_max } else { 0.0 };
                let ret = if delivered > 1.0 { ts + heat * 60.0 / (delivered * 4.18) } else { ts + 20.0 };
                let lvl = self.reservoir[c] - f.cdu_leak_rate.get(c).copied().unwrap_or(0.0) * dt / 60.0;
                let lvl = if f.cdu_leak_rate.contains_key(c) { lvl.max(5.0) } else { lag(lvl, 94.0, dt, 20.0) };
                self.reservoir.insert(c.clone(), lvl);
                cdu_out.insert(c.clone(), (if *avail { 1 } else { 0 }, share * (0.99 + 0.0), ts, ret));
            }
            let supply = if maxes.is_empty() { self.fws_t } else { row_supply / maxes.len() as f64 };
            for i in ridx {
                let st = self.l.racks[i].clone();
                let isolated = f.rack_isolated.contains(&st.id);
                let flow = if demand > 0.0 && !isolated { delivered * st.design_flow / demand } else { 0.0 };
                let r = &mut self.racks[i];
                r.flow = flow;
                r.supply = supply + 0.4;
                let q = r.p_kw * 0.85;
                let target = if flow > 5.0 { r.supply + q * 60.0 / (flow * 4.18) } else { r.supply + if r.p_kw > 1.0 { 45.0 } else { 2.0 } };
                r.ret = lag(r.ret, target, dt, 1.5);
                let thr_t = if r.ret < 47.0 { 1.0 } else { (1.0 - 0.07 * (r.ret - 47.0)).clamp(0.35, 1.0) };
                r.throttle = lag(r.throttle, thr_t, dt, 2.0);
                if r.ret > 58.0 {
                    r.shutdown = true;
                }
            }
        }

        // ---------------- rack power ----------------
        let mut it_kw = 0.0;
        let mut tap_kw: BTreeMap<String, f64> = BTreeMap::new();
        for i in 0..self.l.racks.len() {
            let st = self.l.racks[i].clone();
            let feeds: Vec<&String> = st.taps.iter().filter(|t| tap_alive(t)).collect();
            let (base, amp, nz) = match st.tier {
                Some(Tier::Premium) => (0.80, 0.10, 0.02),
                Some(Tier::Standard) => (0.72, 0.13, 0.03),
                Some(Tier::Spot) => (0.66, 0.18, 0.05),
                None => (0.18, 0.04, 0.02),
            };
            let n = self.noise(nz);
            let r = &mut self.racks[i];
            r.noise = (r.noise * 0.9 + n).clamp(-0.08, 0.08);
            let util = (base + amp * day + r.noise).clamp(0.05, 1.0);
            let failed = f.rack_psu_failed.get(&st.id).copied().unwrap_or(0) as f64;
            let psu_cap = if st.psus - failed <= st.psus - 2.0 { 0.75 } else { 1.0 };
            r.cap = flex_cap[i].min(psu_cap);
            if r.shutdown && r.ret < 40.0 && !feeds.is_empty() && !f.rack_isolated.contains(&st.id) {
                r.shutdown = false;
                r.throttle = 0.6;
            }
            r.p_raw = if feeds.is_empty() || r.shutdown { 0.0 } else { st.rated_kw * util * r.throttle };
            r.p_kw = r.p_raw * r.cap;
            it_kw += r.p_kw;
            for t in &feeds {
                *tap_kw.entry((*t).clone()).or_default() += r.p_kw / feeds.len() as f64;
            }
        }
        self.last_it_kw = it_kw.max(500.0);

        // mechanical (critical, on UPS) and air-side (on ATS) loads
        let cdu_count = self.l.cdus_by_row.values().map(|v| v.len()).sum::<usize>() as f64;
        let fan = (racks_heat_liq / dc_cap.max(1.0) * 100.0).clamp(20.0, 100.0);
        let mech_kw = cdu_count * 6.0 + dcs.len() as f64 * 45.0 * fan / 100.0 + 35.0 + (self.l.leafs.len() as f64 * 1.5) + 6.0;
        let air_kw = self.l.crahs.len() as f64 * 8.0 + 140.0;
        let alive_sides: Vec<&String> = sides.iter().filter(|s| swb_alive(s)).collect();
        let mut swb_kw: BTreeMap<String, f64> = sides.iter().map(|s| (s.clone(), 0.0)).collect();
        let mut bw_kw: BTreeMap<String, f64> = BTreeMap::new();
        for (t, kw) in &tap_kw {
            let side = t.rsplit('-').next().unwrap_or("A").to_string();
            let rack_row = t.split("-TAP-").next().unwrap_or("");
            let row = rack_row.rsplitn(2, '-').nth(1).unwrap_or("");
            *bw_kw.entry(format!("BW-{row}-{side}")).or_default() += kw;
            *swb_kw.entry(side).or_default() += kw;
        }
        for s in &alive_sides {
            *swb_kw.get_mut(*s).unwrap() += mech_kw / alive_sides.len() as f64;
        }

        // UPS batteries
        let mut ups_load: BTreeMap<String, f64> = BTreeMap::new();
        for s in &sides {
            let mods = self.l.ups.get(s).cloned().unwrap_or_default();
            let delivering: Vec<&String> = mods.iter().filter(|m| matches!(ups_mode[*m], 0 | 1 | 2 | 5)).collect();
            let per = if delivering.is_empty() { 0.0 } else { swb_kw[s] / delivering.len() as f64 };
            for m in &mods {
                let load = if delivering.contains(&m) { per } else { 0.0 };
                ups_load.insert(m.clone(), load);
                let cap = self.attr(m, "capacity_kw", 1500.0);
                let energy = cap * self.attr(m, "runtime_min", 9.0) / 60.0;
                let soc = self.soc[m];
                let soc = if ups_mode[m] == 1 {
                    (soc - load * dt / 3600.0 / energy * 100.0).max(0.0)
                } else if matches!(ups_mode[m], 0 | 5) {
                    (soc + 4.0 * dt / 60.0).min(100.0)
                } else {
                    soc
                };
                self.soc.insert(m.clone(), soc);
            }
        }

        // transformers
        for s in &sides {
            let load = if source_ok[s] { swb_kw[s] * 1.03 + air_kw / 2.0 } else { 0.0 };
            let cap = self.attr(&format!("XFMR-{s}"), "capacity_kw", 5000.0);
            let target = 55.0 + 75.0 * (load / cap).powi(2) + f.xfmr_extra.get(s).copied().unwrap_or(0.0);
            let w = lag(self.winding[s], target, dt, 20.0);
            self.winding.insert(s.clone(), w);
            if record {
                self.put(&format!("XFMR-{s}"), "xfmr_load_kw", load);
                self.put(&format!("XFMR-{s}"), "winding_temp", w);
            }
        }

        // air side: chillers and CRAHs
        let chillers = self.l.chillers.clone();
        let ch_avail: Vec<&String> = chillers.iter().filter(|c| ats_powered && !f.ch_trip.contains(*c)).collect();
        let lead_ch = ch_avail.first().map(|c| (*c).clone());
        let chw_target = if lead_ch.is_some() { 12.0 + lead_ch.as_ref().and_then(|c| f.ch_head.get(c)).copied().unwrap_or(0.0) * 0.006 } else { 22.0 };
        self.chw_t = lag(self.chw_t, chw_target, dt, 20.0);
        let crahs = self.l.crahs.clone();
        let crah_run = crahs.iter().filter(|c| ats_powered && !f.crah_fault.contains(*c)).count() as f64;
        let air_cap = crah_run * self.attr("CRAH-1", "capacity_kw", 160.0) * (1.0 - ((self.chw_t - 14.0) / 16.0).clamp(0.0, 0.6));
        let air_ratio = if air_cap > 0.0 { racks_heat_air / air_cap } else { 3.0 };
        let air_target = self.chw_t + 7.0 + 6.0 * (air_ratio - 1.0).max(0.0);
        self.air_t = lag(self.air_t, air_target.min(38.0), dt, 8.0);

        // BESS
        let bess_status = if f.bess_isolated {
            4
        } else if bess_kw > 0.0 {
            2
        } else if self.bess_soc < 95.0 {
            1
        } else {
            0
        };
        let bess_kwh = self.attr("BESS-1", "capacity_kwh", 2000.0);
        if bess_status == 2 {
            self.bess_soc = (self.bess_soc - bess_kw * dt / 3600.0 / bess_kwh * 100.0).max(0.0);
        } else if bess_status == 1 {
            self.bess_soc = (self.bess_soc + 0.3 * dt / 60.0).min(100.0);
        }
        self.bess_temp = lag(self.bess_temp, 24.0 + 0.008 * bess_kw + f.bess_extra_temp, dt, 10.0);

        // generator fuel: burns while running; the contractor tops the day tank back up after a run
        for s in &sides {
            let g = self.gens[s].0;
            let rate = match g {
                Gen::Running => -0.4,
                Gen::Standby => 0.25,
                _ => 0.0,
            };
            let fu = self.fuel[s] + rate * dt / 60.0;
            self.fuel.insert(s.clone(), fu.clamp(0.0, 100.0));
        }

        if !record {
            return;
        }
        // ======================= readings =======================
        let facility = it_kw + mech_kw + air_kw + it_kw * 0.04;
        let any_util = util_code.values().any(|u| *u != 0);
        self.put("dsx1", "site_it_kw", it_kw);
        self.put("dsx1", "site_facility_kw", facility);
        self.put("dsx1", "site_pue", if it_kw > 1.0 { facility / it_kw } else { 0.0 });
        let fq = 60.0 + f.grid_freq_dev + self.noise(0.012);
        self.put("GRID", "grid_frequency", fq);
        self.put("GRID", "grid_dr_event", f.dr_event as f64);
        self.put("GRID", "grid_import_kw", if any_util { (facility - bess_kw).max(0.0) } else { 0.0 });
        for s in &sides {
            let u = util_code[s];
            self.put(&format!("UTIL-{s}"), "util_status", u as f64);
            let v = match u {
                0 => 0.0,
                2 => 13_800.0 * (0.91 + self.noise(0.01)),
                _ => 13_800.0 * (1.0 + f.util_voltage_dev + self.noise(0.004)),
            };
            self.put(&format!("UTIL-{s}"), "util_voltage", v);
            let (g, _) = self.gens[s];
            let code = match g {
                Gen::Standby => 0,
                Gen::Starting(_) => 1,
                Gen::Running => 2,
                Gen::FailToStart => 3,
            };
            self.put(&format!("GEN-{s}"), "gen_status", code as f64);
            self.put(&format!("GEN-{s}"), "gen_output_kw", if g == Gen::Running && util_code[s] == 0 { swb_kw[s] + air_kw / 2.0 } else { 0.0 });
            let fuel = self.fuel[s];
            self.put(&format!("GEN-{s}"), "gen_fuel_pct", fuel);
            self.put(&format!("ATS-{s}"), "ats_position", ats_pos[s] as f64);
            self.put(&format!("SWB-{s}"), "active_power", if swb_alive(s) { swb_kw[s] } else { 0.0 });
            self.put(&format!("SWB-{s}"), "bus_status", if swb_alive(s) { 1.0 } else { 0.0 });
            self.put(&format!("XFMR-{s}"), "xfmr_status", if f.xfmr_trip.contains(s) { 0.0 } else { 1.0 });
            for m in self.l.ups.get(s).cloned().unwrap_or_default() {
                let cap = self.attr(&m, "capacity_kw", 1500.0);
                let load = ups_load[&m];
                let energy = cap * self.attr(&m, "runtime_min", 9.0) / 60.0;
                let soc = self.soc[&m];
                self.put(&m, "ups_mode", ups_mode[&m] as f64);
                self.put(&m, "ups_load_pct", load / cap * 100.0);
                self.put(&m, "battery_soc", soc);
                self.put(&m, "runtime_min", if load > 1.0 { (soc / 100.0 * energy / load * 60.0).min(240.0) } else { 240.0 });
            }
        }
        self.put("BESS-1", "bess_status", bess_status as f64);
        self.put("BESS-1", "bess_soc", self.bess_soc);
        self.put("BESS-1", "bess_power_kw", bess_kw);
        self.put("BESS-1", "bess_cell_temp", self.bess_temp);
        for (bw, (_, s)) in self.l.busways.clone() {
            let kw = if busway_alive[&bw] { bw_kw.get(&bw).copied().unwrap_or(0.0) } else { 0.0 };
            self.put(&bw, "active_power", kw);
            self.put(&bw, "bus_status", if busway_alive[&bw] { 1.0 } else { 0.0 });
            let hot = f.busway_hot.get(&bw).copied().unwrap_or(0.0);
            let _ = s;
            self.put(&bw, "current", amps(kw) * (1.0 + hot));
        }
        // cooling plant
        for d in &dcs {
            let code = if f.dc_fault.contains(d) { 2 } else if mech_powered { 1 } else { 0 };
            self.put(d, "ct_status", code as f64);
            { let v = (self.fws_t - wb).max(0.0) + self.noise(0.1); self.put(d, "approach_temp", v); }
            self.put(d, "fan_speed", if code == 1 { fan } else { 0.0 });
        }
        for p in &fwps {
            let code = if f.fwp_trip.contains(p) { 2 } else if Some(p.clone()) == lead_pump { 1 } else { 0 };
            self.put(p, "fwp_status", code as f64);
            let v = if code == 1 { 2.1 + f.fwp_vib.get(p).copied().unwrap_or(0.0) } else { 0.3 } + self.noise(0.08);
            self.put(p, "fwp_vibration", v.max(0.0));
        }
        { let v = self.fws_t + self.noise(0.05); self.put("FWS", "fw_supply_temp", v); }
        self.put("FWS", "fw_return_temp", fw_ret);
        self.put("FWS", "fw_flow", fw_flow);
        for c in &chillers {
            let code = if f.ch_trip.contains(c) {
                3
            } else if f.ch_alarm.contains(c) {
                2
            } else if Some(c.clone()) == lead_ch {
                1
            } else {
                0
            };
            self.put(c, "chiller_status", code as f64);
            self.put(c, "chw_supply_temp", self.chw_t);
            let hp = 1150.0 + f.ch_head.get(c).copied().unwrap_or(0.0) + self.noise(8.0);
            self.put(c, "head_pressure", if code == 3 { 600.0 } else { hp });
        }
        for c in &crahs {
            let code = if f.crah_fault.contains(c) { 2 } else if ats_powered { 1 } else { 0 };
            self.put(c, "crah_status", code as f64);
            { let v = self.air_t + self.noise(0.1); self.put(c, "supply_air_temp", v); }
        }
        for (c, (st, flow, ts, ret)) in cdu_out.clone() {
            self.put(&c, "cdu_status", st as f64);
            { let v = ts + self.noise(0.05); self.put(&c, "cdu_supply_temp", v); }
            self.put(&c, "cdu_return_temp", ret);
            { let v = flow * (1.0 + self.noise(0.005)); self.put(&c, "cdu_flow", v); }
            let wear = f.cdu_wear.get(&c).copied().unwrap_or(0.0);
            let vib = 2.2 + 6.0 * wear + f.cdu_vib.get(&c).copied().unwrap_or(0.0) + self.noise(0.1);
            self.put(&c, "pump_vibration", if st == 1 { vib } else { 0.2 });
            let lvl = self.reservoir[&c];
            self.put(&c, "reservoir_level", lvl);
            self.put(&c, "cdu_leak", if f.cdu_leak_detect.contains(&c) { 1.0 } else { 0.0 });
        }
        // network
        for s in self.l.spines.clone() {
            let up = !f.switch_down.contains(&s) && mech_powered;
            self.put(&s, "switch_status", if up { 1.0 } else { 0.0 });
            let e = 1.0 + f.link_errors.get(&s).copied().unwrap_or(0.0) + self.noise(1.0).abs();
            self.put(&s, "link_errors", if up { e } else { 0.0 });
        }
        for s in self.l.leafs.clone() {
            let up = !f.switch_down.contains(&s) && mech_powered;
            self.put(&s, "switch_status", if up { 1.0 } else { 0.0 });
            let e = 1.0 + f.link_errors.get(&s).copied().unwrap_or(0.0) + self.noise(1.0).abs();
            self.put(&s, "link_errors", if up { e } else { 0.0 });
        }
        // racks
        let air = self.air_t;
        for i in 0..self.l.racks.len() {
            let st = self.l.racks[i].clone();
            let r = self.racks[i].clone();
            let id = st.id.as_str();
            let seep = f.rack_seep.get(id).copied().unwrap_or(0.0);
            let isolated = f.rack_isolated.contains(id);
            self.put(id, "rack_power", r.p_kw);
            { let v = r.supply + self.noise(0.05); self.put(id, "liquid_supply_temp", v); }
            { let v = r.ret + self.noise(0.08); self.put(id, "liquid_return_temp", v); }
            { let v = r.flow * (1.0 + self.noise(0.004)); self.put(id, "liquid_flow", v); }
            let dp = st.design_dp * (r.flow / st.design_flow).powi(2) * (1.0 - 0.35 * seep);
            { let v = dp + self.noise(0.6); self.put(id, "liquid_dp", v); }
            self.put(id, "valve_pos", if isolated { 0.0 } else { 100.0 });
            self.put(id, "rack_leak", if f.rack_leak.contains(id) { 1.0 } else { 0.0 });
            self.put(id, "liquid_isolation", if isolated { 1.0 } else { 0.0 });
            self.put(id, "power_cap_pct", if r.p_kw > 0.0 { r.cap * 100.0 } else { 100.0 });
            self.put(id, "gpu_perf_pct", if r.shutdown || r.p_kw <= 0.0 { 0.0 } else { r.throttle * 100.0 });
            { let v = air + 2.5 + r.p_kw / 400.0 + self.noise(0.1); self.put(id, "inlet_air_temp", v); }
            let failed = f.rack_psu_failed.get(id).copied().unwrap_or(0) as f64;
            self.put(id, "psu_healthy", st.psus - failed);
            for side in 0..2 {
                let tap = st.taps[side].clone();
                if tap.is_empty() {
                    continue;
                }
                let alive = tap_alive(&tap);
                let kw = tap_kw.get(&tap).copied().unwrap_or(0.0);
                let ov = f.tap_overload.get(&tap).copied().unwrap_or(0.0);
                let rating = self.attr(&tap, "current_limit_a", 250.0);
                let a = if ov > 0.0 && alive { amps(kw).max(rating * ov) } else { amps(kw) };
                self.put(&tap, "current", a);
                self.put(&tap, "breaker_status", if alive || !f.tap_trip.contains(&tap) { 1.0 } else { 0.0 });
            }
            let nvl = format!("{id}-NVL");
            let failed = f.nvl_failed.get(&nvl).copied().unwrap_or(0) as f64;
            self.put(&nvl, "nvl_trays_ok", 9.0 - failed);
            let e = 0.4 + f.nvl_errors.get(&nvl).copied().unwrap_or(0.0) + self.noise(0.3).abs();
            self.put(&nvl, "nvl_errors", e);
        }
        for (tray, _) in self.l.trays.clone() {
            let h = f.tray_health.get(&tray).copied().unwrap_or(0);
            self.readings.insert(format!("{tray}.gpu_health"), (h as f64, 1));
        }
        // sensor faults and comms loss
        let offsets: Vec<(String, f64)> = f.sensor_offset.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (p, off) in offsets {
            if let Some(v) = self.readings.get_mut(&p) {
                v.0 += off;
            }
        }
        let stuck: BTreeSet<String> = f.sensor_stuck.clone();
        for p in &stuck {
            let cur = self.readings.get(p).map(|v| v.0).unwrap_or(0.0);
            let held = *self.stuck.entry(p.clone()).or_insert(cur);
            if let Some(v) = self.readings.get_mut(p) {
                v.0 = held;
            }
        }
        self.stuck.retain(|k, _| stuck.contains(k));
        if !f.comms_lost.is_empty() {
            let lost = f.comms_lost.clone();
            for (k, v) in self.readings.iter_mut() {
                let asset = k.split('.').next().unwrap_or("");
                if lost.contains(asset) {
                    v.1 = 0;
                }
            }
        }
    }
}
