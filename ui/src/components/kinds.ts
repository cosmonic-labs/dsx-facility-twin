import type { AssetKind } from "../lib/types.ts";

export const KIND_ICON: Partial<Record<AssetKind, string>> = {
  site: "factory", hall: "warehouse", pod: "boxes", row: "rows-3",
  grid: "utility-pole", utility_feed: "cable", generator: "fuel", ats: "toggle-right", transformer: "zap",
  ups: "battery-charging", bess: "battery-full", switchboard: "panel-top", busway: "minus", tap_off: "plug",
  power_shelf: "plug-zap", cooling_tower: "fan", facility_pump: "circle-gauge", facility_water: "waves",
  chiller: "snowflake", crah: "air-vent", cdu: "droplets", spine_switch: "network", leaf_switch: "router",
  rack: "server", nvlink_fabric: "share-2", compute_tray: "cpu",
};

const LABEL: Partial<Record<AssetKind, string>> = {
  site: "Site", hall: "Data hall", pod: "Pod", row: "Row", grid: "Grid", utility_feed: "Utility", generator: "Generator",
  ats: "ATS", transformer: "Transformer", ups: "UPS module", bess: "BESS", switchboard: "Switchboard", busway: "Busway",
  tap_off: "Tap-off", power_shelf: "Power shelf", cooling_tower: "Dry cooler", facility_pump: "FW pump",
  facility_water: "FW loop", chiller: "Chiller", crah: "CRAH", cdu: "CDU", spine_switch: "Spine", leaf_switch: "Leaf",
  rack: "Rack", nvlink_fabric: "NVLink", compute_tray: "Tray",
};
export const kindLabel = (k: AssetKind) => LABEL[k] ?? k;

/** Column order for the domain graphs (left → right = upstream → downstream). */
export const POWER_ORDER: AssetKind[] = ["grid", "utility_feed", "generator", "ats", "transformer", "ups", "bess", "switchboard", "busway", "tap_off", "rack"];
export const COOLING_ORDER: AssetKind[] = ["cooling_tower", "facility_pump", "facility_water", "chiller", "crah", "cdu", "rack"];
export const NETWORK_ORDER: AssetKind[] = ["spine_switch", "leaf_switch", "rack"];
