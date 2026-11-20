import React, { useMemo } from "react";
import type { Ctx } from "../App.tsx";
import { usePoll } from "../lib/api.ts";
import { stateOf } from "../lib/model.ts";
import { go } from "../lib/router.ts";
import { rank, worst } from "../lib/tlp.ts";
import type { Tlp, TreeNode } from "../lib/types.ts";
import { ImpactTree } from "./Tree.tsx";
import { Empty, GpuBar } from "./ui.tsx";

/** Customers whose GPUs depend on `root`, as a tree: asset → customer → racks / trays. */
export interface CustomerImpactResult { tree: TreeNode | null; gpus: number; impacted: number; via: number; other: string[]; loading: boolean }

export function useCustomerImpact(c: Ctx, root: string | null): CustomerImpactResult {
  const [down] = usePoll<TreeNode>(root ? `/api/tree?root=${encodeURIComponent(root)}&dir=down&depth=8&trays=1` : null, 4000);
  return useMemo(() => {
    const ra = root ? c.ix.byId.get(root) : undefined;
    if (!root || !ra || !c.snap || !down || down.id !== root) return { tree: null, gpus: 0, impacted: 0, via: 0, other: [], loading: !!root };
    const reach = new Set<string>();
    const walk = (n: TreeNode) => {
      reach.add(n.id);
      n.children.forEach(walk);
    };
    walk(down);
    const rs = stateOf(c.snap, root);
    // Trouble is "through" the root when the allocation's causes include the root or what the root itself blames.
    const rootCauses = new Set<string>(rs.tlp === "green" ? [] : [root, ...(rs.causes ?? [])]);
    const through = (causes?: string[]) => !!causes?.some((x) => rootCauses.has(x));
    const other = new Set<string>();
    const children: TreeNode[] = [];
    let gpus = 0,
      impacted = 0,
      via = 0;
    for (const cu of c.topo.customers) {
      const kids: TreeNode[] = [];
      for (const al of cu.allocations) {
        const a = c.ix.byId.get(al.asset);
        if (!a) continue;
        const rackId = a.kind === "compute_tray" ? a.parent! : a.id;
        if (!(reach.has(al.asset) || reach.has(rackId) || root === rackId || root === al.asset)) continue;
        const s = stateOf(c.snap, al.asset);
        const trays: TreeNode[] = (c.ix.children.get(rackId) ?? [])
          .filter((t) => t.kind === "compute_tray" && (a.kind !== "compute_tray" || t.id === a.id) && stateOf(c.snap, t.id).tlp !== "green")
          .map((t) => {
            const ts = stateOf(c.snap, t.id);
            return { id: t.id, name: t.name, kind: t.kind, tlp: ts.tlp, reason: ts.reason, eta_min: ts.eta_min, gpus: 4, size: 0, children: [] };
          });
        const tlp = trays.reduce<Tlp>((w, t) => worst(w, t.tlp), s.tlp);
        kids.push({ id: al.asset, name: a.name, kind: a.kind, tlp, reason: s.reason, eta_min: s.eta_min, gpus: al.gpus, size: trays.length, children: trays, via: `${al.gpus} GPUs` });
        gpus += al.gpus;
        if (tlp !== "green") {
          impacted += al.gpus;
          const cs = [s.causes ?? [], ...trays.map((t) => stateOf(c.snap, t.id).causes ?? [])].flat();
          if (through(cs)) via += al.gpus;
          else cs.filter((x) => !reach.has(x) || x === al.asset || x === rackId).forEach((x) => other.add(x));
        }
      }
      if (!kids.length) continue;
      const cs = c.snap.customers.find((x) => x.id === cu.id);
      kids.sort((x, y) => rank(y.tlp) - rank(x.tlp));
      children.push({
        id: "cust:" + cu.id,
        name: cu.name,
        kind: "site",
        tlp: kids.reduce<Tlp>((w, k) => worst(w, k.tlp), "green"),
        reason: kids.some((k) => k.tlp !== "green") ? cs?.reason : `${kids.length} allocation${kids.length > 1 ? "s" : ""} depend on ${ra.name}; covered for now`,
        eta_min: cs?.eta_min,
        gpus: kids.reduce((s, k) => s + k.gpus, 0),
        size: kids.length,
        children: kids,
        via: `${cu.tier} · ${kids.reduce((s, k) => s + k.gpus, 0)} GPUs`,
        customers: [cu.id],
      });
    }
    children.sort((a, b) => rank(b.tlp) - rank(a.tlp) || b.gpus - a.gpus);
    return { tree: { id: root, name: ra.name, kind: ra.kind, tlp: rs.tlp, reason: rs.reason, eta_min: rs.eta_min, gpus, size: children.length, children }, gpus, impacted, via, other: [...other], loading: false };
  }, [down, c.snap, root]);
}

export function CustomerImpact({ c, root, compact }: { c: Ctx; root: string; compact?: boolean }) {
  const { tree, gpus, impacted, via, other, loading } = useCustomerImpact(c, root);
  const rs = stateOf(c.snap, root);
  const names = other.map((id) => c.ix.byId.get(id)?.name ?? id);
  if (loading || !tree) return <div style={{ color: "var(--text-muted)", fontSize: 13 }}>Tracing dependents…</div>;
  if (!tree.children.length)
    return (
      <Empty icon="users" title="No customer GPUs depend on this asset">
        It feeds infrastructure only, or nothing downstream is allocated.
      </Empty>
    );
  const g = { green: gpus - impacted, yellow: 0, orange: 0, red: 0, unknown: 0 };
  for (const cu of tree.children) for (const k of cu.children) if (k.tlp !== "green") g[k.tlp] += k.gpus;
  return (
    <div>
      {!compact && (
        <div style={{ display: "flex", gap: 16, alignItems: "center", marginBottom: 10, fontSize: 12.5, color: "var(--text-secondary)" }}>
          <span>
            <b style={{ color: "var(--text-primary)" }}>{tree.children.length}</b> customers · <b style={{ color: "var(--text-primary)" }}>{gpus.toLocaleString()}</b> GPUs depend on it
          </span>
          {rs.tlp !== "green" && (
            <span>
              <b style={{ color: via ? "#9a3d0b" : "var(--text-primary)" }}>{via.toLocaleString()}</b> GPUs affected through {tree.name}
            </span>
          )}
          {impacted - via > 0 && (
            <span title={names.join(", ")}>
              <b style={{ color: "var(--text-primary)" }}>{(impacted - via).toLocaleString()}</b> not green for other reasons
              {names.length > 0 && <> ({names.slice(0, 2).join(", ")}{names.length > 2 ? ` +${names.length - 2}` : ""})</>}
            </span>
          )}
          {impacted === 0 && <span>All of them are normal now</span>}
          <div style={{ flex: 1, maxWidth: 260 }}>
            <GpuBar g={g} />
          </div>
        </div>
      )}
      <ImpactTree root={tree} customers={c.topo.customers} onSelect={(id) => (id.startsWith("cust:") ? go("customers", { id: id.slice(5) }) : c.select(id))} selected={c.selected ?? undefined} openDepth={1} />
    </div>
  );
}
