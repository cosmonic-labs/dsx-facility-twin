import React, { useEffect, useMemo, useState } from "react";
import { Button, Icon } from "@design";
import type { Ctx } from "../App.tsx";
import { usePoll } from "../lib/api.ts";
import { KEY_CLASSES, domainGraph, stateOf } from "../lib/model.ts";
import { TLP, rank } from "../lib/tlp.ts";
import type { Tlp } from "../lib/types.ts";
import { GraphCanvas } from "../components/GraphCanvas.tsx";
import { EtaBadge, Panel, TlpDot } from "../components/ui.tsx";
import { fmtNum } from "../components/AssetDrawer.tsx";
import { CustomerImpact } from "../components/CustomerImpact.tsx";
import { go } from "../lib/router.ts";

const COPY: Record<string, { title: string; blurb: string }> = {
  power: { title: "Power", blurb: "Grid → utility and generators → transfer switches → transformers → UPS modules → switchboards → busways → racks. A and B sides are independent (2N)." },
  cooling: { title: "Cooling", blurb: "Dry coolers and facility pumps feed the facility water loop; in-row CDUs move heat from the racks into it. Trim chillers and CRAHs handle the air side." },
  network: { title: "Network", blurb: "Spines and dual-homed leafs. Each compute tray needs one healthy leaf in its row." },
};

interface ClassDef {
  label: string;
  unit: string;
  states?: { value: number; text: string }[];
}

export function DomainView({ c, domain }: { c: Ctx; domain: "power" | "cooling" | "network" }) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [cfg] = usePoll<{ classes: Record<string, ClassDef> }>("/api/config", 600_000);
  const focus = c.params.get("focus");
  // Focusing a rack-level asset expands its row so it is visible.
  useEffect(() => {
    if (!focus) return;
    const a = c.ix.byId.get(focus);
    const row = a?.kind === "rack" || a?.kind === "tap_off" ? a.parent : a?.kind === "compute_tray" ? c.ix.byId.get(a.parent ?? "")?.parent : undefined;
    if (row) setExpanded((e) => new Set([...e, row]));
  }, [focus]);
  const g = useMemo(() => domainGraph(c.topo, c.ix, c.snap, domain, expanded), [c.topo, c.ix, c.snap, domain, expanded]);

  const pointIds = useMemo(() => {
    const ids: string[] = [];
    for (const n of g.nodes) {
      const a = c.ix.byId.get(n.id);
      if (!a) continue;
      for (const k of (KEY_CLASSES[a.kind] ?? []).slice(0, 3)) ids.push(`${a.id}.${k}`);
    }
    return ids.slice(0, 200);
  }, [g.nodes]);
  const [lkv] = usePoll<Record<string, { v: number; tlp: Tlp; q: number }>>(pointIds.length ? `/api/lkv?points=${pointIds.join(",")}` : null, 5000);
  for (const n of g.nodes) {
    const a = c.ix.byId.get(n.id);
    if (!a) continue;
    const rows = (KEY_CLASSES[a.kind] ?? []).slice(0, 3).flatMap((k) => {
      const l = lkv?.[`${a.id}.${k}`];
      const d = cfg?.classes[k];
      if (!l || !d) return [];
      const st = d.states?.find((s) => s.value === Math.round(l.v));
      return [{ k: d.label, v: l.q === 0 ? "no data" : st ? st.text : `${fmtNum(l.v)} ${d.unit}`, tlp: l.q === 0 ? ("unknown" as Tlp) : l.tlp }];
    });
    if (rows.length) n.rows = rows;
  }

  const toggle = (id: string) => {
    const a = c.ix.byId.get(id);
    const row = id.startsWith("group:") ? id.slice(6) : a?.parent;
    if (!row) return;
    setExpanded((e) => {
      const n = new Set(e);
      n.has(row) ? n.delete(row) : n.add(row);
      return n;
    });
  };
  const sel = c.selected && g.nodes.some((n) => n.id === c.selected) ? c.selected : null;
  const trouble = g.nodes.filter((n) => n.tlp !== "green").sort((a, b) => rank(b.tlp) - rank(a.tlp));
  const counts = g.nodes.reduce<Record<string, number>>((m, n) => ((m[n.tlp] = (m[n.tlp] ?? 0) + 1), m), {});
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>{COPY[domain].title}</h1>
          <p>{COPY[domain].blurb} Rows of racks are folded; open one with its expand button.</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <Button size="sm" variant="secondary" icon="maximize-2" onClick={() => setExpanded(new Set(c.topo.assets.filter((a) => a.kind === "row").map((a) => a.id)))}>
            Expand all rows
          </Button>
          <Button size="sm" variant="ghost" icon="minimize-2" onClick={() => setExpanded(new Set())}>
            Fold
          </Button>
        </div>
      </div>
      <div className="tw-domain">
        <div style={{ display: "flex", flexDirection: "column", gap: 16, minWidth: 0 }}>
        <GraphCanvas nodes={g.nodes} edges={g.edges} selected={sel} onSelect={(id) => (id?.startsWith("group:") ? toggle(id) : c.select(id))} onToggle={toggle} height={600} focus={focus} />
        {sel && c.ix.byId.has(sel) ? (
          <Panel title={<span>Customers depending on <b>{c.ix.byId.get(sel)!.name}</b></span>} icon="users" actions={<button className="tw-link" onClick={() => go("impact", { root: sel, by: "customer" })}>Open in impact explorer</button>}>
            <CustomerImpact c={c} root={sel} />
          </Panel>
        ) : (
          <div className="tw-hint"><Icon name="mouse-pointer-click" size={14} /> Click an element in the graph to see which customers depend on it and how they are doing.</div>
        )}
        </div>
        <Panel title="In this view" icon="list-filter" style={{ minWidth: 0 }}>
          <div className="tw-legend col">
            {(["red", "orange", "yellow", "unknown", "green"] as const).map((t) => (
              <span key={t}>
                <TlpDot tlp={t} /> {TLP[t].label} <b>{counts[t] ?? 0}</b>
              </span>
            ))}
          </div>
          <div style={{ height: 12 }} />
          {trouble.length === 0 && <div style={{ color: "var(--text-muted)", fontSize: 13 }}>Everything in this view is normal.</div>}
          {trouble.slice(0, 14).map((n) => {
            const s = stateOf(c.snap, n.id);
            return (
              <button key={n.id} className="tw-mini" onClick={() => (n.id.startsWith("group:") ? toggle(n.id) : c.select(n.id))} style={{ borderLeftColor: TLP[n.tlp].line }}>
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  <TlpDot tlp={n.tlp} />
                  <b>{n.title}</b>
                  <EtaBadge eta={n.tlp === "yellow" ? n.eta : undefined} />
                </div>
                {s.reason && <div className="tw-mini-d">{s.reason}</div>}
              </button>
            );
          })}
          <div style={{ marginTop: 12, fontSize: 11.5, color: "var(--text-muted)", display: "flex", gap: 6 }}>
            <Icon name="info" size={13} /> Edge color is the state of the upstream asset. Red edges animate.
          </div>
        </Panel>
      </div>
    </div>
  );
}
