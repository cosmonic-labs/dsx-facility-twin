// Mirrors crates/twin-model (Rust). Keep in sync with msg.rs / topo.rs / engine.rs.
export type Tlp = "green" | "unknown" | "yellow" | "orange" | "red";
export type Domain = "grid" | "power" | "cooling" | "network" | "compute" | "space";
export type AssetKind =
  | "site" | "hall" | "pod" | "row" | "grid" | "utility_feed" | "generator" | "ats" | "transformer" | "ups" | "bess"
  | "switchboard" | "busway" | "tap_off" | "power_shelf" | "cooling_tower" | "facility_pump" | "facility_water"
  | "chiller" | "crah" | "cdu" | "spine_switch" | "leaf_switch" | "rack" | "nvlink_fabric" | "compute_tray";

export interface Requirement { domain: Domain; label: string; need: number; of: string[]; bridged?: boolean; partial?: boolean }
export interface PointDef { id: string; class: string; unit: string; subject: string; forecast?: boolean }
export interface Asset {
  id: string; name: string; kind: AssetKind; parent?: string; requires?: Requirement[];
  attrs?: Record<string, number>; points?: PointDef[]; tags?: string[];
}
export interface Allocation { asset: string; gpus: number }
export interface Customer { id: string; name: string; tier: "premium" | "standard" | "spot"; rate_per_gpu_hr: number; sla_pct: number; allocations: Allocation[]; color: string }
export interface Topology { version: number; site: { id: string; name: string; org: string; location: string; source: string }; assets: Asset[]; customers: Customer[] }

export interface AssetState { tlp: Tlp; own: Tlp; reason: string; causes?: string[]; eta_min?: number; redundancy_lost?: boolean }
export interface GpuCounts { green: number; yellow: number; orange: number; red: number; unknown: number }
export interface CustomerState { id: string; tlp: Tlp; gpus: GpuCounts; revenue_impacted_hr: number; revenue_at_risk_hr: number; eta_min?: number; causes: string[]; reason: string }
export type WarningKind = "out" | "degraded" | "prediction" | "watch" | "redundancy_lost" | "data_quality";
export interface Warning { id: string; kind: WarningKind; tlp: Tlp; asset: string; asset_name: string; title: string; detail: string; eta_min?: number; customers: string[]; gpus: number; view: string }
export interface Kpis {
  gpus_total: number; gpus: GpuCounts; customers_total: number; customers_affected: number; customers_at_risk: number;
  revenue_impacted_hr: number; revenue_at_risk_hr: number; assets_by_tlp: Record<string, number>; redundancy_lost: number;
}
export interface Snapshot { at_ms: number; topo_version: number; assets: Record<string, AssetState>; customers: CustomerState[]; warnings: Warning[]; kpis: Kpis; predictions_at_ms: number; took_us: number }

export type PredictionKind = "forecast" | "peer_outlier" | "runtime";
export interface Prediction {
  asset: string; point: string; class: string; kind: PredictionKind; tlp_at_eta: Tlp; eta_min: number; text: string;
  current: number; threshold: number; history: number[]; forecast: number[]; upper: number[]; lower: number[];
}
export interface PredictionSet { at_ms: number; fitted: number; took_ms: number; horizon_min: number; items: Prediction[] }

export interface TreeNode {
  id: string; name: string; kind: AssetKind; tlp: Tlp; reason?: string; eta_min?: number; via?: string;
  customers?: string[]; gpus: number; size: number; children: TreeNode[];
}

export interface Reading { point: string; class: string; label: string; unit: string; forecast: boolean; subject: string; v?: number; t?: number; q?: number; tlp?: Tlp; state?: string; text: string }
export interface AssetDetail { asset: Asset; kind_label: string; state?: AssetState; readings: Reading[]; upstream?: TreeNode; downstream?: TreeNode; tenants: { id: string; name: string; tier: string }[] }

export interface StageDef { key: string; name: string; kind: "warn" | "fail" | "escalate" | "mitigate" | "restore"; minutes: number; detail: string; expect: string }
export interface ScenarioDef { id: string; name: string; domain: string; summary: string; default_target: string; stages: StageDef[]; candidates: { id: string; name: string }[] }
export interface RunView {
  id: number; scenario: string; name: string; domain: string; target: string; target_name: string; stage: number; auto: boolean; done: boolean;
  stage_started_ms: number; started_ms: number; ended_ms?: number; progress: number; elapsed_min: number; stages: StageDef[];
}
export interface Clock { sim_ms: number; speed: number; paused: boolean }
