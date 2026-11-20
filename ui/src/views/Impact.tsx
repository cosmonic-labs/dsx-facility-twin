import React, { useMemo, useState } from "react";
import { Icon } from "@design";
import type { Ctx } from "../App.tsx";
import { usePoll } from "../lib/api.ts";
import { stateOf } from "../lib/model.ts";
import { go } from "../lib/router.ts";
import { TLP, rank, worst } from "../lib/tlp.ts";
import type { Tlp, TreeNode } from "../lib/types.ts";
import { ImpactTree } from "../components/Tree.tsx";
import { CustomerImpact } from "../components/CustomerImpact.tsx";
import { Empty, Panel, Seg, TlpDot, EtaBadge } from "../components/ui.tsx";

export function Impact({ c }: { c: Ctx }) {
  const roots = useMemo(() => {
    const seen = new Set<string>();
    return (c.snap?.warnings ?? []).filter((w) => w.kind !== "redundancy_lost" && !seen.has(w.asset) && seen.add(w.asset));
  }, [c.snap]);
  const root = c.params.get("root") ?? roots[0]?.asset ?? "GRID";
  const [mode, setMode] = useState<"hardware" | "customer">((c.params.get("by") as "hardware") ?? "hardware");
  const [trays, setTrays] = useState(false);
  const [tree] = usePoll<TreeNode>(`/api/tree?root=${encodeURIComponent(root)}&dir=down&depth=7&trays=${trays ? 1 : 0}`, 4000);
  const ra = c.ix.byId.get(root);
  const rs = stateOf(c.snap, root);

  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Impact explorer</h1>
          <p>Start from a cause and walk what it touches. Branches with trouble open on their own; healthy ones stay folded.</p>
        </div>
        <Seg value={mode} onChange={setMode} options={[{ value: "hardware", label: "By hardware", icon: "server" }, { value: "customer", label: "By customer", icon: "users" }]} />
      </div>
      <div className="tw-impact">
        <Panel title="Causes" icon="crosshair" pad={false}>
          <div className="tw-wlist">
            {roots.length === 0 && <Empty title="No active causes">Pick any asset from Hardware, or start a scenario.</Empty>}
            {roots.map((w) => (
              <button key={w.id} className={"tw-w" + (w.asset === root ? " on" : "")} onClick={() => go("impact", { root: w.asset, by: mode })} style={{ borderLeftColor: TLP[w.tlp].line }}>
                <div className="tw-w-t">
                  <TlpDot tlp={w.tlp} />
                  <b>{w.asset_name}</b>
                  {w.eta_min !== undefined && w.eta_min > 0 && <EtaBadge eta={w.eta_min} />}
                </div>
                <div className="tw-w-d">{w.title}</div>
              </button>
            ))}
          </div>
        </Panel>
        <Panel
          title={
            <span>
              {mode === "hardware" ? "What depends on" : "Who feels"} <b>{ra?.name ?? root}</b>
            </span>
          }
          icon={mode === "hardware" ? "git-fork" : "users"}
          actions={
            mode === "hardware" ? (
              <label className="tw-check">
                <input type="checkbox" checked={trays} onChange={(e) => setTrays(e.target.checked)} /> show compute trays
              </label>
            ) : null
          }
        >
          {mode === "hardware" && tree && <ImpactTree root={tree} customers={c.topo.customers} onSelect={c.select} selected={c.selected ?? undefined} openDepth={2} />}
          {mode === "customer" && <CustomerImpact c={c} root={root} />}
          {!tree && <div style={{ color: "var(--text-muted)" }}>Loading…</div>}
          <div style={{ marginTop: 12, fontSize: 11.5, color: "var(--text-muted)", display: "flex", gap: 6, alignItems: "center" }}>
            <Icon name="info" size={13} /> Yellow: on trend to be impacted · Orange: degraded (power, cooling or performance limited) · Red: out. Computed by petgraph in the twin-impact Wasm component.
          </div>
        </Panel>
      </div>
    </div>
  );
}
