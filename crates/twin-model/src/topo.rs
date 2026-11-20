//! Site topology: the assets in an AI factory, what each one depends on, and who uses it.
//!
//! The topology is a DAG of *requirements*: a consumer (a rack) requires a group of
//! providers (its two busway taps) and needs `need` of them healthy. Redundancy (2N, N+1)
//! is just `need < of.len()`. Every view in the UI (power, cooling, network, customer) is a
//! projection of this one graph.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Grid,
    Power,
    Cooling,
    Network,
    Compute,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    // physical containers (no dependencies, used for layout)
    Site,
    Hall,
    Pod,
    Row,
    // grid and power
    Grid,
    UtilityFeed,
    Generator,
    Ats,
    Transformer,
    Ups,
    Bess,
    Switchboard,
    Busway,
    TapOff,
    PowerShelf,
    // cooling
    CoolingTower,
    FacilityPump,
    FacilityWater,
    Chiller,
    Crah,
    Cdu,
    // network
    SpineSwitch,
    LeafSwitch,
    // compute
    Rack,
    NvlinkFabric,
    ComputeTray,
}

impl AssetKind {
    pub fn domain(self) -> Domain {
        use AssetKind::*;
        match self {
            Site | Hall | Pod | Row => Domain::Space,
            Grid => Domain::Grid,
            UtilityFeed | Generator | Ats | Transformer | Ups | Bess | Switchboard | Busway
            | TapOff | PowerShelf => Domain::Power,
            CoolingTower | FacilityPump | FacilityWater | Chiller | Crah | Cdu => Domain::Cooling,
            SpineSwitch | LeafSwitch => Domain::Network,
            Rack | NvlinkFabric | ComputeTray => Domain::Compute,
        }
    }
    /// DSX BMS object type used on the wire. Types DSX does not define today are
    /// marked with `x` in docs/data-model.md.
    pub fn dsx_object_type(self) -> &'static str {
        use AssetKind::*;
        match self {
            Grid => "GridConnection",
            UtilityFeed => "PowerMeter",
            Generator => "Generator",
            Ats => "TransferSwitch",
            Transformer => "Transformer",
            Ups => "UPS",
            Bess => "BESS",
            Switchboard => "PowerMeter",
            Busway => "PowerMeter",
            TapOff => "PowerMeter",
            PowerShelf => "PowerShelf",
            CoolingTower => "CoolingTower",
            FacilityPump => "Pump",
            FacilityWater => "WaterLoop",
            Chiller => "Chiller",
            Crah => "CRAH",
            Cdu => "CDU",
            SpineSwitch | LeafSwitch => "NetworkSwitch",
            Rack => "Rack",
            NvlinkFabric => "NVLinkFabric",
            ComputeTray => "ComputeTray",
            Site | Hall | Pod | Row => "Space",
        }
    }
    pub fn label(self) -> &'static str {
        use AssetKind::*;
        match self {
            Site => "Site",
            Hall => "Data hall",
            Pod => "Pod",
            Row => "Row",
            Grid => "Grid connection",
            UtilityFeed => "Utility feed",
            Generator => "Generator",
            Ats => "Transfer switch",
            Transformer => "Transformer",
            Ups => "UPS module",
            Bess => "Battery storage",
            Switchboard => "Switchboard",
            Busway => "Busway",
            TapOff => "Busway tap-off",
            PowerShelf => "Power shelf",
            CoolingTower => "Dry cooler",
            FacilityPump => "Facility water pump",
            FacilityWater => "Facility water loop",
            Chiller => "Trim chiller",
            Crah => "CRAH",
            Cdu => "CDU",
            SpineSwitch => "Spine switch",
            LeafSwitch => "Leaf switch",
            Rack => "GB200 NVL72 rack",
            NvlinkFabric => "NVLink fabric",
            ComputeTray => "Compute tray",
        }
    }
}

/// A dependency group: the consumer needs `need` of `of` to be healthy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub domain: Domain,
    /// Short human label, e.g. "power A/B", "liquid cooling", "fabric uplinks".
    pub label: String,
    pub need: u32,
    pub of: Vec<String>,
    /// A bridged requirement can ride through a loss on stored energy (a UPS on battery):
    /// losing it makes the consumer Yellow with an ETA instead of Red.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bridged: bool,
    /// A capacity group: with fewer than `need` but more than zero providers available, the
    /// consumer runs limited (Orange) instead of failing (heat rejection, UPS modules, CRAHs).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

/// A telemetry point on an asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointDef {
    /// `<asset>.<class>` (unique).
    pub id: String,
    /// Canonical class (see config.rs), e.g. `liquid_return_temp`.
    pub class: String,
    pub unit: String,
    /// The NATS subject this point arrives on (DSX `BMS.v1.PUB.Value.…`).
    pub subject: String,
    /// Keep history and forecast this point with augurs.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forecast: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    /// Physical parent (row, pod, hall) for layout; not a dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Requirement>,
    /// Design values: rated_kw, design_flow_lpm, capacity_kw, gpus, current_limit_a, …
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<PointDef>,
    /// Free-form labels, e.g. `side:A`, `row:P1-R1`, `vendor:Vertiv`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Asset {
    pub fn domain(&self) -> Domain {
        self.kind.domain()
    }
    pub fn attr(&self, k: &str) -> Option<f64> {
        self.attrs.get(k).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Premium,
    Standard,
    Spot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Allocation {
    /// A rack or a compute tray id.
    pub asset: String,
    pub gpus: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Customer {
    pub id: String,
    pub name: String,
    pub tier: Tier,
    /// Contracted price, used for revenue-at-risk.
    pub rate_per_gpu_hr: f64,
    /// Contracted availability, e.g. 99.95.
    pub sla_pct: f64,
    pub allocations: Vec<Allocation>,
    #[serde(default)]
    pub color: String,
}

impl Customer {
    pub fn gpus(&self) -> u32 {
        self.allocations.iter().map(|a| a.gpus).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SiteInfo {
    pub id: String,
    pub name: String,
    pub org: String,
    pub location: String,
    /// Where the data in this topology came from (template, DCIM import, DSX metadata …).
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Topology {
    pub version: u64,
    pub site: SiteInfo,
    pub assets: Vec<Asset>,
    pub customers: Vec<Customer>,
}

impl Topology {
    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }
    pub fn index(&self) -> BTreeMap<&str, &Asset> {
        self.assets.iter().map(|a| (a.id.as_str(), a)).collect()
    }
    /// Asset id → the customers allocated on it (directly).
    pub fn tenants_by_asset(&self) -> BTreeMap<String, Vec<(String, u32)>> {
        let mut m: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
        for c in &self.customers {
            for a in &c.allocations {
                m.entry(a.asset.clone()).or_default().push((c.id.clone(), a.gpus));
            }
        }
        m
    }
    pub fn points(&self) -> impl Iterator<Item = (&Asset, &PointDef)> {
        self.assets.iter().flat_map(|a| a.points.iter().map(move |p| (a, p)))
    }
}
