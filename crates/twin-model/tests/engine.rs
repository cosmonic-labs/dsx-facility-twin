#![cfg(feature = "profile")]
use std::collections::BTreeMap;
use std::path::Path;
use twin_model::engine::{compute, TreeCtx};
use twin_model::msg::*;
use twin_model::Tlp;

fn profile() -> ProfileMsg {
    twin_model::profile::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../profiles/demo-dsx1"), 1).unwrap()
}

fn cond(c: &mut Conditions, asset: &str, class: &str, tlp: Tlp, text: &str) {
    c.entry(asset.into()).or_default().insert(
        format!("{asset}.{class}"),
        PointCond { class: class.into(), tlp, text: text.into(), value: 0.0, t_ms: 0 },
    );
}

#[test]
fn loads_and_is_green() {
    let p = profile();
    let n = p.topology.assets.len();
    let pts: usize = p.topology.assets.iter().map(|a| a.points.len()).sum();
    eprintln!("assets {n}, points {pts}, customers {}", p.topology.customers.len());
    assert!(n > 700);
    let s = compute(&p.topology, &p.config, &BTreeMap::new(), &PredictionSet::default(), 0).unwrap();
    assert!(s.assets.is_empty(), "{:?}", s.assets.keys().take(5).collect::<Vec<_>>());
    assert_eq!(s.kpis.gpus.green, s.kpis.gpus_total);
}

#[test]
fn tap_trip_is_redundancy_only_busway_both_sides_is_red() {
    let p = profile();
    let mut c = Conditions::new();
    cond(&mut c, "P1-R1-01-TAP-A", "breaker_status", Tlp::Red, "Breaker: Tripped");
    let s = compute(&p.topology, &p.config, &c, &PredictionSet::default(), 0).unwrap();
    let r = s.assets.get("P1-R1-01").expect("rack listed");
    assert_eq!(r.tlp, Tlp::Green);
    assert!(r.redundancy_lost);
    cond(&mut c, "P1-R1-01-TAP-B", "breaker_status", Tlp::Red, "Breaker: Tripped");
    let s = compute(&p.topology, &p.config, &c, &PredictionSet::default(), 0).unwrap();
    assert_eq!(s.assets["P1-R1-01"].tlp, Tlp::Red);
    assert_eq!(s.assets["P1-R1-01-T05"].tlp, Tlp::Red);
    let helios = s.customers.iter().find(|c| c.id == "helios").unwrap();
    assert_eq!(helios.tlp, Tlp::Red);
    assert_eq!(helios.gpus.red, 72);
    assert!(s.warnings.iter().any(|w| w.asset == "P1-R1-01-TAP-B" && w.customers.contains(&"helios".to_string())));
}

#[test]
fn utility_loss_is_yellow_with_eta() {
    let p = profile();
    let mut c = Conditions::new();
    for s in ["A", "B"] {
        cond(&mut c, &format!("UTIL-{s}"), "util_status", Tlp::Red, "Utility: Lost");
        cond(&mut c, &format!("GEN-{s}"), "gen_status", Tlp::Red, "Generator: FailToStart");
    }
    let mut preds = PredictionSet::default();
    for s in ["A", "B"] {
        for m in 1..=3 {
            cond(&mut c, &format!("UPS-{s}{m}"), "ups_mode", Tlp::Yellow, "UPS mode: OnBattery");
            preds.items.push(Prediction {
                asset: format!("UPS-{s}{m}"), point: format!("UPS-{s}{m}.runtime_min"), class: "runtime_min".into(),
                kind: PredictionKind::Runtime, tlp_at_eta: Tlp::Red, eta_min: 7.0, text: "7 min of battery".into(),
                current: 7.0, threshold: 0.0, history: vec![], forecast: vec![], upper: vec![], lower: vec![],
            });
        }
    }
    let s = compute(&p.topology, &p.config, &c, &preds, 0).unwrap();
    let r = &s.assets["P2-R2-03"];
    assert_eq!(r.tlp, Tlp::Yellow, "{r:?}");
    assert!(r.eta_min.is_some());
    assert!(s.kpis.customers_at_risk >= 5);
    let ctx = TreeCtx::new(&p.topology, &s).unwrap();
    let t = ctx.downstream("UPS-A1", 3, false).unwrap();
    assert!(t.size > 10);
    let up = ctx.upstream("P1-R1-01", 4).unwrap();
    assert!(up.children.len() >= 3);
}

#[test]
fn cdu_out_with_partner_degraded_is_orange() {
    let p = profile();
    let mut c = Conditions::new();
    cond(&mut c, "CDU-P1-R2-A", "cdu_status", Tlp::Red, "CDU status: Off");
    cond(&mut c, "CDU-P1-R2-B", "cdu_supply_temp", Tlp::Orange, "CDU supply 36 °C");
    let s = compute(&p.topology, &p.config, &c, &PredictionSet::default(), 0).unwrap();
    assert_eq!(s.assets["P1-R2-03"].tlp, Tlp::Orange);
    let lumen = s.customers.iter().find(|c| c.id == "lumen").unwrap();
    assert_eq!(lumen.tlp, Tlp::Orange);
    assert!(lumen.revenue_impacted_hr > 0.0);
}

#[test]
fn message_sizes_fit_nats() {
    let p = profile();
    let prof = serde_json::to_vec(&p).unwrap().len();
    let topo = serde_json::to_vec(&p.topology).unwrap().len();
    // worst case snapshot: everything out
    let mut c = Conditions::new();
    for a in &p.topology.assets {
        cond(&mut c, &a.id, "x", Tlp::Red, "Something long enough to be a realistic reason text for an asset");
    }
    let s = compute(&p.topology, &p.config, &c, &PredictionSet::default(), 0).unwrap();
    let snap = serde_json::to_vec(&s).unwrap().len();
    eprintln!("profile {prof} topology {topo} worst snapshot {snap} warnings {}", s.warnings.len());
    assert!(prof < 950_000 && snap < 950_000);
}

#[test]
fn rack_symptoms_blame_the_cdu() {
    let p = profile();
    let mut c = Conditions::new();
    cond(&mut c, "CDU-P1-R2-A", "pump_vibration", Tlp::Orange, "Pump vibration 9.3 mm/s");
    for k in 1..=8 {
        cond(&mut c, &format!("P1-R2-0{k}"), "liquid_flow", Tlp::Yellow, "Liquid flow low");
    }
    let s = compute(&p.topology, &p.config, &c, &PredictionSet::default(), 0).unwrap();
    let r = &s.assets["P1-R2-03"];
    assert_eq!(r.causes, vec!["CDU-P1-R2-A".to_string()], "{r:?}");
    let roots: Vec<&str> = s.warnings.iter().filter(|w| w.kind != WarningKind::Prediction && w.kind != WarningKind::RedundancyLost).map(|w| w.asset.as_str()).collect();
    assert_eq!(roots, vec!["CDU-P1-R2-A"], "{roots:?}");
    let w = &s.warnings[0];
    assert!(w.customers.contains(&"lumen".to_string()), "{w:?}");
}
