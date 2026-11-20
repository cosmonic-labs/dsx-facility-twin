import React, { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "@design";
import { TLP, fmtEta } from "../lib/tlp.ts";
import type { AssetKind, Tlp } from "../lib/types.ts";
import { KIND_ICON } from "./kinds.ts";

/*
 * GraphCanvas — the canvas that draws the facility graphs.
 *
 * Cards with a tinted header and port dots, bezier edges with an animated dash on the selected
 * node, a dotted light canvas or the dark "cosmic" one with a starfield, and the three treatments
 * Signal (bezier), Schematic (orthogonal, more detail) and Constellation (radial). Colors carry
 * TLP state.
 */

export interface GNode {
  id: string;
  title: string;
  sub?: string;
  kind: AssetKind;
  col: number;
  tlp: Tlp;
  eta?: number;
  /** Short facts shown as rows in the Schematic treatment. */
  rows?: { k: string; v: string; tlp?: Tlp }[];
  /** For collapsed groups: number of members and whether it can expand. */
  members?: number;
  expandable?: boolean;
  expanded?: boolean;
  badge?: string;
  ring?: string; // customer color strip
}
export interface GEdge {
  from: string;
  to: string;
  tlp: Tlp;
  label?: string;
}
export type Treatment = "signal" | "schematic" | "constellation";

const COL_W = 260;
const CARD_W = 196;

const EDGE_GREEN = { light: "#1fa971", dark: "#2bbd85" };

function cardH(n: GNode, detailed: boolean) {
  return 46 + (detailed ? (n.rows?.length ?? 0) * 19 + (n.rows?.length ? 8 : 0) : 0);
}

export function GraphCanvas({
  nodes,
  edges,
  selected,
  onSelect,
  onToggle,
  height = 620,
  initialTreatment = "signal",
  focus,
}: {
  nodes: GNode[];
  edges: GEdge[];
  selected?: string | null;
  onSelect?: (id: string | null) => void;
  onToggle?: (id: string) => void;
  height?: number;
  initialTreatment?: Treatment;
  focus?: string | null;
}) {
  const [treatment, setTreatment] = useState<Treatment>(initialTreatment);
  const [dark, setDark] = useState(false);
  const [view, setView] = useState({ x: 24, y: 24, k: 0.85 });
  const drag = useRef<{ x0: number; y0: number; vx0: number; vy0: number; moved: boolean } | null>(null);
  const host = useRef<HTMLDivElement>(null);
  const detailed = treatment === "schematic";
  const radial = treatment === "constellation";

  // ---- layout: columns, then stack; constellation puts columns on rings ----
  const pos = useMemo(() => {
    const cols = new Map<number, GNode[]>();
    nodes.forEach((n) => cols.set(n.col, [...(cols.get(n.col) ?? []), n]));
    const out = new Map<string, { x: number; y: number; h: number }>();
    const colKeys = [...cols.keys()].sort((a, b) => a - b);
    if (radial) {
      const cx = 620,
        cy = 460;
      colKeys.forEach((c, ci) => {
        const list = cols.get(c)!;
        const r = 70 + ci * 150;
        list.forEach((n, i) => {
          const a = (i / list.length) * Math.PI * 2 - Math.PI / 2 + ci * 0.35;
          out.set(n.id, { x: cx + Math.cos(a) * r - CARD_W / 2, y: cy + Math.sin(a) * r - 23, h: cardH(n, false) });
        });
      });
      return out;
    }
    const gap = detailed ? 16 : 12;
    const heights = colKeys.map((c) => cols.get(c)!.reduce((s, n) => s + cardH(n, detailed) + gap, 0));
    const tallest = Math.max(...heights, 0);
    colKeys.forEach((c, ci) => {
      let y = (tallest - heights[ci]) / 2;
      cols.get(c)!.forEach((n) => {
        const h = cardH(n, detailed);
        out.set(n.id, { x: ci * COL_W, y, h });
        y += h + gap;
      });
    });
    return out;
  }, [nodes, detailed, radial]);

  const bounds = useMemo(() => {
    let w = 0,
      h = 0;
    pos.forEach((p) => {
      w = Math.max(w, p.x + CARD_W);
      h = Math.max(h, p.y + p.h);
    });
    return { w, h };
  }, [pos]);

  const fit = () => {
    const el = host.current;
    if (!el) return;
    const k = Math.min((el.clientWidth - 48) / Math.max(bounds.w, 1), (height - 48) / Math.max(bounds.h, 1), 1.1);
    setView({ x: (el.clientWidth - bounds.w * k) / 2, y: (height - bounds.h * k) / 2, k: Math.max(k, 0.25) });
  };
  useEffect(fit, [bounds.w, bounds.h, treatment]);
  useEffect(() => {
    if (!focus || !host.current) return;
    const p = pos.get(focus);
    if (!p) return;
    const k = Math.max(view.k, 0.8);
    setView({ k, x: host.current.clientWidth / 2 - (p.x + CARD_W / 2) * k, y: height / 2 - (p.y + p.h / 2) * k });
  }, [focus, pos]);

  const selEdges = new Set(edges.filter((e) => e.from === selected || e.to === selected).map((e) => e.from + ">" + e.to));
  const neighbors = new Set<string>();
  edges.forEach((e) => {
    if (e.from === selected) neighbors.add(e.to);
    if (e.to === selected) neighbors.add(e.from);
  });

  const edgePath = (a: { x: number; y: number }, b: { x: number; y: number }) => {
    if (radial) {
      const mx = (a.x + b.x) / 2,
        my = (a.y + b.y) / 2;
      return `M ${a.x} ${a.y} Q ${mx + (620 - mx) * 0.45} ${my + (460 - my) * 0.45} ${b.x} ${b.y}`;
    }
    if (detailed) {
      const mx = (a.x + b.x) / 2;
      return `M ${a.x} ${a.y} L ${mx} ${a.y} L ${mx} ${b.y} L ${b.x} ${b.y}`;
    }
    const dx = Math.max(40, Math.abs(b.x - a.x) * 0.45);
    return `M ${a.x} ${a.y} C ${a.x + dx} ${a.y} ${b.x - dx} ${b.y} ${b.x} ${b.y}`;
  };

  const edgeEls = edges.map((e) => {
    const a = pos.get(e.from),
      b = pos.get(e.to);
    if (!a || !b) return null;
    const p1 = { x: a.x + CARD_W, y: a.y + 23 },
      p2 = { x: b.x, y: b.y + 23 };
    const color = e.tlp === "green" ? (dark ? EDGE_GREEN.dark : EDGE_GREEN.light) : TLP[e.tlp].line;
    const hi = selEdges.has(e.from + ">" + e.to);
    const active = !selected || hi;
    return (
      <g key={e.from + ">" + e.to} opacity={active ? (e.tlp === "green" && !hi ? 0.55 : 1) : 0.1} style={{ transition: "opacity 140ms" }}>
        <path
          d={edgePath(p1, p2)}
          fill="none"
          stroke={color}
          strokeWidth={hi ? 2.6 : e.tlp === "green" ? 1.5 : 2.2}
          strokeLinecap="round"
          style={hi || e.tlp === "red" ? { strokeDasharray: "7 6", animation: "ins-dash 0.7s linear infinite" } : undefined}
        >
          <title>{e.label ?? ""}</title>
        </path>
        <circle cx={p2.x} cy={p2.y} r={hi ? 3.6 : 2.8} fill={color} />
      </g>
    );
  });

  const nodeEls = nodes.map((n) => {
    const p = pos.get(n.id)!;
    const c = TLP[n.tlp];
    const isSel = selected === n.id;
    const dim = selected && !isSel && !neighbors.has(n.id);
    const cardBg = dark ? "#16242d" : "#ffffff";
    const border = isSel ? "var(--accent)" : n.tlp === "green" ? (dark ? "#2a3d49" : "#e4e9eb") : c.line;
    const dot = (side: "l" | "r") => (
      <span style={{ position: "absolute", top: 19, [side === "l" ? "left" : "right"]: -4, width: 8, height: 8, borderRadius: 3, background: n.tlp === "green" ? (dark ? "#4f9be0" : "#2a74c7") : c.line, boxShadow: `0 0 0 3px ${cardBg}` }} />
    );
    return (
      <div
        key={n.id}
        data-node
        onClick={(e) => {
          e.stopPropagation();
          if (!drag.current?.moved) onSelect?.(n.id);
        }}
        style={{
          position: "absolute",
          left: p.x,
          top: p.y,
          width: CARD_W,
          minHeight: p.h,
          background: cardBg,
          border: `1.5px solid ${border}`,
          borderRadius: 10,
          boxShadow: isSel ? "0 0 0 3px rgba(104,91,199,0.22), var(--shadow-md)" : dark ? "0 3px 10px rgba(0,0,0,0.35)" : "var(--shadow-sm)",
          opacity: dim ? 0.28 : 1,
          transition: "opacity 140ms, box-shadow 140ms",
          cursor: "pointer",
          color: dark ? "#dfe7ec" : "var(--text-primary)",
        }}
        title={n.title}
      >
        {dot("l")}
        {dot("r")}
        <div style={{ display: "flex", alignItems: "center", gap: 7, padding: "7px 9px 2px", borderRadius: "9px 9px 0 0", background: n.tlp === "green" ? (dark ? "rgba(255,255,255,0.04)" : "#f7f9fb") : dark ? c.line + "26" : c.bg }}>
          <Icon name={KIND_ICON[n.kind] ?? "box"} size={14} color={n.tlp === "green" ? (dark ? "#7fb2ec" : "#2a74c7") : c.fg} />
          <span style={{ fontWeight: 700, fontSize: 12.5, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", flex: 1 }}>{n.title}</span>
          {n.tlp !== "green" && (
            <span style={{ fontSize: 9.5, fontWeight: 800, letterSpacing: "0.04em", color: dark ? c.line : c.fg }}>{n.tlp === "yellow" && n.eta !== undefined ? fmtEta(n.eta) : c.short}</span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "2px 9px 7px", fontFamily: "var(--font-mono)", fontSize: 10.5, color: dark ? "#8fa3b3" : "var(--text-muted)" }}>
          {n.ring && <i style={{ width: 8, height: 8, borderRadius: 2, background: n.ring }} />}
          <span style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", flex: 1 }}>{n.sub}</span>
          {n.expandable && (
            <button
              onClick={(e) => (e.stopPropagation(), onToggle?.(n.id))}
              style={{ border: "1px solid var(--border-default)", background: dark ? "#1e3140" : "#fff", color: "inherit", borderRadius: 6, fontSize: 10, padding: "0 5px", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: 3 }}
              title={n.expanded ? "Collapse" : "Expand"}
            >
              <Icon name={n.expanded ? "minimize-2" : "maximize-2"} size={10} />
              {n.members ?? ""}
            </button>
          )}
        </div>
        {detailed && n.rows && n.rows.length > 0 && (
          <div style={{ borderTop: `1px solid ${dark ? "#21323d" : "#edf1f3"}`, padding: "4px 9px 6px" }}>
            {n.rows.map((r) => (
              <div key={r.k} style={{ display: "flex", fontSize: 11, lineHeight: "19px", gap: 6 }}>
                <span style={{ color: dark ? "#8fa3b3" : "var(--text-secondary)", flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{r.k}</span>
                <span style={{ fontFamily: "var(--font-mono)", fontWeight: 600, color: r.tlp && r.tlp !== "green" ? TLP[r.tlp].line : undefined }}>{r.v}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  });

  const bg = dark
    ? { background: "radial-gradient(1100px 700px at 30% 20%, #18234a 0%, #0f1a2e 45%, #0a1119 100%)" }
    : { backgroundColor: "#f6f8fa", backgroundImage: "radial-gradient(circle, #d9e1e2 1px, transparent 1px)", backgroundSize: "22px 22px" };

  const treatments: { id: Treatment; name: string; icon: string; tip: string }[] = [
    { id: "signal", name: "Signal", icon: "spline", tip: "Compact cards, smooth bezier edges" },
    { id: "schematic", name: "Schematic", icon: "git-commit-horizontal", tip: "Orthogonal wiring with live readings on each card" },
    { id: "constellation", name: "Constellation", icon: "orbit", tip: "Radial: upstream at the center, consumers on the outer rings" },
  ];

  return (
    <div
      ref={host}
      className="tw-canvas"
      onMouseDown={(e) => {
        if ((e.target as Element).closest("[data-node],button")) return;
        drag.current = { x0: e.clientX, y0: e.clientY, vx0: view.x, vy0: view.y, moved: false };
      }}
      onMouseMove={(e) => {
        const d = drag.current;
        if (!d) return;
        if (Math.abs(e.clientX - d.x0) + Math.abs(e.clientY - d.y0) > 3) d.moved = true;
        setView((v) => ({ ...v, x: d.vx0 + e.clientX - d.x0, y: d.vy0 + e.clientY - d.y0 }));
      }}
      onMouseUp={() => {
        const moved = drag.current?.moved;
        drag.current = null;
        if (!moved) onSelect?.(null);
      }}
      onMouseLeave={() => (drag.current = null)}
      onWheel={(e) => {
        const r = host.current!.getBoundingClientRect();
        const mx = e.clientX - r.left,
          my = e.clientY - r.top;
        setView((v) => {
          const k = Math.min(2.2, Math.max(0.2, v.k * (e.deltaY < 0 ? 1.1 : 0.9)));
          return { k, x: mx - ((mx - v.x) / v.k) * k, y: my - ((my - v.y) / v.k) * k };
        });
      }}
      style={{ height, border: dark ? "1px solid #21323d" : "1px solid var(--border-subtle)", ...bg }}
    >
      {dark && (
        <svg width="100%" height="100%" style={{ position: "absolute", inset: 0, pointerEvents: "none" }}>
          {Array.from({ length: 46 }).map((_, i) => (
            <circle key={i} cx={((i * 97.3) % 100) + "%"} cy={((i * 53.7) % 100) + "%"} r={i % 5 === 0 ? 1.4 : 0.8} fill="#fff" opacity={0.08 + (i % 7) * 0.02} />
          ))}
        </svg>
      )}
      <div style={{ position: "absolute", left: 0, top: 0, transform: `translate(${view.x}px, ${view.y}px) scale(${view.k})`, transformOrigin: "0 0", width: bounds.w + 40, height: bounds.h + 40 }}>
        <svg width={bounds.w + 40} height={bounds.h + 40} style={{ position: "absolute", left: 0, top: 0, overflow: "visible" }}>
          {edgeEls}
        </svg>
        {nodeEls}
      </div>
      <div className="tw-canvas-bar" style={{ left: 12, top: 12, background: dark ? "rgba(12,21,28,0.9)" : "rgba(255,255,255,0.95)" }}>
        {treatments.map((t) => (
          <button key={t.id} title={t.tip} className={treatment === t.id ? "on" : ""} onClick={() => setTreatment(t.id)} style={{ color: dark ? "#aab8c1" : undefined }}>
            <Icon name={t.icon} size={13} />
            {t.name}
          </button>
        ))}
      </div>
      <div className="tw-canvas-bar" style={{ right: 12, top: 12, background: dark ? "rgba(12,21,28,0.9)" : "rgba(255,255,255,0.95)" }}>
        <button onClick={() => setView((v) => ({ ...v, k: Math.min(2.2, v.k * 1.2) }))} title="Zoom in" style={{ color: dark ? "#aab8c1" : undefined }}>
          <Icon name="zoom-in" size={14} />
        </button>
        <button onClick={() => setView((v) => ({ ...v, k: Math.max(0.2, v.k / 1.2) }))} title="Zoom out" style={{ color: dark ? "#aab8c1" : undefined }}>
          <Icon name="zoom-out" size={14} />
        </button>
        <button onClick={fit} title="Fit" style={{ color: dark ? "#aab8c1" : undefined }}>
          <Icon name="scan" size={14} />
        </button>
        <button onClick={() => setDark(!dark)} title="Canvas theme" style={{ color: dark ? "#aab8c1" : undefined }}>
          <Icon name={dark ? "sun" : "moon-star"} size={14} />
        </button>
      </div>
      <div className="tw-canvas-zoom" style={{ color: dark ? "#8fa3b3" : "var(--text-muted)" }}>{Math.round(view.k * 100)}%</div>
    </div>
  );
}
