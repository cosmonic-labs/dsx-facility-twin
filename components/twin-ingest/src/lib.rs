//! twin-ingest — binds telemetry to the model.
//!
//! Subscribed to DSX BMS values and metadata, GPU/network health feeds, history backfill and
//! site profiles. For each reading it resolves the subject to a twin point (topology index →
//! DSX metadata learned at runtime → subject rules), evaluates it against the class limits,
//! stores the latest value, rolls forecastable points into 5-minute KV series, and publishes a
//! `twin.v1.sev.<asset>` event when the point's condition changes.
wit_bindgen::generate!({ path: "../wit", world: "twin-brain", generate_all });

use exports::wasmcloud::nats::core_handler::Guest;
use std::collections::HashMap;
use std::sync::Mutex;
use twin_model::config::PointBinding;
use twin_model::msg::*;
use twin_model::{keys, subjects, ts, Tlp};
use wasmcloud::nats as nats;

include!("../../common/kv.rs");

struct Index {
    version: u64,
    by_subject: HashMap<String, (PointBinding, bool)>,
    asset_ix: HashMap<String, usize>,
}
static INDEX: Mutex<Option<std::sync::Arc<Index>>> = Mutex::new(None);

fn index_for(m: &Model) -> std::sync::Arc<Index> {
    if let Some(ix) = INDEX.lock().unwrap().as_ref() {
        if ix.version == m.version {
            return ix.clone();
        }
    }
    let mut by_subject = HashMap::new();
    let mut asset_ix = HashMap::new();
    for (i, a) in m.topo.assets.iter().enumerate() {
        asset_ix.insert(a.id.clone(), i);
        for p in &a.points {
            by_subject.insert(
                p.subject.clone(),
                (PointBinding { asset: a.id.clone(), class: p.class.clone(), point: p.id.clone(), unit: p.unit.clone() }, p.forecast),
            );
        }
    }
    let ix = std::sync::Arc::new(Index { version: m.version, by_subject, asset_ix });
    *INDEX.lock().unwrap() = Some(ix.clone());
    ix
}

struct Component;

impl Guest for Component {
    async fn handle_message(msg: nats::types::NatsMessage) -> Result<(), String> {
        let b = bucket().await?;
        let s = msg.subject.as_str();
        if s == subjects::PROFILE {
            return apply_profile(&b, &msg.body).await;
        }
        if s == subjects::BACKFILL {
            return backfill(&b, &msg.body).await;
        }
        let m = match model(&b).await {
            Ok(m) => m,
            Err(_) => return Ok(()), // no profile yet: nothing to bind to
        };
        let ix = index_for(&m);
        if s.starts_with("BMS.v1.PUB.Metadata.") {
            return learn_metadata(&b, &m, &ix, s, &msg.body).await;
        }
        reading(&b, &m, &ix, s, &msg.body).await
    }
}

async fn apply_profile(b: &nats::kv::Bucket, body: &[u8]) -> Result<(), String> {
    let p: ProfileMsg = serde_json::from_slice(body).map_err(|e| format!("profile: {e}"))?;
    let cur: Option<u64> = get_json(b, "profile.version").await?;
    let v = p.topology.version;
    put_json(b, keys::TOPOLOGY, &p.topology).await?;
    put_json(b, keys::CONFIG, &p.config).await?;
    put_json(b, "profile.version", &v).await?;
    set_model(Model { version: v, topo: p.topology, cfg: p.config });
    if cur != Some(v) {
        publish("twin.v1.profile.applied", serde_json::to_vec(&serde_json::json!({"version": v})).unwrap()).await?;
    }
    Ok(())
}

async fn backfill(b: &nats::kv::Bucket, body: &[u8]) -> Result<(), String> {
    let bf: Backfill = serde_json::from_slice(body).map_err(|e| format!("backfill: {e}"))?;
    let mut chunks: std::collections::BTreeMap<i64, ts::Chunk> = Default::default();
    for (i, v) in bf.values.iter().enumerate() {
        let t = bf.start_ms + i as i64 * bf.step_s * 1000;
        chunks.entry(ts::day_of(t)).or_insert_with(|| ts::Chunk::new(ts::day_of(t))).add(t, *v as f64);
    }
    for (day, c) in chunks {
        put_json(b, &keys::series(&bf.point, day), &c).await?;
    }
    Ok(())
}

/// DSX metadata: learn bindings for subjects the topology does not already know (a real site's
/// BMS announces its points; the twin maps them through `point_types` and `asset_aliases`).
async fn learn_metadata(b: &nats::kv::Bucket, m: &Model, ix: &Index, subject: &str, body: &[u8]) -> Result<(), String> {
    let value_subject = value_subject_for(subject);
    if ix.by_subject.contains_key(&value_subject) {
        return Ok(());
    }
    let md: DsxMetadata = serde_json::from_slice(body).map_err(|e| format!("metadata: {e}"))?;
    let Some(class) = m.cfg.class_for(&md.object_type, &md.point_type) else {
        return put_json(b, &format!("unmapped.{}", keys::safe(&value_subject)), &md).await;
    };
    let asset = m.cfg.asset_id(md.object_id.as_deref().unwrap_or_default()).to_string();
    let binding = PointBinding { point: format!("{asset}.{class}"), asset, class: class.to_string(), unit: md.eng_unit.unwrap_or_default() };
    put_json(b, &keys::pmap(&value_subject), &binding).await
}

async fn reading(b: &nats::kv::Bucket, m: &Model, ix: &Index, subject: &str, body: &[u8]) -> Result<(), String> {
    let (binding, forecast) = match ix.by_subject.get(subject) {
        Some((pb, f)) => (pb.clone(), *f),
        None => match get_json::<PointBinding>(b, &keys::pmap(subject)).await? {
            Some(pb) => (pb, false),
            None => match m.cfg.match_rule(subject) {
                Some(pb) => (pb, false),
                None => return Ok(()),
            },
        },
    };
    let v: DsxValue = serde_json::from_slice(body).map_err(|e| format!("value on {subject}: {e}"))?;
    let asset = ix.asset_ix.get(&binding.asset).map(|i| &m.topo.assets[*i]);
    let cond = m.cfg.evaluate(&binding.class, v.value, v.quality, asset);
    let lkv_key = keys::lkv(&binding.point);
    let prev: Option<Lkv> = get_json(b, &lkv_key).await?;
    put_json(b, &lkv_key, &Lkv { v: v.value, t: v.timestamp, q: v.quality, tlp: cond.tlp }).await?;
    if forecast && v.quality != 0 {
        let day = ts::day_of(v.timestamp);
        let key = keys::series(&binding.point, day);
        let mut c: ts::Chunk = get_json(b, &key).await?.unwrap_or_else(|| ts::Chunk::new(day));
        c.add(v.timestamp, v.value);
        put_json(b, &key, &c).await?;
    }
    let changed = match &prev {
        Some(p) => p.tlp != cond.tlp,
        None => cond.tlp != Tlp::Green,
    };
    if changed {
        let ev = SevEvent { asset: binding.asset.clone(), point: binding.point, class: binding.class, tlp: cond.tlp, text: cond.text, value: v.value, t_ms: v.timestamp };
        publish(&format!("{}.{}", subjects::SEV_PREFIX, binding.asset), serde_json::to_vec(&ev).unwrap()).await?;
    }
    Ok(())
}

export!(Component);
