// Shared KV / publish helpers, `include!`d by each component after its bindings are in scope
// as `nats` (wasmcloud:nats). Kept as an include so every component stays one crate.

#[allow(dead_code)]
pub async fn bucket() -> Result<nats::kv::Bucket, String> {
    nats::kv::open(twin_model::BUCKET.to_string()).await.map_err(|e| format!("kv open {}: {e:?}", twin_model::BUCKET))
}

#[allow(dead_code)]
pub async fn get_bytes(b: &nats::kv::Bucket, key: &str) -> Result<Option<Vec<u8>>, String> {
    match b.get(key.to_string()).await {
        Ok(e) => Ok(Some(e.value)),
        Err(nats::types::NatsError::KeyNotFound) => Ok(None),
        Err(e) => Err(format!("kv get {key}: {e:?}")),
    }
}

#[allow(dead_code)]
pub async fn get_json<T: serde::de::DeserializeOwned>(b: &nats::kv::Bucket, key: &str) -> Result<Option<T>, String> {
    match get_bytes(b, key).await? {
        Some(v) => serde_json::from_slice(&v).map(Some).map_err(|e| format!("decode {key}: {e}")),
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub async fn put_json<T: serde::Serialize>(b: &nats::kv::Bucket, key: &str, v: &T) -> Result<(), String> {
    let body = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    b.put(key.to_string(), body).await.map(|_| ()).map_err(|e| format!("kv put {key}: {e:?}"))
}

#[allow(dead_code)]
pub async fn publish(subject: &str, body: Vec<u8>) -> Result<(), String> {
    nats::core::publish(nats::types::NatsMessage { subject: subject.to_string(), body, reply_to: None, headers: None })
        .await
        .map_err(|e| format!("publish {subject}: {e:?}"))
}

#[allow(dead_code)]
pub fn wall_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Topology + config, cached per instance and refreshed when `profile.version` changes.
#[allow(dead_code)]
pub struct Model {
    pub version: u64,
    pub topo: twin_model::topo::Topology,
    pub cfg: twin_model::config::TwinConfig,
}

#[allow(dead_code)]
static MODEL: std::sync::Mutex<Option<(std::sync::Arc<Model>, i64)>> = std::sync::Mutex::new(None);

#[allow(dead_code)]
pub fn cached_model() -> Option<std::sync::Arc<Model>> {
    MODEL.lock().unwrap().as_ref().map(|(m, _)| m.clone())
}

#[allow(dead_code)]
pub fn set_model(m: Model) -> std::sync::Arc<Model> {
    let a = std::sync::Arc::new(m);
    *MODEL.lock().unwrap() = Some((a.clone(), wall_ms()));
    a
}

/// The current model; re-validated against `profile.version` at most every 5 s.
#[allow(dead_code)]
pub async fn model(b: &nats::kv::Bucket) -> Result<std::sync::Arc<Model>, String> {
    let now = wall_ms();
    let cur = MODEL.lock().unwrap().clone();
    if let Some((m, checked)) = &cur {
        if now - checked < 5_000 {
            return Ok(m.clone());
        }
    }
    let ver: u64 = get_json(b, "profile.version").await?.unwrap_or(0);
    if let Some((m, _)) = &cur {
        if m.version == ver && ver != 0 {
            *MODEL.lock().unwrap() = Some((m.clone(), now));
            return Ok(m.clone());
        }
    }
    let topo: twin_model::topo::Topology = get_json(b, twin_model::keys::TOPOLOGY).await?.ok_or("no topology yet (profile not published)")?;
    let cfg: twin_model::config::TwinConfig = get_json(b, twin_model::keys::CONFIG).await?.ok_or("no config yet")?;
    Ok(set_model(Model { version: ver, topo, cfg }))
}
