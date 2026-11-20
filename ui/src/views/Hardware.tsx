import React, { useState } from "react";
import { Icon } from "@design";
import type { Ctx } from "../App.tsx";
import { stateOf } from "../lib/model.ts";
import { TLP, fmtEta } from "../lib/tlp.ts";
import type { Asset, AssetKind } from "../lib/types.ts";
import { KIND_ICON } from "../components/kinds.ts";
import { Panel, Seg, TlpDot } from "../components/ui.tsx";

type ColorBy = "state" | "customer";

export function Hardware({ c }: { c: Ctx }) {
  const [by, setBy] = useState<ColorBy>("state");
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Hardware</h1>
          <p>The hall as it is laid out: plant, pods, rows, racks and every compute tray. Click anything for readings and dependencies.</p>
        </div>
        <Seg value={by} onChange={setBy} options={[{ value: "state", label: "Color by state", icon: "traffic-cone" }, { value: "customer", label: "Color by customer", icon: "users" }]} />
      </div>
      <Plant c={c} />
      <Panel title="Data hall 1" icon="warehouse">
        <HallMap c={c} by={by} />
      </Panel>
      <Legend c={c} by={by} />
    </div>
  );
}

function Legend({ c, by }: { c: Ctx; by: ColorBy }) {
  return (
    <div className="tw-legend" style={{ padding: "0 4px" }}>
      {by === "state"
        ? (["green", "yellow", "orange", "red", "unknown"] as const).map((t) => (
            <span key={t}>
              <TlpDot tlp={t} /> {TLP[t].label}
            </span>
          ))
        : c.topo.customers.map((cu) => (
            <span key={cu.id}>
              <i style={{ width: 10, height: 10, borderRadius: 3, background: cu.color, display: "inline-block" }} /> {cu.name} <small>{cu.tier}</small>
            </span>
          ))}
    </div>
  );
}

function Tile({ a, c, small, label }: { a: Asset; c: Ctx; small?: boolean; label?: string }) {
  const s = stateOf(c.snap, a.id);
  const t = TLP[s.tlp];
  return (
    <button
      className={"tw-tile" + (small ? " sm" : "") + (c.selected === a.id ? " sel" : "")}
      onClick={() => c.select(a.id)}
      title={`${a.name}: ${t.label}${s.reason ? " — " + s.reason : ""}`}
      style={{ background: s.tlp === "green" ? "var(--surface-card)" : t.bg, borderColor: s.tlp === "green" ? "var(--border-default)" : t.line }}
    >
      <Icon name={KIND_ICON[a.kind] ?? "box"} size={small ? 12 : 14} color={s.tlp === "green" ? "var(--text-secondary)" : t.fg} />
      <span>{label ?? a.name}</span>
      {s.tlp === "yellow" && s.eta_min !== undefined && <em>{fmtEta(s.eta_min)}</em>}
      {s.redundancy_lost && s.tlp === "green" && <Icon name="shield-alert" size={11} color="var(--amber-600)" />}
    </button>
  );
}

function Plant({ c }: { c: Ctx }) {
  const of = (...k: AssetKind[]) => c.topo.assets.filter((a) => k.includes(a.kind));
  const group = (title: string, icon: string, list: Asset[]) => (
    <div className="tw-plant-g">
      <div className="tw-plant-h">
        <Icon name={icon} size={14} /> {title}
      </div>
      <div className="tw-plant-tiles">
        {list.map((a) => (
          <Tile key={a.id} a={a} c={c} small />
        ))}
      </div>
    </div>
  );
  return (
    <div className="tw-plant">
      {group("Grid and generation", "utility-pole", of("grid", "utility_feed", "generator", "ats", "bess"))}
      {group("Transformation and UPS", "battery-charging", of("transformer", "ups", "switchboard"))}
      {group("Heat rejection", "fan", of("cooling_tower", "facility_pump", "facility_water"))}
      {group("Air side", "air-vent", of("chiller", "crah"))}
      {group("Network core", "network", of("spine_switch"))}
    </div>
  );
}

/** Pods → rows → racks (with their trays), CDUs, busways and leafs. */
export function HallMap({ c, by = "state", compact }: { c: Ctx; by?: ColorBy; compact?: boolean }) {
  const pods = c.topo.assets.filter((a) => a.kind === "pod");
  const kids = (id: string, k: AssetKind) => (c.ix.children.get(id) ?? []).filter((a) => a.kind === k);
  return (
    <div className={"tw-hall" + (compact ? " compact" : "")}>
      {pods.map((p) => (
        <div key={p.id} className="tw-pod">
          <div className="tw-pod-h">{p.name}</div>
          {kids(p.id, "row").map((row) => (
            <div key={row.id} className="tw-row">
              <div className="tw-row-h">
                <b>{row.name}</b>
                {!compact &&
                  kids(row.id, "cdu").map((cd) => <Tile key={cd.id} a={cd} c={c} small label={cd.name.replace("CDU ", "CDU ")} />)}
                {!compact && kids(row.id, "leaf_switch").map((lf) => <Tile key={lf.id} a={lf} c={c} small />)}
              </div>
              <div className="tw-racks">
                {kids(row.id, "rack").map((r) => (
                  <RackTile key={r.id} r={r} c={c} by={by} compact={compact} />
                ))}
              </div>
              {!compact && (
                <div className="tw-busways">
                  {kids(row.id, "busway").map((bw) => {
                    const s = stateOf(c.snap, bw.id);
                    return (
                      <button key={bw.id} className="tw-bus" onClick={() => c.select(bw.id)} title={`${bw.name}: ${TLP[s.tlp].label}`} style={{ background: TLP[s.tlp].line }}>
                        <span>{bw.name}</span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function RackTile({ r, c, by, compact }: { r: Asset; c: Ctx; by: ColorBy; compact?: boolean }) {
  const s = stateOf(c.snap, r.id);
  const tenant = c.ix.rackTenant.get(r.id);
  const trays = (c.ix.children.get(r.id) ?? []).filter((a) => a.kind === "compute_tray");
  const t = TLP[s.tlp];
  const bg = by === "customer" ? (tenant?.color ?? "var(--neutral-300)") + "33" : s.tlp === "green" ? "var(--surface-card)" : t.bg;
  const border = by === "customer" ? tenant?.color ?? "var(--border-default)" : s.tlp === "green" ? "var(--border-default)" : t.line;
  return (
    <button className={"tw-rack" + (c.selected === r.id ? " sel" : "")} onClick={() => c.select(r.id)} style={{ background: bg, borderColor: border }} title={`${r.name} · ${tenant?.name ?? "unallocated"}\n${t.label}${s.reason ? ": " + s.reason : ""}`}>
      <div className="tw-rack-h">
        <span>{r.id.split("-").pop()}</span>
        {by === "state" && tenant && <i style={{ background: tenant.color }} />}
        {by === "customer" && <TlpDot tlp={s.tlp} size={7} />}
      </div>
      {!compact && (
        <div className="tw-trays">
          {trays.map((tr) => {
            const ts = stateOf(c.snap, tr.id);
            const own = c.ix.trayTenant.get(tr.id);
            return (
              <span
                key={tr.id}
                onClick={(e) => (e.stopPropagation(), c.select(tr.id))}
                title={`${tr.name}: ${TLP[ts.tlp].label}`}
                style={{ background: by === "customer" ? own?.color ?? tenant?.color ?? "var(--neutral-200)" : ts.tlp === "green" ? "var(--neutral-200)" : TLP[ts.tlp].line }}
              />
            );
          })}
        </div>
      )}
      {!compact && s.tlp === "yellow" && s.eta_min !== undefined && <em>{fmtEta(s.eta_min)}</em>}
    </button>
  );
}
