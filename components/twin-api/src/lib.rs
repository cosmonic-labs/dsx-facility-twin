//! twin-api — the control panel, served from Wasm.
//!
//! `wasi:http@0.3.0` handler that serves the React UI (embedded at build time), a JSON API over
//! the twin's NATS KV state, server-side trees from the impact engine (petgraph), and a narrow
//! proxy to the simulator's scenario API. Its only outbound host is the simulator service.
//!
//! Login is a static demo credential (admin/admin) that sets a session cookie. It is a lab
//! convenience, not authentication; see docs/security.md.
mod bindings {
    wit_bindgen::generate!({ path: "../wit", world: "twin-api-state", generate_all });
}
mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}

use bindings::wasmcloud::nats;
use bytes::Bytes;
use http::{Method, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use std::collections::BTreeMap;
use twin_model::engine::TreeCtx;
use twin_model::msg::*;
use twin_model::topo::{AssetKind, Topology};
use twin_model::{keys, ts};
use wasip3::http::types::{ErrorCode, Request};
use wasip3::http_compat::{http_from_wasi_request, http_from_wasi_response, http_into_wasi_request, http_into_wasi_response};

include!("../../common/kv.rs");

struct Component;
wasip3::http::service::export!(Component);

type Resp = Response<Full<Bytes>>;

fn env(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

fn resp(status: StatusCode, ctype: &str, body: impl Into<Bytes>) -> Resp {
    Response::builder()
        .status(status)
        .header("content-type", ctype)
        .header("cache-control", "no-store")
        .body(Full::new(body.into()))
        .unwrap()
}
fn json(v: &serde_json::Value) -> Resp {
    resp(StatusCode::OK, "application/json", serde_json::to_vec(v).unwrap())
}
fn raw_json(b: Vec<u8>) -> Resp {
    resp(StatusCode::OK, "application/json", b)
}
fn err(status: StatusCode, msg: &str) -> Resp {
    resp(status, "application/json", serde_json::to_vec(&serde_json::json!({"error": msg})).unwrap())
}

fn session_token() -> String {
    env("TWIN_SESSION_TOKEN", "dsx-twin-demo-session")
}

fn has_session(req: &http::request::Parts) -> bool {
    let want = format!("twin_session={}", session_token());
    req.headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|c| c.split(';').any(|p| p.trim() == want))
}

fn query(q: Option<&str>) -> BTreeMap<String, String> {
    q.unwrap_or("")
        .split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (!k.is_empty()).then(|| (k.to_string(), v.replace("%20", " ").replace('+', " ")))
        })
        .collect()
}

impl wasip3::exports::http::handler::Guest for Component {
    async fn handle(request: Request) -> Result<wasip3::http::types::Response, ErrorCode> {
        let req = http_from_wasi_request(request)?;
        let (parts, body) = req.into_parts();
        let body = body.collect().await.map(|c| c.to_bytes()).unwrap_or_default();
        let r = route(&parts, body).await.unwrap_or_else(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e));
        http_into_wasi_response(r)
    }
}

async fn route(parts: &http::request::Parts, body: Bytes) -> Result<Resp, String> {
    let path = parts.uri.path().to_string();
    let q = query(parts.uri.query());
    if !path.starts_with("/api/") {
        return Ok(static_file(&path));
    }
    match (parts.method.clone(), path.as_str()) {
        (Method::GET, "/api/health") => return Ok(json(&serde_json::json!({"ok": true}))),
        (Method::POST, "/api/login") => {
            let v: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            let ok = v["username"] == env("TWIN_ADMIN_USER", "admin") && v["password"] == env("TWIN_ADMIN_PASSWORD", "admin");
            if !ok {
                return Ok(err(StatusCode::UNAUTHORIZED, "invalid username or password"));
            }
            let mut r = json(&serde_json::json!({"ok": true, "user": "admin"}));
            r.headers_mut().insert(
                "set-cookie",
                format!("twin_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=43200", session_token()).parse().unwrap(),
            );
            return Ok(r);
        }
        (Method::POST, "/api/logout") => {
            let mut r = json(&serde_json::json!({"ok": true}));
            r.headers_mut().insert("set-cookie", "twin_session=; Path=/; Max-Age=0".parse().unwrap());
            return Ok(r);
        }
        _ => {}
    }
    if !has_session(parts) {
        return Ok(err(StatusCode::UNAUTHORIZED, "login required"));
    }
    if let Some(rest) = path.strip_prefix("/api/sim/") {
        return proxy_sim(parts, rest, body).await;
    }
    let b = bucket().await?;
    let get = |k: &'static str| {
        let b = &b;
        async move { Ok::<_, String>(get_bytes(b, k).await?.map(raw_json).unwrap_or_else(|| err(StatusCode::NOT_FOUND, &format!("{k} not available yet")))) }
    };
    match (parts.method.clone(), path.as_str()) {
        (Method::GET, "/api/me") => Ok(json(&serde_json::json!({"user": "admin"}))),
        (Method::GET, "/api/snapshot") => get(keys::SNAPSHOT).await,
        (Method::GET, "/api/topology") => get(keys::TOPOLOGY).await,
        (Method::GET, "/api/predictions") => get(keys::PREDICTIONS).await,
        (Method::GET, "/api/config") => get(keys::CONFIG).await,
        (Method::GET, "/api/conditions") => get(keys::CONDITIONS).await,
        (Method::GET, "/api/asset") => asset(&b, q.get("id").map(String::as_str).unwrap_or("")).await,
        (Method::GET, "/api/tree") => tree(&b, &q).await,
        (Method::GET, "/api/customer") => customer(&b, q.get("id").map(String::as_str).unwrap_or("")).await,
        (Method::GET, "/api/series") => series(&b, &q).await,
        (Method::GET, "/api/lkv") => lkv(&b, &q).await,
        (Method::GET, "/api/unmapped") => unmapped(&b).await,
        _ => Ok(err(StatusCode::NOT_FOUND, "no such endpoint")),
    }
}

async fn loaded(b: &nats::kv::Bucket) -> Result<(std::sync::Arc<Model>, Snapshot), String> {
    let m = model(b).await?;
    let snap: Snapshot = get_json(b, keys::SNAPSHOT).await?.unwrap_or_default();
    Ok((m, snap))
}

async fn readings_for(b: &nats::kv::Bucket, m: &Model, a: &twin_model::topo::Asset) -> Result<Vec<serde_json::Value>, String> {
    let mut out = vec![];
    for p in &a.points {
        let l: Option<Lkv> = get_json(b, &keys::lkv(&p.id)).await?;
        let def = m.cfg.classes.get(&p.class);
        let text = l.as_ref().map(|l| m.cfg.evaluate(&p.class, l.v, l.q, Some(a)).text).unwrap_or_default();
        let state = l.as_ref().and_then(|l| def.and_then(|d| d.states.iter().find(|s| s.value == l.v.round() as i64)).map(|s| s.text.clone()));
        out.push(serde_json::json!({
            "point": p.id, "class": p.class, "label": def.map(|d| d.label.clone()).unwrap_or(p.class.clone()),
            "unit": p.unit, "forecast": p.forecast, "subject": p.subject,
            "v": l.as_ref().map(|l| l.v), "t": l.as_ref().map(|l| l.t), "q": l.as_ref().map(|l| l.q),
            "tlp": l.as_ref().map(|l| l.tlp), "state": state, "text": text,
        }));
    }
    Ok(out)
}

async fn asset(b: &nats::kv::Bucket, id: &str) -> Result<Resp, String> {
    let (m, snap) = loaded(b).await?;
    let Some(a) = m.topo.asset(id) else { return Ok(err(StatusCode::NOT_FOUND, "no such asset")) };
    let ctx = TreeCtx::new(&m.topo, &snap)?;
    let readings = readings_for(b, &m, a).await?;
    let tenants: Vec<serde_json::Value> = m
        .topo
        .customers
        .iter()
        .filter(|c| c.allocations.iter().any(|al| al.asset == a.id || m.topo.asset(&al.asset).and_then(|x| x.parent.clone()).as_deref() == Some(a.id.as_str())))
        .map(|c| serde_json::json!({"id": c.id, "name": c.name, "tier": c.tier}))
        .collect();
    Ok(json(&serde_json::json!({
        "asset": a,
        "kind_label": a.kind.label(),
        "state": snap.assets.get(id),
        "readings": readings,
        "upstream": ctx.upstream(id, 3),
        "downstream": ctx.downstream(id, 2, false),
        "tenants": tenants,
    })))
}

async fn tree(b: &nats::kv::Bucket, q: &BTreeMap<String, String>) -> Result<Resp, String> {
    let (m, snap) = loaded(b).await?;
    let ctx = TreeCtx::new(&m.topo, &snap)?;
    let root = q.get("root").cloned().unwrap_or_else(|| m.topo.site.id.clone());
    let depth: usize = q.get("depth").and_then(|d| d.parse().ok()).unwrap_or(4);
    let trays = q.get("trays").map(|v| v == "1").unwrap_or(false);
    let t = match q.get("dir").map(String::as_str).unwrap_or("down") {
        "up" => ctx.upstream(&root, depth),
        "contain" => ctx.containment(&root, depth),
        _ => ctx.downstream(&root, depth, trays),
    };
    Ok(t.map(|t| json(&serde_json::to_value(t).unwrap())).unwrap_or_else(|| err(StatusCode::NOT_FOUND, "no such asset")))
}

/// Customer view: the customer at the root, its racks below, and for each rack the chain of
/// equipment it depends on (power, cooling, network) with states.
async fn customer(b: &nats::kv::Bucket, id: &str) -> Result<Resp, String> {
    let (m, snap) = loaded(b).await?;
    let Some(c) = m.topo.customers.iter().find(|c| c.id == id) else { return Ok(err(StatusCode::NOT_FOUND, "no such customer")) };
    let ctx = TreeCtx::new(&m.topo, &snap)?;
    let state = snap.customers.iter().find(|s| s.id == id);
    let mut racks: BTreeMap<String, u32> = BTreeMap::new();
    for al in &c.allocations {
        let rack = match m.topo.asset(&al.asset) {
            Some(a) if a.kind == AssetKind::ComputeTray => a.parent.clone().unwrap_or_default(),
            Some(a) => a.id.clone(),
            None => continue,
        };
        *racks.entry(rack).or_default() += al.gpus;
    }
    let trees: Vec<serde_json::Value> = racks
        .iter()
        .filter_map(|(r, g)| ctx.upstream(r, 4).map(|t| serde_json::json!({"rack": r, "gpus": g, "tree": t})))
        .collect();
    Ok(json(&serde_json::json!({"customer": c, "state": state, "racks": trees})))
}

async fn series(b: &nats::kv::Bucket, q: &BTreeMap<String, String>) -> Result<Resp, String> {
    let point = q.get("point").cloned().unwrap_or_default();
    let hours: i64 = q.get("hours").and_then(|h| h.parse().ok()).unwrap_or(24);
    let snap: Snapshot = get_json(b, keys::SNAPSHOT).await?.unwrap_or_default();
    let mut end = snap.at_ms;
    if let Some(l) = get_json::<Lkv>(b, &keys::lkv(&point)).await? {
        end = end.max(l.t);
    }
    let days = (hours + 23) / 24 + 1;
    let d = ts::day_of(end);
    let mut chunks = vec![];
    for k in (0..days).rev() {
        if let Some(c) = get_json::<ts::Chunk>(b, &keys::series(&point, d - k)).await? {
            chunks.push(c);
        }
    }
    let (start, values) = ts::assemble(&chunks, end, (hours * 12) as usize);
    Ok(json(&serde_json::json!({"point": point, "start_ms": start, "step_s": ts::STEP_S, "values": values})))
}

/// Latest values for a set of points: `?points=a.x,b.y`.
async fn lkv(b: &nats::kv::Bucket, q: &BTreeMap<String, String>) -> Result<Resp, String> {
    let mut out = serde_json::Map::new();
    for p in q.get("points").map(|s| s.split(',').collect::<Vec<_>>()).unwrap_or_default().into_iter().take(200) {
        if let Some(l) = get_json::<Lkv>(b, &keys::lkv(p)).await? {
            out.insert(p.to_string(), serde_json::to_value(l).unwrap());
        }
    }
    Ok(json(&serde_json::Value::Object(out)))
}

async fn unmapped(b: &nats::kv::Bucket) -> Result<Resp, String> {
    let keys = b.keys("unmapped.>".into()).await.map(|p| p.keys).unwrap_or_default();
    let mut out = vec![];
    for k in keys.iter().take(100) {
        if let Some(v) = get_json::<serde_json::Value>(b, k).await? {
            out.push(serde_json::json!({"key": k, "metadata": v}));
        }
    }
    Ok(json(&serde_json::json!({"count": keys.len(), "items": out})))
}

/// Forward `/api/sim/<rest>` to the simulator. Only GET and POST, only under /api.
async fn proxy_sim(parts: &http::request::Parts, rest: &str, body: Bytes) -> Result<Resp, String> {
    let base = env("SIM_URL", "http://twin-sim.dsx-twin.svc.cluster.local:8090");
    let qs = parts.uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let uri = format!("{base}/api/{rest}{qs}");
    let method = if parts.method == Method::POST { Method::POST } else { Method::GET };
    let req = http::Request::builder()
        .method(method)
        .uri(&uri)
        .header("content-type", "application/json")
        .body(Full::new(body))
        .map_err(|e| e.to_string())?;
    let wreq = http_into_wasi_request(req).map_err(|e| format!("{e:?}"))?;
    let wresp = match wasip3::http::client::send(wreq).await {
        Ok(r) => r,
        Err(e) => return Ok(err(StatusCode::BAD_GATEWAY, &format!("simulator unreachable: {e:?}"))),
    };
    let r = http_from_wasi_response(wresp).map_err(|e| format!("{e:?}"))?;
    let status = r.status();
    let bytes = r.into_body().collect().await.map(|c| c.to_bytes()).unwrap_or_default();
    Ok(resp(status, "application/json", bytes))
}

fn static_file(path: &str) -> Resp {
    let p = if path == "/" { "/index.html" } else { path };
    if let Some((ctype, bytes)) = assets::lookup(p) {
        let mut r = resp(StatusCode::OK, ctype, Bytes::from_static(bytes));
        if p.starts_with("/assets/") {
            r.headers_mut().insert("cache-control", "public, max-age=31536000, immutable".parse().unwrap());
        }
        return r;
    }
    // SPA fallback
    match assets::lookup("/index.html") {
        Some((ctype, bytes)) => resp(StatusCode::OK, ctype, Bytes::from_static(bytes)),
        None => resp(StatusCode::OK, "text/html", "<h1>DSX facility twin</h1><p>UI not built. Run make ui.</p>"),
    }
}

#[allow(dead_code)]
fn _unused(_: &Topology) {}
