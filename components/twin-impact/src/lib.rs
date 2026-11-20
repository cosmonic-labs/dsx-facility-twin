//! twin-impact — petgraph in Wasm.
//!
//! Folds `twin.v1.sev.*` condition changes into the `conditions` document, then recomputes the
//! whole snapshot with the impact engine (dependency-ordered now/future passes over the site
//! graph): per-asset TLP with root causes and ETAs, per-customer GPU impact and revenue at risk,
//! and the warnings list. Also recomputes on new predictions and new profiles.
wit_bindgen::generate!({ path: "../wit", world: "twin-brain", generate_all });

use exports::wasmcloud::nats::core_handler::Guest;
use twin_model::msg::*;
use twin_model::{keys, subjects, Tlp};
use wasmcloud::nats as nats;

include!("../../common/kv.rs");

struct Component;

impl Guest for Component {
    async fn handle_message(msg: nats::types::NatsMessage) -> Result<(), String> {
        let b = bucket().await?;
        let Ok(m) = model(&b).await else { return Ok(()) };
        let mut conds: Conditions = get_json(&b, keys::CONDITIONS).await?.unwrap_or_default();
        let mut at_ms = 0;
        if msg.subject.starts_with(subjects::SEV_PREFIX) {
            let ev: SevEvent = serde_json::from_slice(&msg.body).map_err(|e| format!("sev: {e}"))?;
            at_ms = ev.t_ms;
            let entry = conds.entry(ev.asset.clone()).or_default();
            if ev.tlp == Tlp::Green {
                entry.remove(&ev.point);
            } else {
                entry.insert(ev.point.clone(), PointCond { class: ev.class, tlp: ev.tlp, text: ev.text, value: ev.value, t_ms: ev.t_ms });
            }
            if entry.is_empty() {
                conds.remove(&ev.asset);
            }
            // forget assets the current profile no longer has
            let ix = m.topo.index();
            conds.retain(|k, _| ix.contains_key(k.as_str()));
            put_json(&b, keys::CONDITIONS, &conds).await?;
        }
        let preds: PredictionSet = get_json(&b, keys::PREDICTIONS).await?.unwrap_or_default();
        if at_ms == 0 {
            at_ms = preds.at_ms;
        }
        let t0 = wall_ms();
        let mut snap = twin_model::engine::compute(&m.topo, &m.cfg, &conds, &preds, at_ms)?;
        snap.took_us = ((wall_ms() - t0).max(0) * 1000) as u64;
        put_json(&b, keys::SNAPSHOT, &snap).await?;
        publish(
            subjects::SNAPSHOT_UPDATED,
            serde_json::to_vec(&serde_json::json!({"at_ms": snap.at_ms, "warnings": snap.warnings.len(), "affected": snap.kpis.customers_affected, "at_risk": snap.kpis.customers_at_risk})).unwrap(),
        )
        .await
    }
}

export!(Component);
