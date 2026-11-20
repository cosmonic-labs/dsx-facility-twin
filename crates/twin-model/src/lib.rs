//! # twin-model
//!
//! The shared vocabulary of the DSX facility twin: the site topology (assets, the
//! dependencies between them and their redundancy), the canonical point classes and
//! limits that turn telemetry into conditions, the messages that move between the
//! simulator and the Wasm analytics, and the impact engine that colors the graph.
//!
//! The crate compiles for both the native simulator and `wasm32-wasip2` components.
//! YAML profile loading is behind the `profile` feature (native only).

pub mod config;
pub mod engine;
pub mod msg;
pub mod tlp;
pub mod topo;
pub mod ts;

#[cfg(feature = "profile")]
pub mod profile;

pub use tlp::Tlp;

/// KV bucket that holds all twin state (demo: Control's data-plane NATS).
pub const BUCKET: &str = "twin";

/// Subjects. BMS telemetry follows DSX Exchange's `BMS.v1.PUB.{Value|Metadata}.<ObjectType>.<PointType>.…`
/// convention (MQTT topic with `/` mapped to `.`). Everything else is the twin's own namespace.
pub mod subjects {
    pub const BMS_VALUE: &str = "BMS.v1.PUB.Value.>";
    pub const BMS_METADATA: &str = "BMS.v1.PUB.Metadata.>";
    /// GPU / network health (stand-in for NVSentinel, Fleet Intelligence, UFM/NetQ).
    pub const HEALTH: &str = "twin.v1.health.>";
    pub const BACKFILL: &str = "twin.v1.backfill";
    pub const PROFILE: &str = "twin.v1.profile";
    pub const TICK_PREDICT: &str = "twin.v1.tick.predict";
    pub const SEV_PREFIX: &str = "twin.v1.sev";
    pub const PRED: &str = "twin.v1.pred";
    pub const SNAPSHOT_UPDATED: &str = "twin.v1.snapshot.updated";
    pub const TRUTH: &str = "twin.v1.sim.truth";
}

/// KV keys.
pub mod keys {
    pub const TOPOLOGY: &str = "topology";
    pub const CONFIG: &str = "config";
    pub const CONDITIONS: &str = "conditions";
    pub const PREDICTIONS: &str = "predictions";
    pub const SNAPSHOT: &str = "snapshot";
    pub fn lkv(point: &str) -> String {
        format!("lkv.{point}")
    }
    pub fn pmap(subject: &str) -> String {
        format!("pmap.{}", crate::keys::safe(subject))
    }
    pub fn series(point: &str, day: i64) -> String {
        format!("ts.{point}.{day}")
    }
    /// NATS KV keys allow `[-/_=.a-zA-Z0-9]`.
    pub fn safe(s: &str) -> String {
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() || "-_=./".contains(c) { c } else { '_' })
            .collect()
    }
}
