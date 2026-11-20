//! The impact engine: turns point conditions and forecasts into a colored dependency graph.
//!
//! Two passes over the topology in dependency order (petgraph toposort):
//!
//! * **now** — each asset's own condition, then each requirement group: fewer than `need`
//!   available providers → Red (or Yellow if the group is *bridged* by stored energy);
//!   fewer than `need` healthy → Orange; exactly `need` healthy of a larger set → redundancy lost.
//! * **future** — the same, but every forecast is assumed to come true and every leading
//!   indicator (Yellow) is assumed to degrade. Anything that is fine now but not in the future
//!   is **Yellow: on trend to be impacted**, with the ETA at which its group would fail.
use crate::config::TwinConfig;
use crate::msg::*;
use crate::topo::{Asset, AssetKind, Domain, Topology};
use crate::Tlp;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Bfs, EdgeRef, Reversed};
use petgraph::Direction as PgDir;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_CAUSES: usize = 8;

/// The dependency graph: edges point provider → consumer.
pub struct Graph<'a> {
    pub topo: &'a Topology,
    pub g: DiGraph<usize, ()>,
    pub idx: BTreeMap<&'a str, NodeIndex>,
    pub order: Vec<NodeIndex>,
}

impl<'a> Graph<'a> {
    pub fn new(topo: &'a Topology) -> Result<Self, String> {
        let mut g = DiGraph::<usize, ()>::with_capacity(topo.assets.len(), topo.assets.len() * 2);
        let mut idx = BTreeMap::new();
        for (i, a) in topo.assets.iter().enumerate() {
            idx.insert(a.id.as_str(), g.add_node(i));
        }
        for a in &topo.assets {
            let c = idx[a.id.as_str()];
            for r in &a.requires {
                for p in &r.of {
                    if let Some(&pi) = idx.get(p.as_str()) {
                        g.update_edge(pi, c, ());
                    }
                }
            }
        }
        let order = toposort(&g, None).map_err(|c| {
            format!("dependency cycle at {}", topo.assets[g[c.node_id()]].id)
        })?;
        Ok(Graph { topo, g, idx, order })
    }
    pub fn asset(&self, n: NodeIndex) -> &'a Asset {
        &self.topo.assets[self.g[n]]
    }
    /// Everything that (transitively) depends on `id`.
    pub fn downstream(&self, id: &str) -> Vec<&'a Asset> {
        let Some(&s) = self.idx.get(id) else { return vec![] };
        let mut bfs = Bfs::new(&self.g, s);
        let mut out = vec![];
        while let Some(n) = bfs.next(&self.g) {
            if n != s {
                out.push(self.asset(n));
            }
        }
        out
    }
    /// Everything `id` (transitively) depends on.
    pub fn upstream(&self, id: &str) -> Vec<&'a Asset> {
        let Some(&s) = self.idx.get(id) else { return vec![] };
        let rev = Reversed(&self.g);
        let mut bfs = Bfs::new(rev, s);
        let mut out = vec![];
        while let Some(n) = bfs.next(rev) {
            if n != s {
                out.push(self.asset(n));
            }
        }
        out
    }
}

#[derive(Clone, Default)]
struct Eval {
    tlp: Tlp,
    reason: String,
    causes: BTreeSet<String>,
    /// Minutes until this asset reaches `tlp` (future pass) or runs out (bridged).
    eta: Option<f64>,
    redundancy_lost: bool,
}

fn worst_point(conds: Option<&BTreeMap<String, PointCond>>) -> (Tlp, String) {
    let mut best = (Tlp::Green, String::new());
    if let Some(m) = conds {
        for c in m.values() {
            if c.tlp > best.0 {
                best = (c.tlp, c.text.clone());
            }
        }
    }
    best
}

fn names(topo: &BTreeMap<&str, &Asset>, ids: &[&str]) -> String {
    let v: Vec<&str> = ids.iter().map(|i| topo.get(i).map(|a| a.name.as_str()).unwrap_or(i)).collect();
    match v.len() {
        0 => String::new(),
        1..=3 => v.join(", "),
        n => format!("{}, {} and {} more", v[0], v[1], n - 2),
    }
}

fn fmt_eta(m: f64) -> String {
    if m < 1.0 {
        "under a minute".into()
    } else if m < 90.0 {
        format!("~{:.0} min", m)
    } else {
        format!("~{:.1} h", m / 60.0)
    }
}

/// One pass. `future`: escalate Yellow own-states and apply predictions.
fn pass(
    gr: &Graph,
    conds: &Conditions,
    preds: &BTreeMap<String, (Tlp, f64, String)>,
    future: bool,
) -> Vec<Eval> {
    let ix = gr.topo.index();
    let mut ev: Vec<Eval> = vec![Eval::default(); gr.topo.assets.len()];
    for &n in &gr.order {
        let i = gr.g[n];
        let a = &gr.topo.assets[i];
        let (mut own, own_text) = worst_point(conds.get(&a.id));
        let mut e = Eval { tlp: own, reason: own_text.clone(), ..Default::default() };
        if own != Tlp::Green && own != Tlp::Unknown {
            e.causes.insert(a.id.clone());
        }
        if future {
            if let Some((pt, eta, text)) = preds.get(&a.id) {
                if *pt > own {
                    own = *pt;
                    e.tlp = *pt;
                    e.reason = text.clone();
                    e.eta = Some(*eta);
                    e.causes.insert(a.id.clone());
                }
            }
            if own == Tlp::Yellow {
                // A leading indicator with no forecast: assume it degrades.
                e.tlp = Tlp::Orange;
                if e.eta.is_none() {
                    e.eta = preds.get(&a.id).map(|p| p.1);
                }
            }
        } else if own == Tlp::Unknown {
            e.reason = own_text;
        }
        for r in &a.requires {
            let provs: Vec<(&str, &Eval)> = r
                .of
                .iter()
                .filter_map(|p| gr.idx.get(p.as_str()).map(|&pn| (p.as_str(), &ev[gr.g[pn]])))
                .collect();
            if provs.is_empty() {
                continue;
            }
            let need = r.need as usize;
            let avail = provs.iter().filter(|(_, s)| s.tlp.available()).count();
            let healthy = provs.iter().filter(|(_, s)| s.tlp.healthy()).count();
            let bad: Vec<&str> = provs.iter().filter(|(_, s)| !s.tlp.healthy()).map(|(id, _)| *id).collect();
            let (group, text) = if avail < need {
                let out: Vec<&str> = provs.iter().filter(|(_, s)| !s.tlp.available()).map(|(id, _)| *id).collect();
                if r.partial && avail > 0 {
                    (Tlp::Orange, format!("{} below need: {} of {} available ({} out)", cap(&r.label), avail, need, names(&ix, &out)))
                } else if r.bridged && !future {
                    (Tlp::Yellow, format!("Riding through: {} lost ({}), limited time", r.label, names(&ix, &out)))
                } else {
                    (Tlp::Red, format!("Lost {}: {} out", r.label, names(&ix, &out)))
                }
            } else if healthy < need {
                (Tlp::Orange, format!("{} limited: {} degraded", cap(&r.label), names(&ix, &bad)))
            } else {
                if healthy == need && provs.len() > need && !bad.is_empty() {
                    e.redundancy_lost = true;
                    if e.reason.is_empty() {
                        e.reason = format!("Redundancy lost on {}: {} down", r.label, names(&ix, &bad));
                    }
                }
                (Tlp::Green, String::new())
            };
            if group != Tlp::Green {
                for (id, s) in provs.iter().filter(|(_, s)| !s.tlp.healthy()) {
                    if s.causes.is_empty() {
                        e.causes.insert(id.to_string());
                    } else {
                        e.causes.extend(s.causes.iter().cloned());
                    }
                }
                // ETA: the group fails when enough currently-healthy providers have failed.
                let mut etas: Vec<f64> = provs.iter().filter_map(|(_, s)| s.eta).collect();
                etas.sort_by(|x, y| x.partial_cmp(y).unwrap());
                let k = healthy.saturating_sub(need);
                let geta = etas.get(k.min(etas.len().saturating_sub(1))).copied();
                if group > e.tlp {
                    e.tlp = group;
                    e.reason = text;
                    e.eta = geta;
                } else if group == e.tlp && e.eta.is_none() {
                    e.eta = geta;
                }
            }
        }
        // Symptom or cause? If this asset's own condition is off AND something it depends on is
        // off too, the upstream trouble explains it (a rack's low flow is the CDU's pump): blame
        // upstream. Only assets with no troubled provider are root causes.
        if own != Tlp::Green && own != Tlp::Unknown {
            // ...but only when the upstream trouble is at least as bad: a rack that is leaking
            // (red) is its own root even though its CDU's reservoir (yellow) is dropping.
            let mut up: BTreeSet<String> = BTreeSet::new();
            for r in &a.requires {
                for p in &r.of {
                    if let Some(&pn) = gr.idx.get(p.as_str()) {
                        let s = &ev[gr.g[pn]];
                        if s.tlp != Tlp::Green && s.tlp != Tlp::Unknown && s.tlp >= own {
                            if s.causes.is_empty() {
                                up.insert(p.clone());
                            } else {
                                up.extend(s.causes.iter().cloned());
                            }
                        }
                    }
                }
            }
            if !up.is_empty() {
                e.causes.remove(&a.id);
                e.causes.extend(up);
            }
        }
        while e.causes.len() > MAX_CAUSES {
            let last = e.causes.iter().next_back().cloned().unwrap();
            e.causes.remove(&last);
        }
        ev[i] = e;
    }
    ev
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Per-asset forecast summary: worst predicted state, earliest ETA, text.
fn prediction_index(preds: &PredictionSet) -> BTreeMap<String, (Tlp, f64, String)> {
    let mut m: BTreeMap<String, (Tlp, f64, String)> = BTreeMap::new();
    // Peer outliers are data-quality signals: they warn, they do not color the graph.
    for p in preds.items.iter().filter(|p| p.kind != PredictionKind::PeerOutlier) {
        let e = m.entry(p.asset.clone()).or_insert((p.tlp_at_eta, p.eta_min, p.text.clone()));
        if p.tlp_at_eta > e.0 || (p.tlp_at_eta == e.0 && p.eta_min < e.1) {
            *e = (p.tlp_at_eta, p.eta_min, p.text.clone());
        }
    }
    m
}

/// Recompute the whole snapshot.
pub fn compute(topo: &Topology, _cfg: &TwinConfig, conds: &Conditions, preds: &PredictionSet, now_ms: i64) -> Result<Snapshot, String> {
    let gr = Graph::new(topo)?;
    let pidx = prediction_index(preds);
    let now = pass(&gr, conds, &pidx, false);
    let fut = pass(&gr, conds, &pidx, true);
    let ix = topo.index();

    let mut assets = BTreeMap::new();
    let mut states: Vec<AssetState> = Vec::with_capacity(topo.assets.len());
    for (i, a) in topo.assets.iter().enumerate() {
        let (n, f) = (&now[i], &fut[i]);
        let st = if n.tlp >= Tlp::Yellow {
            AssetState {
                tlp: n.tlp,
                own: worst_point(conds.get(&a.id)).0,
                reason: n.reason.clone(),
                causes: n.causes.iter().cloned().collect(),
                eta_min: n.eta.or(if n.tlp == Tlp::Yellow { f.eta } else { None }),
                redundancy_lost: n.redundancy_lost,
            }
        } else if f.tlp >= Tlp::Orange {
            let roots: Vec<&str> = f.causes.iter().map(String::as_str).collect();
            let why = if f.causes.contains(&a.id) {
                f.reason.clone()
            } else {
                format!("On trend: {} — {}", names(&ix, &roots), f.reason)
            };
            AssetState {
                tlp: Tlp::Yellow,
                own: worst_point(conds.get(&a.id)).0,
                reason: why,
                causes: f.causes.iter().cloned().collect(),
                eta_min: f.eta,
                redundancy_lost: n.redundancy_lost,
            }
        } else {
            AssetState {
                tlp: n.tlp,
                own: worst_point(conds.get(&a.id)).0,
                reason: n.reason.clone(),
                causes: vec![],
                eta_min: None,
                redundancy_lost: n.redundancy_lost,
            }
        };
        if st.tlp != Tlp::Green || st.redundancy_lost {
            assets.insert(a.id.clone(), st.clone());
        }
        states.push(st);
    }
    let state_of = |id: &str| -> &AssetState { &states[gr.g[gr.idx[id]]] };

    // ---- customers ----
    let trays_by_rack: BTreeMap<&str, Vec<&Asset>> = {
        let mut m: BTreeMap<&str, Vec<&Asset>> = BTreeMap::new();
        for a in &topo.assets {
            if a.kind == AssetKind::ComputeTray {
                if let Some(p) = &a.parent {
                    m.entry(p.as_str()).or_default().push(a);
                }
            }
        }
        m
    };
    let mut customers = vec![];
    let mut kpis = Kpis { customers_total: topo.customers.len() as u32, ..Default::default() };
    // asset id → customers who use it (directly or through a tray/rack), for warnings.
    let mut users: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for c in &topo.customers {
        let mut cs = CustomerState { id: c.id.clone(), ..Default::default() };
        let mut causes = BTreeSet::new();
        let mut eta: Option<f64> = None;
        let mut worst_reason = (Tlp::Green, String::new());
        for al in &c.allocations {
            let Some(asset) = ix.get(al.asset.as_str()) else { continue };
            let units: Vec<(&Asset, u32)> = if asset.kind == AssetKind::Rack {
                let trays = trays_by_rack.get(asset.id.as_str()).cloned().unwrap_or_default();
                if trays.is_empty() {
                    vec![(*asset, al.gpus)]
                } else {
                    let per = (al.gpus as f64 / trays.len() as f64).ceil() as u32;
                    let mut left = al.gpus;
                    trays
                        .into_iter()
                        .map(|t| {
                            let n = per.min(left);
                            left -= n;
                            (t, n)
                        })
                        .filter(|(_, n)| *n > 0)
                        .collect()
                }
            } else {
                vec![(*asset, al.gpus)]
            };
            for (u, n) in units {
                let s = state_of(&u.id);
                cs.gpus.add(s.tlp, n);
                users.entry(u.id.clone()).or_default().insert(c.id.clone());
                if s.tlp != Tlp::Green {
                    causes.extend(s.causes.iter().cloned());
                    for cz in &s.causes {
                        users.entry(cz.clone()).or_default().insert(c.id.clone());
                    }
                    if s.tlp == Tlp::Yellow {
                        if let Some(e) = s.eta_min {
                            eta = Some(eta.map_or(e, |x: f64| x.min(e)));
                        }
                    }
                    if s.tlp > worst_reason.0 {
                        worst_reason = (s.tlp, format!("{}: {}", u.name, s.reason));
                    }
                }
            }
        }
        cs.tlp = if cs.gpus.red > 0 {
            Tlp::Red
        } else if cs.gpus.orange > 0 {
            Tlp::Orange
        } else if cs.gpus.yellow > 0 {
            Tlp::Yellow
        } else if cs.gpus.unknown > 0 {
            Tlp::Unknown
        } else {
            Tlp::Green
        };
        cs.revenue_impacted_hr = (cs.gpus.orange + cs.gpus.red) as f64 * c.rate_per_gpu_hr;
        cs.revenue_at_risk_hr = cs.gpus.yellow as f64 * c.rate_per_gpu_hr;
        cs.eta_min = eta;
        cs.causes = causes.into_iter().take(MAX_CAUSES).collect();
        cs.reason = worst_reason.1;
        kpis.gpus_total += c.gpus();
        kpis.gpus.green += cs.gpus.green;
        kpis.gpus.yellow += cs.gpus.yellow;
        kpis.gpus.orange += cs.gpus.orange;
        kpis.gpus.red += cs.gpus.red;
        kpis.gpus.unknown += cs.gpus.unknown;
        if cs.gpus.orange + cs.gpus.red > 0 {
            kpis.customers_affected += 1;
        } else if cs.gpus.yellow > 0 {
            kpis.customers_at_risk += 1;
        }
        kpis.revenue_impacted_hr += cs.revenue_impacted_hr;
        kpis.revenue_at_risk_hr += cs.revenue_at_risk_hr;
        customers.push(cs);
    }
    customers.sort_by(|a, b| {
        b.tlp.cmp(&a.tlp).then(
            (b.revenue_impacted_hr + b.revenue_at_risk_hr)
                .partial_cmp(&(a.revenue_impacted_hr + a.revenue_at_risk_hr))
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    for s in &states {
        *kpis.assets_by_tlp.entry(format!("{:?}", s.tlp).to_lowercase()).or_default() += 1;
        if s.redundancy_lost {
            kpis.redundancy_lost += 1;
        }
    }

    // ---- warnings: one per root cause, plus every forecast ----
    let gpus_of = |cust: &BTreeSet<String>| -> u32 {
        topo.customers.iter().filter(|c| cust.contains(&c.id)).map(|c| c.gpus()).sum()
    };
    let view_for = |a: &Asset| -> String {
        match a.domain() {
            Domain::Power | Domain::Grid => "power",
            Domain::Cooling => "cooling",
            Domain::Network => "network",
            _ => "hardware",
        }
        .to_string()
    };
    let mut warnings = vec![];
    let mut roots: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in topo.assets.iter().enumerate() {
        let own = worst_point(conds.get(&a.id));
        if own.0 == Tlp::Green {
            continue;
        }
        // A symptom of an upstream cause is folded into that cause's warning.
        let explained = !now[i].causes.is_empty() && !now[i].causes.contains(&a.id) && own.0 != Tlp::Unknown;
        if explained {
            continue;
        }
        roots.insert(a.id.as_str());
        let kind = match own.0 {
            Tlp::Red => WarningKind::Out,
            Tlp::Orange => WarningKind::Degraded,
            Tlp::Yellow => WarningKind::Watch,
            Tlp::Unknown => WarningKind::DataQuality,
            Tlp::Green => unreachable!(),
        };
        let affected = users.get(&a.id).cloned().unwrap_or_default();
        let downstream = gr.downstream(&a.id);
        let impacted = downstream.iter().filter(|d| state_of(&d.id).tlp >= Tlp::Orange).count();
        let red_lost = downstream.iter().filter(|d| state_of(&d.id).redundancy_lost).count();
        let mut detail = own.1.clone();
        if impacted > 0 {
            detail.push_str(&format!(" · {impacted} downstream assets degraded or out"));
        }
        if red_lost > 0 {
            detail.push_str(&format!(" · redundancy lost on {red_lost}"));
        }
        warnings.push(Warning {
            id: format!("{}:{}", a.id, format!("{kind:?}").to_lowercase()),
            kind,
            tlp: own.0,
            asset: a.id.clone(),
            asset_name: a.name.clone(),
            title: match kind {
                WarningKind::Out => format!("{} out", a.name),
                WarningKind::Degraded => format!("{} degraded", a.name),
                WarningKind::Watch => format!("{}: early warning", a.name),
                _ => format!("{}: no trustworthy data", a.name),
            },
            detail,
            eta_min: states[i].eta_min.filter(|_| own.0 == Tlp::Yellow),
            gpus: gpus_of(&affected),
            customers: affected.into_iter().collect(),
            view: view_for(a),
        });
        if red_lost > 0 && own.0 >= Tlp::Orange {
            warnings.push(Warning {
                id: format!("{}:redundancy", a.id),
                kind: WarningKind::RedundancyLost,
                tlp: Tlp::Yellow,
                asset: a.id.clone(),
                asset_name: a.name.clone(),
                title: format!("Redundancy lost: {red_lost} assets on a single path"),
                detail: format!("{} is down; one more failure on the remaining path takes them out", a.name),
                eta_min: None,
                customers: vec![],
                gpus: 0,
                view: view_for(a),
            });
        }
    }
    for p in &preds.items {
        let Some(a) = ix.get(p.asset.as_str()) else { continue };
        let affected = users.get(&a.id).cloned().unwrap_or_default();
        warnings.push(Warning {
            id: format!("{}:pred:{}", p.point, format!("{:?}", p.kind).to_lowercase()),
            kind: WarningKind::Prediction,
            tlp: Tlp::Yellow,
            asset: a.id.clone(),
            asset_name: a.name.clone(),
            title: match p.kind {
                PredictionKind::PeerOutlier => format!("{}: unusual versus its peers", a.name),
                PredictionKind::Runtime => format!("{}: stored energy runs out in {}", a.name, fmt_eta(p.eta_min)),
                PredictionKind::Forecast => format!("{}: predicted {} in {}", a.name, p.tlp_at_eta.label().to_lowercase(), fmt_eta(p.eta_min)),
            },
            detail: p.text.clone(),
            eta_min: Some(p.eta_min),
            gpus: gpus_of(&affected),
            customers: affected.into_iter().collect(),
            view: view_for(a),
        });
    }
    warnings.sort_by(|a, b| {
        b.tlp
            .cmp(&a.tlp)
            .then(a.kind.cmp(&b.kind))
            .then(b.gpus.cmp(&a.gpus))
            .then(a.eta_min.unwrap_or(1e9).partial_cmp(&b.eta_min.unwrap_or(1e9)).unwrap())
    });

    Ok(Snapshot {
        at_ms: now_ms,
        topo_version: topo.version,
        assets,
        customers,
        warnings,
        kpis,
        predictions_at_ms: preds.at_ms,
        took_us: 0,
    })
}

// ------------------------------------------------------------------ trees ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    pub tlp: Tlp,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<f64>,
    /// Via which requirement this node connects to its parent (e.g. "power A/B").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub via: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub customers: Vec<String>,
    pub gpus: u32,
    /// Total descendants (including collapsed ones).
    pub size: u32,
    pub children: Vec<TreeNode>,
}

pub struct TreeCtx<'a> {
    pub gr: Graph<'a>,
    pub snap: &'a Snapshot,
    pub tenants: BTreeMap<String, Vec<(String, u32)>>,
    pub children: BTreeMap<&'a str, Vec<&'a Asset>>,
}

impl<'a> TreeCtx<'a> {
    pub fn new(topo: &'a Topology, snap: &'a Snapshot) -> Result<Self, String> {
        let mut children: BTreeMap<&str, Vec<&Asset>> = BTreeMap::new();
        for a in &topo.assets {
            if let Some(p) = &a.parent {
                children.entry(p.as_str()).or_default().push(a);
            }
        }
        Ok(TreeCtx { gr: Graph::new(topo)?, snap, tenants: topo.tenants_by_asset(), children })
    }
    fn state(&self, id: &str) -> AssetState {
        self.snap.assets.get(id).cloned().unwrap_or_default()
    }
    fn node(&self, a: &Asset, via: String) -> TreeNode {
        let s = self.state(&a.id);
        let t = self.tenants.get(&a.id);
        TreeNode {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind,
            tlp: s.tlp,
            reason: s.reason,
            eta_min: s.eta_min,
            via,
            customers: t.map(|v| v.iter().map(|x| x.0.clone()).collect()).unwrap_or_default(),
            gpus: t.map(|v| v.iter().map(|x| x.1).sum()).unwrap_or(0),
            size: 0,
            children: vec![],
        }
    }

    /// What depends on `root` (impact), as a spanning tree. Trays are folded into racks
    /// unless `trays` is set.
    pub fn downstream(&self, root: &str, depth: usize, trays: bool) -> Option<TreeNode> {
        let &s = self.gr.idx.get(root)?;
        let mut seen = BTreeSet::new();
        Some(self.down_rec(s, String::new(), depth, trays, &mut seen))
    }
    fn down_rec(&self, n: NodeIndex, via: String, depth: usize, trays: bool, seen: &mut BTreeSet<NodeIndex>) -> TreeNode {
        seen.insert(n);
        let a = self.gr.asset(n);
        let mut node = self.node(a, via);
        let mut kids: Vec<NodeIndex> = self
            .gr
            .g
            .edges_directed(n, PgDir::Outgoing)
            .map(|e| e.target())
            .filter(|c| !seen.contains(c))
            .filter(|c| trays || self.gr.asset(*c).kind != AssetKind::ComputeTray)
            .collect();
        kids.sort_by_key(|c| (std::cmp::Reverse(self.state(&self.gr.asset(*c).id).tlp), self.gr.asset(*c).id.clone()));
        for c in &kids {
            seen.insert(*c);
        }
        let mut size = 0;
        for c in kids {
            let ca = self.gr.asset(c);
            let via = ca.requires.iter().find(|r| r.of.contains(&a.id)).map(|r| r.label.clone()).unwrap_or_default();
            if depth == 0 {
                size += 1;
                continue;
            }
            let child = self.down_rec(c, via, depth - 1, trays, seen);
            size += 1 + child.size;
            node.gpus = node.gpus.max(0);
            node.children.push(child);
        }
        node.size = size;
        node
    }

    /// What `id` depends on (root-cause view): requirement groups as children.
    pub fn upstream(&self, id: &str, depth: usize) -> Option<TreeNode> {
        let &s = self.gr.idx.get(id)?;
        Some(self.up_rec(s, String::new(), depth))
    }
    fn up_rec(&self, n: NodeIndex, via: String, depth: usize) -> TreeNode {
        let a = self.gr.asset(n);
        let mut node = self.node(a, via);
        if depth == 0 {
            node.size = a.requires.iter().map(|r| r.of.len() as u32).sum();
            return node;
        }
        let mut size = 0;
        for r in &a.requires {
            for p in &r.of {
                if let Some(&pn) = self.gr.idx.get(p.as_str()) {
                    let child = self.up_rec(pn, format!("{} (needs {} of {})", r.label, r.need, r.of.len()), depth - 1);
                    size += 1 + child.size;
                    node.children.push(child);
                }
            }
        }
        node.children.sort_by_key(|c| std::cmp::Reverse(c.tlp));
        node.size = size;
        node
    }

    /// Physical containment (site → hall → pod → row → rack → tray) with worst-child rollup.
    pub fn containment(&self, id: &str, depth: usize) -> Option<TreeNode> {
        let a = self.gr.topo.asset(id)?;
        Some(self.cont_rec(a, depth))
    }
    fn cont_rec(&self, a: &Asset, depth: usize) -> TreeNode {
        let mut node = self.node(a, String::new());
        let kids = self.children.get(a.id.as_str()).cloned().unwrap_or_default();
        let mut size = 0;
        for k in kids {
            let child = self.cont_rec(k, depth.saturating_sub(1));
            size += 1 + child.size;
            if child.tlp > node.tlp {
                node.tlp = child.tlp;
                if node.reason.is_empty() || child.tlp >= Tlp::Orange {
                    node.reason = format!("{}: {}", child.name, child.reason);
                }
            }
            node.gpus += child.gpus;
            if depth > 0 {
                node.children.push(child);
            }
        }
        node.children.sort_by(|x, y| y.tlp.cmp(&x.tlp).then(x.id.cmp(&y.id)));
        node.size = size;
        node
    }
}
