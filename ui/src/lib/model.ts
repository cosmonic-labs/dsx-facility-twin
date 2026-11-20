import type { GEdge, GNode } from "../components/GraphCanvas.tsx";
import { COOLING_ORDER, NETWORK_ORDER, POWER_ORDER } from "../components/kinds.ts";
import { worst } from "./tlp.ts";
import type { Asset, AssetKind, AssetState, Customer, Snapshot, Tlp, Topology } from "./types.ts";

export interface Index {
  byId: Map<string, Asset>;
  children: Map<string, Asset[]>;
  rackTenant: Map<string, Customer>;
  trayTenant: Map<string, Customer>;
  customers: Map<string, Customer>;
}

export function indexTopo(t: Topology): Index {
  const byId = new Map(t.assets.map((a) => [a.id, a]));
  const children = new Map<string, Asset[]>();
  for (const a of t.assets) if (a.parent) children.set(a.parent, [...(children.get(a.parent) ?? []), a]);
  const rackTenant = new Map<string, Customer>();
  const trayTenant = new Map<string, Customer>();
  for (const c of t.customers)
    for (const al of c.allocations) {
      const a = byId.get(al.asset);
      if (!a) continue;
      if (a.kind === "compute_tray") {
        trayTenant.set(a.id, c);
        if (a.parent && !rackTenant.has(a.parent)) rackTenant.set(a.parent, c);
      } else rackTenant.set(a.id, c);
    }
  return { byId, children, rackTenant, trayTenant, customers: new Map(t.customers.map((c) => [c.id, c])) };
}

export const GREEN: AssetState = { tlp: "green", own: "green", reason: "" };
export const stateOf = (s: Snapshot | null, id: string): AssetState => s?.assets[id] ?? GREEN;

export const KEY_CLASSES: Partial<Record<AssetKind, string[]>> = {
  grid: ["grid_frequency", "grid_dr_event"],
  utility_feed: ["util_status", "util_voltage"],
  generator: ["gen_status", "gen_output_kw"],
  ats: ["ats_position"],
  transformer: ["xfmr_load_kw", "winding_temp"],
  ups: ["ups_mode", "ups_load_pct", "battery_soc"],
  bess: ["bess_status", "bess_soc"],
  switchboard: ["active_power"],
  busway: ["active_power", "current"],
  tap_off: ["current", "breaker_status"],
  cooling_tower: ["ct_status", "approach_temp"],
  facility_pump: ["fwp_status", "fwp_vibration"],
  facility_water: ["fw_supply_temp", "fw_flow"],
  chiller: ["chiller_status", "chw_supply_temp"],
  crah: ["crah_status", "supply_air_temp"],
  cdu: ["cdu_supply_temp", "cdu_flow", "pump_vibration"],
  spine_switch: ["switch_status", "link_errors"],
  leaf_switch: ["switch_status", "link_errors"],
  rack: ["liquid_return_temp", "rack_power"],
};

export type Domain3 = "power" | "cooling" | "network";

/** Project the topology onto one domain, folding racks per row until a row is expanded. */
export function domainGraph(t: Topology, ix: Index, snap: Snapshot | null, domain: Domain3, expanded: Set<string>): { nodes: GNode[]; edges: GEdge[] } {
  const order: AssetKind[] = domain === "power" ? POWER_ORDER : domain === "cooling" ? COOLING_ORDER : NETWORK_ORDER;
  const col = (k: AssetKind) => order.indexOf(k);
  const rows = t.assets.filter((a) => a.kind === "row");
  const rowOf = (a: Asset): string | undefined => (a.kind === "rack" || a.kind === "tap_off" || a.kind === "busway" || a.kind === "cdu" || a.kind === "leaf_switch" ? a.parent : undefined);
  const visible = new Map<string, GNode>();
  const st = (id: string) => stateOf(snap, id);

  const addAsset = (a: Asset) => {
    const s = st(a.id);
    const tenant = a.kind === "rack" ? ix.rackTenant.get(a.id) : undefined;
    visible.set(a.id, {
      id: a.id,
      title: a.name,
      sub: tenant ? tenant.name : sub(a),
      kind: a.kind,
      col: col(a.kind),
      tlp: s.tlp,
      eta: s.eta_min,
      ring: tenant?.color,
      rows: [],
    });
  };

  for (const a of t.assets) {
    if (col(a.kind) < 0) continue;
    if (a.kind === "rack" || a.kind === "tap_off") {
      const r = rowOf(a);
      if (!r || !expanded.has(r)) continue;
    }
    if (domain === "cooling" && a.kind === "rack") {
      const r = rowOf(a);
      if (!r || !expanded.has(r)) continue;
    }
    addAsset(a);
  }
  // row groups for collapsed rows
  for (const r of rows) {
    if (expanded.has(r.id)) continue;
    const racks = (ix.children.get(r.id) ?? []).filter((a) => a.kind === "rack");
    const tlp = racks.reduce<Tlp>((w, a) => worst(w, st(a.id).tlp), "green");
    const eta = racks.map((a) => st(a.id)).filter((s) => s.tlp === "yellow" && s.eta_min !== undefined).map((s) => s.eta_min!)[0];
    const tenants = [...new Set(racks.map((a) => ix.rackTenant.get(a.id)?.name).filter(Boolean))];
    visible.set("group:" + r.id, {
      id: "group:" + r.id,
      title: `${r.name} racks`,
      sub: `${racks.length} racks · ${tenants.slice(0, 2).join(", ")}${tenants.length > 2 ? "…" : ""}`,
      kind: "rack",
      col: col("rack"),
      tlp,
      eta,
      members: racks.length,
      expandable: true,
      expanded: false,
      rows: [
        { k: "Out", v: String(racks.filter((a) => st(a.id).tlp === "red").length), tlp: "red" },
        { k: "Degraded", v: String(racks.filter((a) => st(a.id).tlp === "orange").length), tlp: "orange" },
        { k: "At risk", v: String(racks.filter((a) => st(a.id).tlp === "yellow").length), tlp: "yellow" },
      ],
    });
  }
  // expanded rows: mark busways/CDUs/leafs expandable to collapse again
  for (const n of visible.values()) {
    const a = ix.byId.get(n.id);
    if (!a) continue;
    if (a.kind === "busway" || a.kind === "cdu" || a.kind === "leaf_switch") {
      n.expandable = true;
      n.expanded = !!a.parent && expanded.has(a.parent);
    }
  }

  const edges: GEdge[] = [];
  const seen = new Set<string>();
  const add = (from: string, to: string, label: string) => {
    if (!visible.has(from) || !visible.has(to) || from === to) return;
    const k = from + ">" + to;
    if (seen.has(k)) return;
    seen.add(k);
    edges.push({ from, to, tlp: st(from).tlp === "unknown" ? "unknown" : visible.get(from)!.tlp, label });
  };
  const wanted = (d: string) =>
    domain === "power" ? d === "power" || d === "grid" : domain === "cooling" ? d === "cooling" : d === "network";
  for (const a of t.assets) {
    for (const r of a.requires ?? []) {
      if (!wanted(r.domain)) continue;
      for (const p of r.of) {
        let to = a.id;
        if (!visible.has(to) && a.kind === "rack" && a.parent) to = "group:" + a.parent;
        if (!visible.has(to) && a.kind === "tap_off" && a.parent) continue;
        let from = p;
        const pa = ix.byId.get(p);
        if (!visible.has(from) && pa?.kind === "tap_off" && pa.parent) from = pa.parent ? busFor(pa) : from;
        add(from, to, `${r.label} (needs ${r.need} of ${r.of.length})`);
      }
    }
  }
  // network: trays require leafs, racks don't. Draw leaf → row group / rack.
  if (domain === "network") {
    for (const a of t.assets.filter((x) => x.kind === "leaf_switch")) {
      const r = a.parent!;
      if (expanded.has(r)) for (const rk of (ix.children.get(r) ?? []).filter((x) => x.kind === "rack")) add(a.id, rk.id, "fabric uplinks");
      else add(a.id, "group:" + r, "fabric uplinks");
    }
  }
  // collapsed power: busway → row group
  if (domain === "power")
    for (const a of t.assets.filter((x) => x.kind === "busway"))
      if (a.parent && !expanded.has(a.parent)) add(a.id, "group:" + a.parent, "busway feeds rack taps");

  const nodes = [...visible.values()].filter((n) => n.kind === "bess" || edges.some((e) => e.from === n.id || e.to === n.id));
  return { nodes, edges };
}

function busFor(tap: Asset): string {
  // P1-R1-01-TAP-A -> BW-P1-R1-A
  const side = tap.id.split("-").pop();
  return `BW-${tap.parent}-${side}`;
}

function sub(a: Asset): string {
  const at = a.attrs ?? {};
  if (at.capacity_kw) return `${(at.capacity_kw / 1000).toFixed(at.capacity_kw >= 1000 ? 1 : 2)} MW rated`;
  if (at.current_limit_a) return `${at.current_limit_a} A`;
  if (at.design_flow_lpm) return `${at.design_flow_lpm} LPM design`;
  return a.id;
}
