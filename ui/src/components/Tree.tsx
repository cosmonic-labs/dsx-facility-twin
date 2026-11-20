import React, { useMemo, useState } from "react";
import { Icon } from "@design";
import { TLP, rank } from "../lib/tlp.ts";
import type { Customer, TreeNode } from "../lib/types.ts";
import { EtaBadge, TlpDot } from "./ui.tsx";
import { KIND_ICON, kindLabel } from "./kinds.ts";

/**
 * Progressive-disclosure impact tree. Branches that carry a non-green state open by default;
 * healthy branches stay folded behind a count. Long sibling lists show the worst first and fold
 * the rest behind "show N more".
 */
export function ImpactTree({ root, onSelect, customers, selected, openDepth = 2, maxSiblings = 8 }: { root: TreeNode; onSelect?: (id: string) => void; customers?: Customer[]; selected?: string; openDepth?: number; maxSiblings?: number }) {
  const names = useMemo(() => Object.fromEntries((customers ?? []).map((c) => [c.id, c])), [customers]);
  return (
    <div className="tw-tree" role="tree">
      <Row node={root} depth={0} onSelect={onSelect} names={names} selected={selected} openDepth={openDepth} maxSiblings={maxSiblings} />
    </div>
  );
}

function hasTrouble(n: TreeNode): boolean {
  return n.tlp !== "green" || n.children.some(hasTrouble);
}

function Row({ node, depth, onSelect, names, selected, openDepth, maxSiblings }: { node: TreeNode; depth: number; onSelect?: (id: string) => void; names: Record<string, Customer>; selected?: string; openDepth: number; maxSiblings: number }) {
  const trouble = hasTrouble(node);
  const [open, setOpen] = useState(depth < 1 || (trouble && depth < openDepth + 2) || depth < openDepth && node.children.length <= 4);
  const [more, setMore] = useState(false);
  const kids = [...node.children].sort((a, b) => rank(b.tlp) - rank(a.tlp));
  const shown = more ? kids : kids.slice(0, maxSiblings);
  const hidden = node.size - node.children.length;
  const c = TLP[node.tlp];
  const can = node.children.length > 0;
  return (
    <div role="treeitem" aria-expanded={can ? open : undefined}>
      <div
        className={"tw-tree-row" + (selected === node.id ? " sel" : "")}
        style={{ paddingLeft: 8 + depth * 18, background: node.tlp !== "green" ? c.bg + "99" : undefined, borderLeftColor: c.line }}
        onClick={() => onSelect?.(node.id)}
      >
        <button className="tw-tree-tw" onClick={(e) => (e.stopPropagation(), setOpen(!open))} style={{ visibility: can ? "visible" : "hidden" }} aria-label={open ? "Collapse" : "Expand"}>
          <Icon name={open ? "chevron-down" : "chevron-right"} size={14} />
        </button>
        <TlpDot tlp={node.tlp} />
        <Icon name={KIND_ICON[node.kind] ?? "box"} size={14} color="var(--text-secondary)" />
        <span className="tw-tree-name">{node.name}</span>
        <span className="tw-tree-kind">{kindLabel(node.kind)}</span>
        {node.via && <span className="tw-tree-via">via {node.via}</span>}
        <EtaBadge eta={node.tlp === "yellow" ? node.eta_min : undefined} />
        {(node.customers ?? []).slice(0, 3).map((id) => (
          <span key={id} className="tw-cust" style={{ borderColor: names[id]?.color ?? "var(--border-default)" }}>
            <i style={{ background: names[id]?.color }} />
            {names[id]?.name ?? id}
          </span>
        ))}
        {node.reason && node.tlp !== "green" && <span className="tw-tree-reason" title={node.reason}>{node.reason}</span>}
        {!open && node.size > 0 && <span className="tw-tree-count">{node.size}</span>}
      </div>
      {open && (
        <div role="group">
          {shown.map((k) => (
            <Row key={k.id + (k.via ?? "")} node={k} depth={depth + 1} onSelect={onSelect} names={names} selected={selected} openDepth={openDepth} maxSiblings={maxSiblings} />
          ))}
          {kids.length > shown.length && (
            <button className="tw-tree-more" style={{ marginLeft: 30 + (depth + 1) * 18 }} onClick={() => setMore(true)}>
              Show {kids.length - shown.length} more (all {kids.slice(maxSiblings).every((k) => k.tlp === "green") ? "normal" : "mixed"})
            </button>
          )}
          {hidden > 0 && kids.length === 0 && (
            <div className="tw-tree-more" style={{ marginLeft: 30 + (depth + 1) * 18 }}>
              {hidden} further dependents
            </div>
          )}
        </div>
      )}
    </div>
  );
}
