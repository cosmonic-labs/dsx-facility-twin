//! twin-predict — augurs in Wasm.
//!
//! On every `twin.v1.tick.predict` it walks the forecastable points that have a degraded limit,
//! triages them (close to the limit, or trending toward it), fits MSTL (daily seasonality) with
//! an AutoETS trend on the candidates, and records any forecast that crosses the limit inside the
//! two-hour horizon. It also runs a physics-aware peer check (augurs MAD) on each row's rack
//! return temperatures, and turns UPS runtime-on-battery into an ETA.
wit_bindgen::generate!({ path: "../wit", world: "twin-brain", generate_all });

use exports::wasmcloud::nats::core_handler::Guest;
use std::collections::BTreeMap;
use twin_analytics::{forecast, peer_outliers, worth_fitting, Dir};
use twin_model::config::Direction;
use twin_model::msg::*;
use twin_model::topo::AssetKind;
use twin_model::{keys, ts, Tlp};
use wasmcloud::nats as nats;

include!("../../common/kv.rs");

const HORIZON: usize = 24; // 24 × 5 min = 2 h
const STEP_MIN: f64 = 5.0;

struct Component;

#[derive(serde::Deserialize)]
struct Tick {
    at_ms: i64,
}

async fn series(b: &nats::kv::Bucket, point: &str, at_ms: i64, days: i64) -> Result<Vec<f64>, String> {
    let d = ts::day_of(at_ms);
    let mut chunks = vec![];
    for k in (0..days).rev() {
        if let Some(c) = get_json::<ts::Chunk>(b, &keys::series(point, d - k)).await? {
            chunks.push(c);
        }
    }
    Ok(ts::assemble(&chunks, at_ms, days as usize * ts::SLOTS).1)
}

fn tail(v: &[f64], n: usize) -> Vec<f32> {
    v[v.len().saturating_sub(n)..].iter().map(|x| *x as f32).collect()
}

impl Guest for Component {
    async fn handle_message(msg: nats::types::NatsMessage) -> Result<(), String> {
        let t0 = wall_ms();
        let tick: Tick = serde_json::from_slice(&msg.body).map_err(|e| e.to_string())?;
        let b = bucket().await?;
        let Ok(m) = model(&b).await else { return Ok(()) };
        let mut items = vec![];
        let mut fitted = 0u32;
        let mut cache: BTreeMap<String, Vec<f64>> = BTreeMap::new();

        let conds: Conditions = get_json(&b, keys::CONDITIONS).await?.unwrap_or_default();
        let is_out = |id: &str| conds.get(id).map(|c| c.values().any(|x| x.tlp == Tlp::Red)).unwrap_or(false);

        // ---- forecasts against the degraded limit ----
        for (a, p) in m.topo.points() {
            if !p.forecast {
                continue;
            }
            let Some((dir, thr)) = m.cfg.degraded_threshold(&p.class, Some(a)) else { continue };
            let y = series(&b, &p.id, tick.at_ms, 3).await?;
            if a.kind == AssetKind::Rack {
                cache.insert(p.id.clone(), y.clone());
            }
            // An asset that is already out has nothing left to predict.
            if is_out(&a.id) {
                continue;
            }
            let d = if dir == Direction::High { Dir::High } else { Dir::Low };
            if !worth_fitting(&y, thr, d, HORIZON) {
                continue;
            }
            fitted += 1;
            let Ok(fc) = forecast(&y, HORIZON, thr, d) else { continue };
            let Some(step) = fc.cross_step else { continue };
            let def = &m.cfg.classes[&p.class];
            let out_thr = def.limit.as_ref().and_then(|l| l.out).map(|o| o * l_scale(def, a));
            let to_red = out_thr.map(|o| fc.point.iter().any(|v| if d == Dir::High { *v >= o } else { *v <= o })).unwrap_or(false);
            let eta = step as f64 * STEP_MIN;
            let now = *y.last().unwrap_or(&0.0);
            items.push(Prediction {
                asset: a.id.clone(),
                point: p.id.clone(),
                class: p.class.clone(),
                kind: PredictionKind::Forecast,
                tlp_at_eta: if to_red { Tlp::Red } else { Tlp::Orange },
                eta_min: eta,
                text: format!(
                    "{} {:.1} {} now; augurs forecasts {} {:.1} {} in ~{:.0} min",
                    def.label, now, p.unit, if d == Dir::High { "crossing" } else { "falling below" }, thr, p.unit, eta
                ),
                current: now,
                threshold: thr,
                history: tail(&y, 48),
                forecast: fc.point.iter().map(|x| *x as f32).collect(),
                upper: fc.upper.iter().map(|x| *x as f32).collect(),
                lower: fc.lower.iter().map(|x| *x as f32).collect(),
            });
        }

        // ---- physics-aware peer check: rack return temperature vs what power and flow explain ----
        let mut rows: BTreeMap<String, Vec<&twin_model::topo::Asset>> = BTreeMap::new();
        for a in m.topo.assets.iter().filter(|a| a.kind == AssetKind::Rack) {
            rows.entry(a.parent.clone().unwrap_or_default()).or_default().push(a);
        }
        const W: usize = 12;
        for (_, racks) in rows {
            let mut resid: Vec<Vec<f64>> = vec![];
            let mut ids = vec![];
            for r in &racks {
                let get = |c: &str| cache.get(&format!("{}.{c}", r.id)).cloned().unwrap_or_default();
                let (ret, sup, flow) = (get("liquid_return_temp"), get("liquid_supply_temp"), get("liquid_flow"));
                let pw = series(&b, &format!("{}.rack_power", r.id), tick.at_ms, 1).await?;
                if ret.len() < W || sup.len() < W || flow.len() < W || pw.len() < W {
                    continue;
                }
                let s: Vec<f64> = (0..W)
                    .map(|k| {
                        let i = |v: &Vec<f64>| v[v.len() - W + k];
                        let expect = if i(&flow) > 5.0 { i(&pw) * 0.85 * 60.0 / (i(&flow) * 4.18) } else { 0.0 };
                        (i(&ret) - i(&sup)) - expect
                    })
                    .collect();
                resid.push(s);
                ids.push(*r);
            }
            for k in peer_outliers(&resid, 0.5) {
                let r = ids[k];
                let off = resid[k].iter().sum::<f64>() / W as f64;
                if off.abs() < 1.5 {
                    continue; // physically meaningless spread
                }
                items.push(Prediction {
                    asset: r.id.clone(),
                    point: format!("{}.liquid_return_temp", r.id),
                    class: "liquid_return_temp".into(),
                    kind: PredictionKind::PeerOutlier,
                    tlp_at_eta: Tlp::Yellow,
                    eta_min: 0.0,
                    text: format!(
                        "Return temperature reads {off:+.1} °C away from what this rack's power and flow explain; its row peers agree with physics. Likely a sensor, not the rack."
                    ),
                    current: cache.get(&format!("{}.liquid_return_temp", r.id)).and_then(|v| v.last().copied()).unwrap_or(0.0),
                    threshold: 0.0,
                    history: resid[k].iter().map(|x| *x as f32).collect(),
                    forecast: vec![],
                    upper: vec![],
                    lower: vec![],
                });
            }
        }

        // ---- stored energy: UPS runtime on battery ----
        for (a, p) in m.topo.points() {
            if !m.cfg.classes.get(&p.class).map(|c| c.eta_minutes).unwrap_or(false) {
                continue;
            }
            // "OnBattery" from the class's state table (an Alarm is also yellow but is not a countdown).
            let on_batt_v = m.cfg.classes.get("ups_mode").and_then(|c| c.states.iter().find(|s| s.text.eq_ignore_ascii_case("OnBattery")).map(|s| s.value as f64)).unwrap_or(1.0);
            let on_battery = conds.get(&a.id).map(|c| c.values().any(|x| x.class == "ups_mode" && (x.value - on_batt_v).abs() < 0.5)).unwrap_or(false);
            if !on_battery {
                continue;
            }
            if let Some(l) = get_json::<Lkv>(&b, &keys::lkv(&p.id)).await? {
                if l.v < 180.0 {
                    items.push(Prediction {
                        asset: a.id.clone(),
                        point: p.id.clone(),
                        class: p.class.clone(),
                        kind: PredictionKind::Runtime,
                        tlp_at_eta: Tlp::Red,
                        eta_min: l.v,
                        text: format!("On battery: about {:.0} min of runtime left at the present load", l.v),
                        current: l.v,
                        threshold: 0.0,
                        history: vec![],
                        forecast: vec![],
                        upper: vec![],
                        lower: vec![],
                    });
                }
            }
        }

        items.sort_by(|x, y| x.eta_min.partial_cmp(&y.eta_min).unwrap_or(std::cmp::Ordering::Equal));
        let set = PredictionSet { at_ms: tick.at_ms, fitted, took_ms: (wall_ms() - t0).max(0) as u64, horizon_min: HORIZON as f64 * STEP_MIN, items };
        put_json(&b, keys::PREDICTIONS, &set).await?;
        publish(twin_model::subjects::PRED, serde_json::to_vec(&serde_json::json!({"at_ms": set.at_ms, "n": set.items.len(), "fitted": fitted, "took_ms": set.took_ms})).unwrap()).await
    }
}

fn l_scale(def: &twin_model::config::ClassDef, a: &twin_model::topo::Asset) -> f64 {
    def.limit.as_ref().and_then(|l| l.relative_to.as_deref()).and_then(|k| a.attr(k)).unwrap_or(1.0)
}

export!(Component);
