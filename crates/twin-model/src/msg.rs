//! Messages on the bus and documents in KV.
use crate::config::TwinConfig;
use crate::topo::Topology;
use crate::Tlp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// DSX BMS value payload: `{"value": 24.6, "timestamp": 1731000000000, "quality": 1}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DsxValue {
    pub value: f64,
    pub timestamp: i64,
    #[serde(default = "one")]
    pub quality: i64,
}
fn one() -> i64 {
    1
}

/// DSX BMS metadata payload (published on the matching `Metadata` subject).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DsxMetadata {
    pub object_type: String,
    pub point_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eng_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serves_id: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rack_location_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_text: Option<Vec<StateText>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateText {
    pub value: i64,
    pub text: String,
}

/// The Value subject that corresponds to a Metadata subject.
pub fn value_subject_for(metadata_subject: &str) -> String {
    metadata_subject.replacen(".Metadata.", ".Value.", 1)
}

/// History import: what a site's existing historian hands the twin on day one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backfill {
    pub point: String,
    pub start_ms: i64,
    pub step_s: i64,
    pub values: Vec<f32>,
}

/// A site profile, expanded: what `twinctl publish` (or the simulator) sends the twin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileMsg {
    pub topology: Topology,
    pub config: TwinConfig,
}

/// Latest reading of a point (`lkv.<point>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lkv {
    pub v: f64,
    pub t: i64,
    pub q: i64,
    pub tlp: Tlp,
}

/// A point's condition changed (published by twin-ingest on `twin.v1.sev.<asset>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SevEvent {
    pub asset: String,
    pub point: String,
    pub class: String,
    pub tlp: Tlp,
    pub text: String,
    pub value: f64,
    pub t_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointCond {
    pub class: String,
    pub tlp: Tlp,
    pub text: String,
    pub value: f64,
    pub t_ms: i64,
}

/// `conditions`: asset → point → condition. Only non-green points are kept.
pub type Conditions = BTreeMap<String, BTreeMap<String, PointCond>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionKind {
    /// augurs MSTL forecast crosses the degraded limit within the horizon.
    Forecast,
    /// augurs MAD detector: this point disagrees with its peers.
    PeerOutlier,
    /// A stored-energy runtime point (UPS on battery) counts down to empty.
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub asset: String,
    pub point: String,
    pub class: String,
    pub kind: PredictionKind,
    /// State expected at the ETA.
    pub tlp_at_eta: Tlp,
    pub eta_min: f64,
    pub text: String,
    pub current: f64,
    pub threshold: f64,
    /// Recent history and the forecast, for the UI sparkline.
    #[serde(default)]
    pub history: Vec<f32>,
    #[serde(default)]
    pub forecast: Vec<f32>,
    #[serde(default)]
    pub upper: Vec<f32>,
    #[serde(default)]
    pub lower: Vec<f32>,
}

/// `predictions`: written by twin-predict after each fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PredictionSet {
    pub at_ms: i64,
    pub fitted: u32,
    pub took_ms: u64,
    pub horizon_min: f64,
    pub items: Vec<Prediction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    Out,
    Degraded,
    Prediction,
    Watch,
    RedundancyLost,
    DataQuality,
}

/// One row on the Warnings & predictions page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    pub id: String,
    pub kind: WarningKind,
    pub tlp: Tlp,
    pub asset: String,
    pub asset_name: String,
    pub title: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<f64>,
    /// Customers who would feel it.
    pub customers: Vec<String>,
    pub gpus: u32,
    /// Which visualization shows it best: hardware | power | cooling | network | customer.
    pub view: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssetState {
    pub tlp: Tlp,
    /// The asset's own condition, before dependencies.
    pub own: Tlp,
    pub reason: String,
    /// Upstream assets whose condition explains this one (root causes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub redundancy_lost: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GpuCounts {
    pub green: u32,
    pub yellow: u32,
    pub orange: u32,
    pub red: u32,
    pub unknown: u32,
}

impl GpuCounts {
    pub fn add(&mut self, t: Tlp, n: u32) {
        match t {
            Tlp::Green => self.green += n,
            Tlp::Yellow => self.yellow += n,
            Tlp::Orange => self.orange += n,
            Tlp::Red => self.red += n,
            Tlp::Unknown => self.unknown += n,
        }
    }
    pub fn affected(&self) -> u32 {
        self.yellow + self.orange + self.red
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CustomerState {
    pub id: String,
    pub tlp: Tlp,
    pub gpus: GpuCounts,
    /// $/hr of contracted capacity that is degraded or out now.
    pub revenue_impacted_hr: f64,
    /// $/hr that is predicted to be impacted (Yellow).
    pub revenue_at_risk_hr: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<f64>,
    pub causes: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Kpis {
    pub gpus_total: u32,
    pub gpus: GpuCounts,
    pub customers_total: u32,
    pub customers_affected: u32,
    pub customers_at_risk: u32,
    pub revenue_impacted_hr: f64,
    pub revenue_at_risk_hr: f64,
    pub assets_by_tlp: BTreeMap<String, u32>,
    pub redundancy_lost: u32,
}

/// `snapshot`: everything the dashboards need, recomputed by twin-impact on every change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub at_ms: i64,
    pub topo_version: u64,
    /// Non-green assets only; anything missing is Green.
    pub assets: BTreeMap<String, AssetState>,
    pub customers: Vec<CustomerState>,
    pub warnings: Vec<Warning>,
    pub kpis: Kpis,
    pub predictions_at_ms: i64,
    pub took_us: u64,
}
