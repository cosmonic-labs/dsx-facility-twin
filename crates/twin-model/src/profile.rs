//! Site profiles (YAML) → expanded topology + config. Native only.
//!
//! A profile directory holds four files; docs/mapping.md explains each one:
//!
//! * `site.yaml`     — the site, plus either a parametric `template` (a DSX reference-style
//!                     GB200 NVL72 hall) or an explicit `assets:` list, or both.
//! * `tenants.yaml`  — customers, tiers, prices and what they are allocated.
//! * `mapping.yaml`  — the customer's point types and object ids → twin classes and assets.
//! * `classes.yaml`  — canonical classes with units, limits and status tables.
use crate::config::{ClassDef, SubjectRule, TwinConfig};
use crate::msg::ProfileMsg;
use crate::topo::*;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct SiteFile {
    pub site: SiteInfo,
    #[serde(default)]
    pub template: Option<Template>,
    /// Explicit assets (customer import). Appended after the template, same ids win.
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Template {
    pub pods: u32,
    pub rows_per_pod: u32,
    pub racks_per_row: u32,
    pub rack: RackSpec,
    pub power: PowerSpec,
    pub cooling: CoolingSpec,
    pub network: NetworkSpec,
    #[serde(default)]
    pub bess: Option<BessSpec>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RackSpec {
    pub rated_kw: f64,
    pub gpus: u32,
    pub trays: u32,
    pub design_flow_lpm: f64,
    pub design_dp_kpa: f64,
    pub psus: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PowerSpec {
    pub ups_modules_per_side: u32,
    pub ups_need: u32,
    pub ups_module_kw: f64,
    pub ups_runtime_min: f64,
    pub generator_kw: f64,
    pub transformer_kw: f64,
    pub tap_limit_a: f64,
    pub busway_limit_a: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoolingSpec {
    pub cdus_per_row: u32,
    pub cdu_need: u32,
    pub cdu_capacity_kw: f64,
    pub cdu_max_flow_lpm: f64,
    pub dry_coolers: u32,
    pub dry_cooler_need: u32,
    pub dry_cooler_kw: f64,
    pub fw_pumps: u32,
    pub chillers: u32,
    pub crahs: u32,
    pub crah_need: u32,
    pub crah_kw: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkSpec {
    pub spines: u32,
    pub leafs_per_row: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BessSpec {
    pub kwh: f64,
    pub kw: f64,
}

#[derive(Debug, Deserialize)]
pub struct TenantsFile {
    pub customers: Vec<CustomerSpec>,
}

#[derive(Debug, Deserialize)]
pub struct CustomerSpec {
    pub id: String,
    pub name: String,
    pub tier: Tier,
    pub rate_per_gpu_hr: f64,
    pub sla_pct: f64,
    #[serde(default)]
    pub color: String,
    /// Whole racks. `P1-R1-01..04` expands to 01, 02, 03, 04.
    #[serde(default)]
    pub racks: Vec<String>,
    /// Individual compute trays (partial racks), same range syntax.
    #[serde(default)]
    pub trays: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MappingFile {
    #[serde(default)]
    pub point_types: BTreeMap<String, String>,
    #[serde(default)]
    pub asset_aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub subject_rules: Vec<SubjectRule>,
}

#[derive(Debug, Deserialize)]
pub struct ClassesFile {
    pub classes: BTreeMap<String, ClassDef>,
}

/// Points generated for each asset kind by the template.
pub fn classes_for(kind: AssetKind) -> &'static [&'static str] {
    use AssetKind::*;
    match kind {
        Site => &["site_it_kw", "site_facility_kw", "site_pue"],
        Grid => &["grid_frequency", "grid_dr_event", "grid_import_kw"],
        UtilityFeed => &["util_status", "util_voltage"],
        Generator => &["gen_status", "gen_output_kw", "gen_fuel_pct"],
        Ats => &["ats_position"],
        Transformer => &["xfmr_status", "xfmr_load_kw", "winding_temp"],
        Ups => &["ups_mode", "ups_load_pct", "battery_soc", "runtime_min"],
        Bess => &["bess_status", "bess_soc", "bess_power_kw", "bess_cell_temp"],
        Switchboard => &["bus_status", "active_power"],
        Busway => &["bus_status", "active_power", "current"],
        TapOff => &["current", "breaker_status"],
        CoolingTower => &["ct_status", "approach_temp", "fan_speed"],
        FacilityPump => &["fwp_status", "fwp_vibration"],
        FacilityWater => &["fw_supply_temp", "fw_return_temp", "fw_flow"],
        Chiller => &["chiller_status", "chw_supply_temp", "head_pressure"],
        Crah => &["crah_status", "supply_air_temp"],
        Cdu => &["cdu_status", "cdu_supply_temp", "cdu_return_temp", "cdu_flow", "pump_vibration", "reservoir_level", "cdu_leak"],
        SpineSwitch | LeafSwitch => &["switch_status", "link_errors"],
        Rack => &[
            "rack_power", "liquid_supply_temp", "liquid_return_temp", "liquid_flow", "liquid_dp", "valve_pos",
            "rack_leak", "liquid_isolation", "power_cap_pct", "gpu_perf_pct", "inlet_air_temp", "psu_healthy",
        ],
        NvlinkFabric => &["nvl_trays_ok", "nvl_errors"],
        ComputeTray => &["gpu_health"],
        Hall | Pod | Row | PowerShelf => &[],
    }
}

/// Expand `1..04`-style ranges: `P1-R1-01..04` → P1-R1-01 … P1-R1-04.
pub fn expand_range(s: &str) -> Vec<String> {
    if let Some((head, end)) = s.split_once("..") {
        let digits = head.chars().rev().take_while(|c| c.is_ascii_digit()).count();
        let (prefix, start) = head.split_at(head.len() - digits);
        if let (Ok(a), Ok(b)) = (start.parse::<u32>(), end.parse::<u32>()) {
            return (a..=b).map(|n| format!("{prefix}{n:0w$}", w = digits)).collect();
        }
    }
    vec![s.to_string()]
}

fn req(domain: Domain, label: &str, need: u32, of: Vec<String>) -> Requirement {
    Requirement { domain, label: label.into(), need, of, bridged: false, partial: false }
}
fn partial(r: Requirement) -> Requirement {
    Requirement { partial: true, ..r }
}

struct Builder<'a> {
    site: &'a str,
    classes: &'a BTreeMap<String, ClassDef>,
    assets: Vec<Asset>,
}

impl Builder<'_> {
    fn add(&mut self, id: String, name: String, kind: AssetKind, parent: Option<&str>, area: &str, requires: Vec<Requirement>, attrs: &[(&str, f64)], tags: &[String]) {
        let mut points = vec![];
        for c in classes_for(kind) {
            let def = self.classes.get(*c);
            let (subject, unit, forecast) = match def {
                Some(d) if !d.dsx.is_empty() => {
                    let (ot, pt) = d.dsx.split_once('/').unwrap_or(("Point", d.dsx.as_str()));
                    (
                        format!("BMS.v1.PUB.Value.{ot}.{pt}.cosmonic.{}.{area}.{id}.{c}.value", self.site),
                        d.unit.clone(),
                        d.forecast,
                    )
                }
                Some(d) => (format!("twin.v1.health.{c}.{id}"), d.unit.clone(), d.forecast),
                None => (format!("twin.v1.health.{c}.{id}"), String::new(), false),
            };
            points.push(PointDef { id: format!("{id}.{c}"), class: c.to_string(), unit, subject, forecast });
        }
        self.assets.push(Asset {
            id,
            name,
            kind,
            parent: parent.map(String::from),
            requires,
            attrs: attrs.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            points,
            tags: tags.to_vec(),
        });
    }
}

fn sides() -> [&'static str; 2] {
    ["A", "B"]
}

/// Build a DSX reference-style GB200 NVL72 hall from the template.
pub fn expand_template(site: &SiteInfo, t: &Template, classes: &BTreeMap<String, ClassDef>) -> Vec<Asset> {
    let s = site.id.as_str();
    let mut b = Builder { site: s, classes, assets: vec![] };
    let tags = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    b.add(s.into(), site.name.clone(), AssetKind::Site, None, "site", vec![], &[], &[]);
    b.add("H1".into(), "Data hall 1".into(), AssetKind::Hall, Some(s), "plant", vec![], &[], &[]);

    let swb_ab = || vec!["SWB-A".to_string(), "SWB-B".to_string()];
    // ---- grid and power: 2N, A and B sides ----
    b.add("GRID".into(), "Grid interconnect".into(), AssetKind::Grid, Some(s), "plant", vec![], &[], &tags(&["flex:dsx"]));
    if let Some(bs) = &t.bess {
        b.add("BESS-1".into(), "BESS 1".into(), AssetKind::Bess, Some(s), "plant", vec![], &[("capacity_kwh", bs.kwh), ("power_kw", bs.kw)], &[]);
    }
    let p = &t.power;
    for side in sides() {
        let st = vec![format!("side:{side}")];
        b.add(format!("UTIL-{side}"), format!("Utility feed {side}"), AssetKind::UtilityFeed, Some(s), "plant",
            vec![req(Domain::Grid, "grid", 1, vec!["GRID".into()])], &[("nominal_v", 13_800.0)], &st);
        b.add(format!("GEN-{side}"), format!("Generator {side}"), AssetKind::Generator, Some(s), "plant", vec![], &[("capacity_kw", p.generator_kw)], &st);
        b.add(format!("ATS-{side}"), format!("Transfer switch {side}"), AssetKind::Ats, Some(s), "plant",
            vec![req(Domain::Power, "utility or generator", 1, vec![format!("UTIL-{side}"), format!("GEN-{side}")])], &[], &st);
        b.add(format!("XFMR-{side}"), format!("Transformer {side}"), AssetKind::Transformer, Some(s), "plant",
            vec![req(Domain::Power, "MV supply", 1, vec![format!("ATS-{side}")])], &[("capacity_kw", p.transformer_kw)], &st);
        let mods: Vec<String> = (1..=p.ups_modules_per_side).map(|m| format!("UPS-{side}{m}")).collect();
        for m in &mods {
            let mut r = req(Domain::Power, "UPS input", 1, vec![format!("XFMR-{side}")]);
            r.bridged = true;
            b.add(m.clone(), format!("UPS {}", &m[4..]), AssetKind::Ups, Some(s), "plant", vec![r],
                &[("capacity_kw", p.ups_module_kw), ("runtime_min", p.ups_runtime_min)], &st);
        }
        b.add(format!("SWB-{side}"), format!("Switchboard {side}"), AssetKind::Switchboard, Some("H1"), "plant",
            vec![partial(req(Domain::Power, "UPS modules", p.ups_need, mods.clone()))], &[], &st);
    }
    let ats_ab = || vec!["ATS-A".to_string(), "ATS-B".to_string()];

    // ---- facility cooling plant ----
    let c = &t.cooling;
    let dcs: Vec<String> = (1..=c.dry_coolers).map(|i| format!("DC-{i}")).collect();
    for d in &dcs {
        // Critical mechanical load (heat rejection, pumping) sits on UPS, as in DSX reference designs.
        b.add(d.clone(), format!("Dry cooler {}", &d[3..]), AssetKind::CoolingTower, Some(s), "plant",
            vec![req(Domain::Power, "UPS power A/B", 1, swb_ab())], &[("capacity_kw", c.dry_cooler_kw)], &[]);
    }
    let fwps: Vec<String> = (1..=c.fw_pumps).map(|i| format!("FWP-{i}")).collect();
    for f in &fwps {
        b.add(f.clone(), format!("Facility pump {}", &f[4..]), AssetKind::FacilityPump, Some(s), "plant",
            vec![req(Domain::Power, "UPS power A/B", 1, swb_ab())], &[], &[]);
    }
    let total_rows = (t.pods * t.rows_per_pod) as f64;
    b.add("FWS".into(), "Facility water loop".into(), AssetKind::FacilityWater, Some(s), "plant",
        vec![
            partial(req(Domain::Cooling, "heat rejection", c.dry_cooler_need, dcs.clone())),
            req(Domain::Cooling, "facility pumping", 1, fwps.clone()),
        ],
        &[("design_flow_lpm", c.cdu_max_flow_lpm * total_rows * 1.2)], &[]);
    let chs: Vec<String> = (1..=c.chillers).map(|i| format!("CH-{i}")).collect();
    for ch in &chs {
        b.add(ch.clone(), format!("Trim chiller {}", &ch[3..]), AssetKind::Chiller, Some(s), "plant",
            vec![req(Domain::Power, "mechanical power A/B", 1, ats_ab())], &[], &[]);
    }
    let crahs: Vec<String> = (1..=c.crahs).map(|i| format!("CRAH-{i}")).collect();
    for cr in &crahs {
        b.add(cr.clone(), format!("CRAH {}", &cr[5..]), AssetKind::Crah, Some("H1"), "plant",
            vec![
                req(Domain::Cooling, "chilled water", 1, chs.clone()),
                req(Domain::Power, "power A/B", 1, ats_ab()),
            ],
            &[("capacity_kw", c.crah_kw)], &[]);
    }

    // ---- network core ----
    let spines: Vec<String> = (1..=t.network.spines).map(|i| format!("SPINE-{i}")).collect();
    for sp in &spines {
        b.add(sp.clone(), format!("Spine {}", &sp[6..]), AssetKind::SpineSwitch, Some("H1"), "plant",
            vec![req(Domain::Power, "power A/B", 1, swb_ab())], &[], &[]);
    }

    // ---- pods, rows, racks ----
    let r = &t.rack;
    for pi in 1..=t.pods {
        let pod = format!("P{pi}");
        b.add(pod.clone(), format!("Pod {pi}"), AssetKind::Pod, Some("H1"), &pod, vec![], &[], &[]);
        for ri in 1..=t.rows_per_pod {
            let row = format!("{pod}-R{ri}");
            let rown = (pi - 1) * t.rows_per_pod + ri;
            b.add(row.clone(), format!("Row {rown}"), AssetKind::Row, Some(&pod), &pod, vec![], &[], &[]);
            let mut busways = vec![];
            for side in sides() {
                let bw = format!("BW-{row}-{side}");
                b.add(bw.clone(), format!("Busway {rown}{side}"), AssetKind::Busway, Some(&row), &pod,
                    vec![req(Domain::Power, "switchboard", 1, vec![format!("SWB-{side}")])],
                    &[("current_limit_a", p.busway_limit_a)], &[format!("side:{side}")]);
                busways.push(bw);
            }
            let cdus: Vec<String> = (0..c.cdus_per_row).map(|k| format!("CDU-{row}-{}", (b'A' + k as u8) as char)).collect();
            for (k, cd) in cdus.iter().enumerate() {
                b.add(cd.clone(), format!("CDU {rown}{}", (b'A' + k as u8) as char), AssetKind::Cdu, Some(&row), &pod,
                    vec![
                        req(Domain::Power, "power A/B", 1, swb_ab()),
                        req(Domain::Cooling, "facility water", 1, vec!["FWS".into()]),
                    ],
                    &[("capacity_kw", c.cdu_capacity_kw), ("design_flow_lpm", c.cdu_max_flow_lpm)], &[]);
            }
            let leafs: Vec<String> = (0..t.network.leafs_per_row).map(|k| format!("LEAF-{row}-{}", (b'A' + k as u8) as char)).collect();
            for (k, lf) in leafs.iter().enumerate() {
                b.add(lf.clone(), format!("Leaf {rown}{}", (b'A' + k as u8) as char), AssetKind::LeafSwitch, Some(&row), &pod,
                    vec![
                        req(Domain::Network, "spine uplinks", 1, spines.clone()),
                        req(Domain::Power, "power A/B", 1, swb_ab()),
                    ], &[], &[]);
            }
            for k in 1..=t.racks_per_row {
                let rack = format!("{row}-{k:02}");
                let rname = format!("Rack {rown}-{k:02}");
                let mut taps = vec![];
                for side in sides() {
                    let tap = format!("{rack}-TAP-{side}");
                    b.add(tap.clone(), format!("Tap {rown}-{k:02}{side}"), AssetKind::TapOff, Some(&row), &pod,
                        vec![req(Domain::Power, "busway", 1, vec![format!("BW-{row}-{side}")])],
                        &[("current_limit_a", p.tap_limit_a)], &[format!("side:{side}")]);
                    taps.push(tap);
                }
                b.add(rack.clone(), rname.clone(), AssetKind::Rack, Some(&row), &pod,
                    vec![
                        req(Domain::Power, "power A/B", 1, taps),
                        req(Domain::Cooling, "liquid cooling", c.cdu_need, cdus.clone()),
                        // Room air has thermal inertia: losing CRAHs is a ride-through, not an instant trip.
                        Requirement { bridged: true, partial: true, ..req(Domain::Cooling, "air cooling", c.crah_need, crahs.clone()) },
                    ],
                    &[("rated_kw", r.rated_kw), ("gpus", r.gpus as f64), ("design_flow_lpm", r.design_flow_lpm),
                      ("design_dp_kpa", r.design_dp_kpa), ("psu_count", r.psus as f64)], &[]);
                let nvl = format!("{rack}-NVL");
                b.add(nvl.clone(), format!("NVLink {rown}-{k:02}"), AssetKind::NvlinkFabric, Some(&rack), &pod,
                    vec![req(Domain::Compute, "rack", 1, vec![rack.clone()])], &[("switch_trays", 9.0)], &[]);
                let per = r.gpus / r.trays.max(1);
                for tr in 1..=r.trays {
                    b.add(format!("{rack}-T{tr:02}"), format!("Tray {rown}-{k:02}.{tr:02}"), AssetKind::ComputeTray, Some(&rack), &pod,
                        vec![
                            req(Domain::Compute, "rack", 1, vec![rack.clone()]),
                            req(Domain::Compute, "NVLink", 1, vec![nvl.clone()]),
                            req(Domain::Network, "fabric uplinks", 1, leafs.clone()),
                        ],
                        &[("gpus", per as f64)], &[]);
                }
            }
        }
    }
    b.assets
}

fn read<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<T, String> {
    let p = dir.join(name);
    let s = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_yaml::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// Load a profile directory into the message the twin consumes.
pub fn load_dir(dir: &Path, version: u64) -> Result<ProfileMsg, String> {
    let site: SiteFile = read(dir, "site.yaml")?;
    let tenants: TenantsFile = read(dir, "tenants.yaml")?;
    let mapping: MappingFile = read(dir, "mapping.yaml").unwrap_or_default();
    let classes: ClassesFile = read(dir, "classes.yaml")?;

    let mut assets = match &site.template {
        Some(t) => expand_template(&site.site, t, &classes.classes),
        None => vec![],
    };
    for a in site.assets {
        if let Some(pos) = assets.iter().position(|x| x.id == a.id) {
            assets[pos] = a;
        } else {
            assets.push(a);
        }
    }
    let ids: std::collections::BTreeSet<&str> = assets.iter().map(|a| a.id.as_str()).collect();
    let mut customers = vec![];
    for c in tenants.customers {
        let mut allocations = vec![];
        for r in c.racks.iter().flat_map(|r| expand_range(r)) {
            if !ids.contains(r.as_str()) {
                return Err(format!("tenant {}: unknown rack {r}", c.id));
            }
            let gpus = assets.iter().find(|a| a.id == r).and_then(|a| a.attr("gpus")).unwrap_or(72.0) as u32;
            allocations.push(Allocation { asset: r, gpus });
        }
        for tr in c.trays.iter().flat_map(|r| expand_range(r)) {
            if !ids.contains(tr.as_str()) {
                return Err(format!("tenant {}: unknown tray {tr}", c.id));
            }
            let gpus = assets.iter().find(|a| a.id == tr).and_then(|a| a.attr("gpus")).unwrap_or(4.0) as u32;
            allocations.push(Allocation { asset: tr, gpus });
        }
        customers.push(Customer {
            id: c.id,
            name: c.name,
            tier: c.tier,
            rate_per_gpu_hr: c.rate_per_gpu_hr,
            sla_pct: c.sla_pct,
            allocations,
            color: c.color,
        });
    }
    let topology = Topology { version, site: site.site, assets, customers };
    // Validate: every requirement must reference a known asset, and the graph must be a DAG.
    let ix: std::collections::BTreeSet<&str> = topology.assets.iter().map(|a| a.id.as_str()).collect();
    for a in &topology.assets {
        for r in &a.requires {
            for p in &r.of {
                if !ix.contains(p.as_str()) {
                    return Err(format!("{}: requires unknown asset {p}", a.id));
                }
            }
        }
    }
    crate::engine::Graph::new(&topology)?;
    let config = TwinConfig {
        version,
        point_types: mapping.point_types,
        asset_aliases: mapping.asset_aliases,
        subject_rules: mapping.subject_rules,
        classes: classes.classes,
    };
    Ok(ProfileMsg { topology, config })
}
