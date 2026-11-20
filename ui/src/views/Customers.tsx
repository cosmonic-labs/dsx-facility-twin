import React from "react";
import { Icon } from "@design";
import type { Ctx } from "../App.tsx";
import { usePoll } from "../lib/api.ts";
import { go } from "../lib/router.ts";
import { TLP, rank, worst } from "../lib/tlp.ts";
import type { Customer, CustomerState, Tlp, TreeNode } from "../lib/types.ts";
import { ImpactTree } from "../components/Tree.tsx";
import { EtaBadge, GpuBar, Money, Panel, TlpChip, Empty } from "../components/ui.tsx";

export function Customers({ c }: { c: Ctx }) {
  const id = c.params.get("id") ?? c.snap?.customers[0]?.id ?? c.topo.customers[0]?.id;
  const states = new Map((c.snap?.customers ?? []).map((s) => [s.id, s]));
  const list = [...c.topo.customers].sort((a, b) => rank(states.get(b.id)?.tlp ?? "green") - rank(states.get(a.id)?.tlp ?? "green") || a.name.localeCompare(b.name));
  const cu = c.topo.customers.find((x) => x.id === id);
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Customers</h1>
          <p>Each customer, the GPUs they are paying for, and the chain of equipment those GPUs depend on.</p>
        </div>
      </div>
      <div className="tw-cust-layout">
        <div className="tw-cust-list">
          {list.map((x) => {
            const s = states.get(x.id);
            const gpus = x.allocations.reduce((a, b) => a + b.gpus, 0);
            return (
              <button key={x.id} className={"tw-cust-card" + (x.id === id ? " on" : "")} onClick={() => go("customers", { id: x.id })} style={{ borderLeftColor: x.color }}>
                <div className="tw-cust-card-h">
                  <b>{x.name}</b>
                  <TlpChip tlp={s?.tlp ?? "green"} size="sm" />
                </div>
                <div className="tw-cust-card-m">
                  <span>{x.tier} · {gpus} GPUs · ${x.rate_per_gpu_hr.toFixed(2)}/GPU-hr</span>
                  {s?.tlp === "yellow" && <EtaBadge eta={s.eta_min} />}
                </div>
                {s && <GpuBar g={s.gpus} />}
              </button>
            );
          })}
        </div>
        {cu ? <CustomerDetail c={c} cu={cu} st={states.get(cu.id)} /> : <Empty title="No customer selected" />}
      </div>
    </div>
  );
}

function CustomerDetail({ c, cu, st }: { c: Ctx; cu: Customer; st?: CustomerState }) {
  const [d] = usePoll<{ racks: { rack: string; gpus: number; tree: TreeNode }[] }>(`/api/customer?id=${cu.id}`, 4000);
  const gpus = cu.allocations.reduce((a, b) => a + b.gpus, 0);
  const tlp: Tlp = st?.tlp ?? "green";
  const root: TreeNode | null = d
    ? {
        id: cu.id,
        name: cu.name,
        kind: "site",
        tlp: d.racks.reduce<Tlp>((w, r) => worst(w, r.tree.tlp), "green"),
        reason: st?.reason,
        eta_min: st?.eta_min,
        gpus,
        size: d.racks.reduce((s, r) => s + 1 + r.tree.size, 0),
        children: d.racks.map((r) => ({ ...r.tree, via: `${r.gpus} GPUs` })),
      }
    : null;
  const names = new Map(c.topo.assets.map((a) => [a.id, a.name]));
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, minWidth: 0 }}>
      <Panel title={cu.name} icon="building-2" actions={<TlpChip tlp={tlp} label={TLP[tlp].label} />}>
        <div className="tw-cust-kpis">
          <div><small>Tier · SLA</small><b>{cu.tier} · {cu.sla_pct}%</b></div>
          <div><small>GPUs</small><b>{gpus}</b></div>
          <div><small>Impacted now</small><b><Money v={st?.revenue_impacted_hr ?? 0} tlp="orange" /></b></div>
          <div><small>At risk</small><b><Money v={st?.revenue_at_risk_hr ?? 0} tlp="yellow" /></b></div>
          <div><small>When</small><b>{tlp === "yellow" ? <EtaBadge eta={st?.eta_min} /> : tlp === "green" ? "—" : "now"}</b></div>
        </div>
        {st && <GpuBar g={st.gpus} height={12} />}
        {st?.reason && (
          <p style={{ margin: "12px 0 0", color: "var(--text-secondary)", fontSize: 13 }}>
            <Icon name="info" size={13} /> {st.reason}
          </p>
        )}
        {st && st.causes.length > 0 && (
          <div style={{ marginTop: 8, display: "flex", gap: 6, flexWrap: "wrap", fontSize: 12, alignItems: "center" }}>
            <span style={{ color: "var(--text-muted)" }}>Root causes</span>
            {st.causes.map((x) => (
              <button key={x} className="tw-link" onClick={() => go("impact", { root: x })}>
                {names.get(x) ?? x}
              </button>
            ))}
          </div>
        )}
      </Panel>
      <Panel title="What their GPUs depend on" icon="git-branch" actions={<small style={{ color: "var(--text-muted)" }}>customer → racks → power, cooling, network</small>}>
        {root ? <ImpactTree root={root} customers={c.topo.customers} onSelect={(id) => (id === cu.id ? null : c.select(id))} selected={c.selected ?? undefined} openDepth={1} /> : <div style={{ color: "var(--text-muted)" }}>Loading…</div>}
      </Panel>
    </div>
  );
}
