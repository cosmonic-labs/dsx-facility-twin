//! twin-sim: the demo harness. Simulates a DSX AI factory hall, publishes DSX BMS telemetry
//! to NATS, and exposes an HTTP API to run failure scenarios.
//!
//! Environment:
//!   NATS_URL      nats://nexus.cosmonic-system.svc.cluster.local:4222
//!   PROFILE_DIR   /profiles/demo-dsx1
//!   HTTP_ADDR     0.0.0.0:8090
//!   SPEED         30     (simulated seconds per real second)
//!   TICK_S        10     (real seconds between telemetry publishes)
//!   BACKFILL_DAYS 2
mod faults;
mod physics;
mod scenarios;

use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use faults::Faults;
use physics::World;
use scenarios::{apply, catalog, Run, ScenarioDef};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use twin_model::msg::{Backfill, DsxMetadata, DsxValue, ProfileMsg, StateText};
use twin_model::topo::AssetKind;
use twin_model::{subjects, ts};

struct Sim {
    profile: ProfileMsg,
    world: World,
    speed: f64,
    paused: bool,
    runs: Vec<Run>,
    next_id: u64,
    catalog: Vec<ScenarioDef>,
    last_tray: BTreeMap<String, f64>,
    published: u64,
    faults: Faults,
}

type Shared = Arc<Mutex<Sim>>;

fn env(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

impl Sim {
    fn faults(&self) -> Faults {
        let mut f = Faults::default();
        for r in self.runs.iter().filter(|r| !r.done) {
            if let Some(def) = self.catalog.iter().find(|d| d.id == r.scenario) {
                apply(r, def, self.world.t_ms, &self.profile.topology, &mut f);
            }
        }
        f
    }
    fn auto_advance(&mut self) {
        let now = self.world.t_ms;
        for r in self.runs.iter_mut().filter(|r| !r.done) {
            let Some(def) = self.catalog.iter().find(|d| d.id == r.scenario) else { continue };
            let last = r.stage + 1 == def.stages.len();
            if r.progress(def, now) >= 1.0 {
                if last {
                    r.done = true;
                    r.ended_ms = Some(now);
                } else if r.auto {
                    r.stage += 1;
                    r.stage_started_ms = now;
                }
            }
        }
    }
}

fn dsx_types(p: &ProfileMsg, class: &str) -> Option<(String, String)> {
    let d = p.config.classes.get(class)?;
    let (ot, pt) = d.dsx.split_once('/')?;
    Some((ot.to_string(), pt.to_string()))
}

async fn publish_metadata(nc: &async_nats::Client, p: &ProfileMsg) -> Result<usize> {
    let mut n = 0;
    for (a, pt) in p.topology.points() {
        let Some((ot, ptype)) = dsx_types(p, &pt.class) else { continue };
        let def = &p.config.classes[&pt.class];
        let md = DsxMetadata {
            object_type: ot,
            point_type: ptype,
            eng_unit: (!def.unit.is_empty()).then(|| def.unit.clone()),
            object_name: Some(a.name.clone()),
            object_id: Some(a.id.clone()),
            serves_id: if a.kind == AssetKind::TapOff || a.kind == AssetKind::Busway {
                p.topology.assets.iter().filter(|x| x.requires.iter().any(|r| r.of.contains(&a.id))).map(|x| x.id.clone()).collect()
            } else {
                vec![]
            },
            rack_location_id: (a.kind == AssetKind::Rack).then(|| a.id.clone()),
            state_text: (!def.states.is_empty()).then(|| def.states.iter().map(|s| StateText { value: s.value, text: s.text.clone() }).collect()),
        };
        let subj = pt.subject.replacen(".Value.", ".Metadata.", 1);
        nc.publish(subj, serde_json::to_vec(&md)?.into()).await?;
        n += 1;
    }
    nc.flush().await?;
    Ok(n)
}

async fn publish_profile(nc: &async_nats::Client, p: &ProfileMsg) -> Result<usize> {
    let body = serde_json::to_vec(p)?;
    let n = body.len();
    nc.publish(subjects::PROFILE, body.into()).await?;
    nc.flush().await?;
    Ok(n)
}

/// History import: replay the plant forward for N days at 5-minute steps from a clean state
/// and hand the forecastable series to the twin, as a site's historian would on day one.
async fn backfill(nc: &async_nats::Client, p: &ProfileMsg, end_ms: i64, days: i64) -> Result<usize> {
    let start = end_ms - days * 86_400_000;
    let start = start - start.rem_euclid(ts::STEP_S * 1000);
    let mut w = World::new(&p.topology, start, 7);
    let fpoints: Vec<String> = p.topology.points().filter(|(_, pt)| pt.forecast).map(|(_, pt)| pt.id.clone()).collect();
    let steps = (days * 86_400 / ts::STEP_S) as usize;
    let mut series: BTreeMap<String, Vec<f32>> = fpoints.iter().map(|k| (k.clone(), Vec::with_capacity(steps))).collect();
    let f = Faults::default();
    for _ in 0..steps {
        w.advance(ts::STEP_S as f64, &f);
        for k in &fpoints {
            let v = w.readings.get(k).map(|x| x.0).unwrap_or(0.0);
            series.get_mut(k).unwrap().push(v as f32);
        }
    }
    let mut n = 0;
    for (point, values) in series {
        let b = Backfill { point, start_ms: start + ts::STEP_S * 1000, step_s: ts::STEP_S, values };
        nc.publish(subjects::BACKFILL, serde_json::to_vec(&b)?.into()).await?;
        n += 1;
        if n % 50 == 0 {
            nc.flush().await?;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    nc.flush().await?;
    Ok(n)
}

async fn publish_tick(nc: &async_nats::Client, sim: &Shared) -> Result<()> {
    let (msgs, t_ms) = {
        let mut s = sim.lock().await;
        let mut out: Vec<(String, Vec<u8>)> = vec![];
        let t = s.world.t_ms;
        let readings = s.world.readings.clone();
        let points: Vec<(String, String, AssetKind)> = s
            .profile
            .topology
            .points()
            .map(|(a, p)| (p.id.clone(), p.subject.clone(), a.kind))
            .collect();
        for (id, subj, kind) in points {
            let Some(&(v, q)) = readings.get(&id) else { continue };
            if kind == AssetKind::ComputeTray {
                let prev = s.last_tray.get(&id).copied().unwrap_or(0.0);
                if prev == v && (v == 0.0 || s.published % 6 != 0) {
                    continue;
                }
                s.last_tray.insert(id.clone(), v);
            }
            out.push((subj, serde_json::to_vec(&DsxValue { value: (v * 1000.0).round() / 1000.0, timestamp: t, quality: q }).unwrap()));
        }
        s.published += 1;
        (out, t)
    };
    for (subj, body) in msgs {
        nc.publish(subj, body.into()).await?;
    }
    nc.flush().await?;
    let _ = t_ms;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let nats_url = env("NATS_URL", "nats://127.0.0.1:4222");
    let profile_dir = env("PROFILE_DIR", "profiles/demo-dsx1");
    let addr = env("HTTP_ADDR", "0.0.0.0:8090");
    let speed: f64 = env("SPEED", "30").parse()?;
    let tick_s: f64 = env("TICK_S", "10").parse()?;
    let days: i64 = env("BACKFILL_DAYS", "2").parse()?;

    let version = now_ms() as u64;
    let profile = twin_model::profile::load_dir(std::path::Path::new(&profile_dir), version).map_err(anyhow::Error::msg)?;
    eprintln!(
        "profile {}: {} assets, {} points, {} customers",
        profile.topology.site.name,
        profile.topology.assets.len(),
        profile.topology.points().count(),
        profile.topology.customers.len()
    );
    let start = now_ms();
    let world = World::new(&profile.topology, start, 42);
    let sim: Shared = Arc::new(Mutex::new(Sim {
        profile: profile.clone(),
        world,
        speed,
        paused: false,
        runs: vec![],
        next_id: 1,
        catalog: catalog(),
        last_tray: BTreeMap::new(),
        published: 0,
        faults: Faults::default(),
    }));

    let nc = loop {
        match async_nats::connect(&nats_url).await {
            Ok(c) => break c,
            Err(e) => {
                eprintln!("nats connect {nats_url}: {e}; retrying");
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    };
    eprintln!("connected to {nats_url}");

    // Bootstrap: profile, metadata, history.
    {
        let nc = nc.clone();
        let p = profile.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match publish_profile(&nc, &p).await {
                Ok(n) => eprintln!("published profile ({n} bytes)"),
                Err(e) => eprintln!("profile publish: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
            match publish_metadata(&nc, &p).await {
                Ok(n) => eprintln!("published metadata for {n} points"),
                Err(e) => eprintln!("metadata: {e}"),
            }
            match backfill(&nc, &p, start, days).await {
                Ok(n) => eprintln!("backfilled {n} series ({days} days)"),
                Err(e) => eprintln!("backfill: {e}"),
            }
        });
    }

    // Main loop.
    {
        let nc = nc.clone();
        let sim = sim.clone();
        let p = profile.clone();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(Duration::from_secs_f64(tick_s));
            let mut n: u64 = 0;
            loop {
                iv.tick().await;
                n += 1;
                {
                    let mut s = sim.lock().await;
                    if !s.paused {
                        s.auto_advance();
                        let f = s.faults();
                        let dt = tick_s * s.speed;
                        s.world.advance(dt, &f);
                        s.faults = f;
                    }
                }
                if let Err(e) = publish_tick(&nc, &sim).await {
                    eprintln!("publish: {e}");
                }
                // augurs re-fit cadence: every 3 ticks.
                if n % 3 == 0 {
                    let t = sim.lock().await.world.t_ms;
                    let _ = nc.publish(subjects::TICK_PREDICT, json!({"at_ms": t}).to_string().into()).await;
                }
                if n % 30 == 0 {
                    let _ = publish_metadata(&nc, &p).await;
                }
                if n % 60 == 0 {
                    let _ = publish_profile(&nc, &p).await;
                }
            }
        });
    }

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/clock", get(get_clock).post(set_clock))
        .route("/api/scenarios", get(get_scenarios))
        .route("/api/runs", get(get_runs).post(start_run))
        .route("/api/runs/{id}/next", post(next_stage))
        .route("/api/runs/{id}/restore", post(restore_run))
        .route("/api/runs/{id}/auto", post(set_auto))
        .route("/api/runs/{id}/cancel", post(cancel_run))
        .route("/api/reset", post(reset_all))
        .route("/api/truth", get(get_truth))
        .route("/api/readings", get(get_readings))
        .route("/api/profile/publish", post(republish))
        .with_state((sim.clone(), nc.clone()))
        .layer(tower_http::cors::CorsLayer::permissive());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("scenario API on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

type St = State<(Shared, async_nats::Client)>;

fn run_view(s: &Sim, r: &Run) -> serde_json::Value {
    let def = s.catalog.iter().find(|d| d.id == r.scenario);
    let now = s.world.t_ms;
    json!({
        "id": r.id, "scenario": r.scenario, "name": def.map(|d| d.name), "domain": def.map(|d| d.domain),
        "target": r.target, "target_name": r.target_name, "stage": r.stage, "auto": r.auto, "done": r.done,
        "stage_started_ms": r.stage_started_ms, "started_ms": r.started_ms, "ended_ms": r.ended_ms,
        "progress": def.map(|d| r.progress(d, now)).unwrap_or(1.0),
        "elapsed_min": (now - r.stage_started_ms) as f64 / 60000.0,
        "stages": def.map(|d| &d.stages),
    })
}

async fn get_clock(State((sim, _)): St) -> impl IntoResponse {
    let s = sim.lock().await;
    Json(json!({"sim_ms": s.world.t_ms, "speed": s.speed, "paused": s.paused}))
}

#[derive(Deserialize)]
struct ClockReq {
    speed: Option<f64>,
    paused: Option<bool>,
}

async fn set_clock(State((sim, _)): St, Json(req): Json<ClockReq>) -> impl IntoResponse {
    let mut s = sim.lock().await;
    if let Some(v) = req.speed {
        s.speed = v.clamp(1.0, 240.0);
    }
    if let Some(p) = req.paused {
        s.paused = p;
    }
    Json(json!({"sim_ms": s.world.t_ms, "speed": s.speed, "paused": s.paused}))
}

async fn get_scenarios(State((sim, _)): St) -> impl IntoResponse {
    let s = sim.lock().await;
    let topo = &s.profile.topology;
    let out: Vec<serde_json::Value> = s
        .catalog
        .iter()
        .map(|d| {
            let candidates: Vec<serde_json::Value> = topo
                .assets
                .iter()
                .filter(|a| d.target_kinds.contains(&a.kind))
                .map(|a| json!({"id": a.id, "name": a.name}))
                .collect();
            json!({"id": d.id, "name": d.name, "domain": d.domain, "summary": d.summary,
                   "default_target": d.default_target, "stages": d.stages, "candidates": candidates})
        })
        .collect();
    Json(out)
}

async fn get_runs(State((sim, _)): St) -> impl IntoResponse {
    let s = sim.lock().await;
    let v: Vec<serde_json::Value> = s.runs.iter().rev().take(30).map(|r| run_view(&s, r)).collect();
    Json(v)
}

#[derive(Deserialize)]
struct StartReq {
    scenario: String,
    target: Option<String>,
    auto: Option<bool>,
}

async fn start_run(State((sim, _)): St, Json(req): Json<StartReq>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut s = sim.lock().await;
    let def = s.catalog.iter().find(|d| d.id == req.scenario).cloned().ok_or((StatusCode::NOT_FOUND, "unknown scenario".into()))?;
    let target = req.target.unwrap_or_else(|| def.default_target.to_string());
    let asset = s.profile.topology.asset(&target).cloned().ok_or((StatusCode::BAD_REQUEST, format!("unknown target {target}")))?;
    if !def.target_kinds.contains(&asset.kind) {
        return Err((StatusCode::BAD_REQUEST, format!("{target} is a {:?}; this scenario targets {:?}", asset.kind, def.target_kinds)));
    }
    let now = s.world.t_ms;
    let id = s.next_id;
    s.next_id += 1;
    let run = Run { id, scenario: def.id.into(), target, target_name: asset.name, stage: 0, stage_started_ms: now, started_ms: now, auto: req.auto.unwrap_or(false), done: false, ended_ms: None };
    s.runs.push(run.clone());
    Ok(Json(run_view(&s, &run)))
}

async fn with_run(sim: &Shared, id: u64, f: impl FnOnce(&mut Run, &ScenarioDef, i64)) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut s = sim.lock().await;
    let now = s.world.t_ms;
    let catalog = s.catalog.clone();
    let r = s.runs.iter_mut().find(|r| r.id == id).ok_or((StatusCode::NOT_FOUND, "no such run".into()))?;
    let def = catalog.iter().find(|d| d.id == r.scenario).unwrap();
    f(r, def, now);
    let r = r.clone();
    Ok(Json(run_view(&s, &r)))
}

async fn next_stage(State((sim, _)): St, Path(id): Path<u64>) -> impl IntoResponse {
    with_run(&sim, id, |r, d, now| {
        if r.done {
            return;
        }
        if r.stage + 1 < d.stages.len() {
            r.stage += 1;
            r.stage_started_ms = now;
        } else {
            r.done = true;
            r.ended_ms = Some(now);
        }
    })
    .await
}

async fn restore_run(State((sim, _)): St, Path(id): Path<u64>) -> impl IntoResponse {
    with_run(&sim, id, |r, d, now| {
        if !r.done {
            r.stage = d.stages.len() - 1;
            r.stage_started_ms = now;
        }
    })
    .await
}

#[derive(Deserialize)]
struct AutoReq {
    auto: bool,
}

async fn set_auto(State((sim, _)): St, Path(id): Path<u64>, Json(req): Json<AutoReq>) -> impl IntoResponse {
    with_run(&sim, id, |r, _, _| r.auto = req.auto).await
}

async fn cancel_run(State((sim, _)): St, Path(id): Path<u64>) -> impl IntoResponse {
    with_run(&sim, id, |r, _, now| {
        r.done = true;
        r.ended_ms = Some(now);
    })
    .await
}

async fn reset_all(State((sim, _)): St) -> impl IntoResponse {
    let mut s = sim.lock().await;
    let now = s.world.t_ms;
    for r in s.runs.iter_mut().filter(|r| !r.done) {
        r.done = true;
        r.ended_ms = Some(now);
    }
    Json(json!({"ok": true}))
}

/// Ground truth for QA: what the simulator actually broke, versus what the twin detects.
async fn get_truth(State((sim, _)): St) -> impl IntoResponse {
    let s = sim.lock().await;
    let active: Vec<serde_json::Value> = s.runs.iter().filter(|r| !r.done).map(|r| run_view(&s, r)).collect();
    Json(json!({"sim_ms": s.world.t_ms, "active": active, "faults": format!("{:?}", s.faults)}))
}

#[derive(Deserialize)]
struct ReadQ {
    asset: Option<String>,
}

async fn get_readings(State((sim, _)): St, Query(q): Query<ReadQ>) -> impl IntoResponse {
    let s = sim.lock().await;
    let m: BTreeMap<&String, &(f64, i64)> = s
        .world
        .readings
        .iter()
        .filter(|(k, _)| q.asset.as_ref().map(|a| k.starts_with(&format!("{a}."))).unwrap_or(true))
        .collect();
    Json(json!(m))
}

async fn republish(State((sim, nc)): St) -> impl IntoResponse {
    let p = sim.lock().await.profile.clone();
    let a = publish_profile(&nc, &p).await.map_err(|e| e.to_string());
    let b = publish_metadata(&nc, &p).await.map_err(|e| e.to_string());
    let t = sim.lock().await.world.t_ms;
    let c = backfill(&nc, &p, t, 2).await.map_err(|e| e.to_string());
    Json(json!({"profile_bytes": a.ok(), "metadata_points": b.ok(), "backfill_series": c.ok()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use twin_model::msg::{Conditions, PointCond, PredictionSet};
    use twin_model::Tlp;

    pub fn profile() -> ProfileMsg {
        twin_model::profile::load_dir(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../profiles/demo-dsx1")), 1).unwrap()
    }

    fn conds(p: &ProfileMsg, w: &World) -> Conditions {
        let mut c = Conditions::new();
        for (a, pt) in p.topology.points() {
            if let Some(&(v, q)) = w.readings.get(&pt.id) {
                let r = p.config.evaluate(&pt.class, v, q, Some(a));
                if r.tlp != Tlp::Green {
                    c.entry(a.id.clone()).or_default().insert(pt.id.clone(), PointCond { class: pt.class.clone(), tlp: r.tlp, text: r.text, value: v, t_ms: w.t_ms });
                }
            }
        }
        c
    }

    #[test]
    fn normal_day_is_green() {
        let p = profile();
        let mut w = World::new(&p.topology, 1_731_300_000_000, 1);
        let f = Faults::default();
        let mut worst: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        for step in 0..288 {
            w.advance(300.0, &f);
            for (k, (v, _)) in &w.readings {
                let class = k.split('.').nth(1).unwrap().to_string();
                let e = worst.entry(class).or_insert((f64::MAX, f64::MIN));
                e.0 = e.0.min(*v);
                e.1 = e.1.max(*v);
            }
            let c = conds(&p, &w);
            if !c.is_empty() && step > 3 {
                let first: Vec<_> = c.iter().take(4).map(|(a, m)| format!("{a}: {:?}", m.values().map(|x| &x.text).collect::<Vec<_>>())).collect();
                panic!("step {step} not green: {first:?}");
            }
        }
        for (k, (lo, hi)) in &worst {
            eprintln!("{k:22} {lo:10.2} .. {hi:10.2}");
        }
    }

    #[test]
    fn scenarios_walkthrough() {
        let p = profile();
        let cat = catalog();
        for def in &cat {
            let mut w = World::new(&p.topology, 1_731_330_000_000, 3); // ~ 13:00 local
            let mut run = Run { id: 1, scenario: def.id.into(), target: def.default_target.into(), target_name: String::new(), stage: 0, stage_started_ms: w.t_ms, started_ms: w.t_ms, auto: true, done: false, ended_ms: None };
            eprintln!("\n=== {} ({})", def.name, def.default_target);
            for (si, stage) in def.stages.iter().enumerate() {
                run.stage = si;
                run.stage_started_ms = w.t_ms;
                let mins = stage.minutes as usize;
                for _ in 0..mins {
                    let mut f = Faults::default();
                    apply(&run, def, w.t_ms, &p.topology, &mut f);
                    w.advance(60.0, &f);
                }
                let c = conds(&p, &w);
                let s = twin_model::engine::compute(&p.topology, &p.config, &c, &PredictionSet::default(), w.t_ms).unwrap();
                let by: Vec<String> = s.kpis.assets_by_tlp.iter().map(|(k, v)| format!("{k}:{v}")).collect();
                let roots: Vec<String> = c.iter().take(5).map(|(a, m)| format!("{a}[{}]", m.values().map(|x| format!("{:?} {}", x.tlp, x.text)).collect::<Vec<_>>().join("; "))).collect();
                let cust: Vec<String> = s.customers.iter().filter(|c| c.tlp != Tlp::Green).map(|c| format!("{}:{:?}", c.id, c.tlp)).collect();
                eprintln!("  [{}] {:28} {} | roots: {} | cust: {}", stage.kind, stage.name, by.join(" "), roots.join(" || "), cust.join(" "));
            }
        }
    }
}
#[cfg(test)]
mod back_to_back {
    use super::tests::profile;
    use super::*;

    /// Scenarios run one after another on the same world (as in the lab). A generator that ran
    /// for the first outage must go back to standby, so the next one can fail to start.
    #[test]
    fn generator_returns_to_standby_between_outages() {
        let p = profile();
        let cat = catalog();
        let mut w = World::new(&p.topology, 1_731_330_000_000, 3);
        for id in ["utility-outage", "gen-fail-to-start"] {
            let def = cat.iter().find(|d| d.id == id).unwrap();
            let mut run = Run { id: 1, scenario: def.id.into(), target: def.default_target.into(), target_name: String::new(), stage: 0, stage_started_ms: w.t_ms, started_ms: w.t_ms, auto: true, done: false, ended_ms: None };
            for (si, stage) in def.stages.iter().enumerate() {
                run.stage = si;
                run.stage_started_ms = w.t_ms;
                for _ in 0..((stage.minutes / 20.0).ceil() as usize + 1) {
                    let mut f = Faults::default();
                    apply(&run, def, w.t_ms, &p.topology, &mut f);
                    w.advance(1200.0, &f); // 10 s ticks at 120x
                }
                if id == "gen-fail-to-start" && si == 0 {
                    assert_eq!(w.gens["A"].0, physics::Gen::FailToStart);
                }
            }
            for _ in 0..2 {
                w.advance(1200.0, &Faults::default());
            }
            assert!(w.gens.values().all(|g| g.0 == physics::Gen::Standby), "after {id}: {:?}", w.gens);
        }
    }
}
