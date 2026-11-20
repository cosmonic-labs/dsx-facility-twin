import React from "react";
import { Icon } from "@design";
import { TLP, fmtEta, money } from "../lib/tlp.ts";
import type { GpuCounts, Tlp } from "../lib/types.ts";

export function TlpDot({ tlp, size = 10, pulse }: { tlp: Tlp; size?: number; pulse?: boolean }) {
  return (
    <span
      className={pulse && tlp !== "green" ? "tw-pulse" : undefined}
      style={{ width: size, height: size, borderRadius: 3, background: TLP[tlp].line, display: "inline-block", flex: "0 0 auto", ["--pulse" as string]: TLP[tlp].line }}
      title={TLP[tlp].label}
    />
  );
}

export function TlpChip({ tlp, label, size = "md" }: { tlp: Tlp; label?: string; size?: "sm" | "md" }) {
  const c = TLP[tlp];
  return (
    <span className="tw-chip" style={{ background: c.bg, color: c.fg, borderColor: c.line + "55", fontSize: size === "sm" ? 10.5 : 11.5 }}>
      <TlpDot tlp={tlp} size={size === "sm" ? 7 : 8} />
      {label ?? c.short}
    </span>
  );
}

export function EtaBadge({ eta, tlp = "yellow" }: { eta?: number; tlp?: Tlp }) {
  if (eta === undefined || eta === null) return null;
  return (
    <span className="tw-eta" style={{ color: TLP[tlp].fg, background: TLP[tlp].bg }}>
      <Icon name="timer" size={12} /> {fmtEta(eta)}
    </span>
  );
}

export function Panel({ title, icon, actions, children, pad = true, style }: { title?: React.ReactNode; icon?: string; actions?: React.ReactNode; children: React.ReactNode; pad?: boolean; style?: React.CSSProperties }) {
  return (
    <section className="tw-panel" style={style}>
      {title && (
        <header className="tw-panel-h">
          {icon && <Icon name={icon} size={16} color="var(--accent)" />}
          <h3>{title}</h3>
          <div style={{ marginLeft: "auto", display: "flex", gap: 8, alignItems: "center" }}>{actions}</div>
        </header>
      )}
      <div style={pad ? { padding: 16 } : undefined}>{children}</div>
    </section>
  );
}

export function Kpi({ label, value, sub, tlp, icon, onClick }: { label: string; value: React.ReactNode; sub?: React.ReactNode; tlp?: Tlp; icon: string; onClick?: () => void }) {
  const c = tlp ? TLP[tlp] : null;
  return (
    <button className="tw-kpi" onClick={onClick} style={{ cursor: onClick ? "pointer" : "default", borderTopColor: c?.line ?? "var(--border-subtle)" }}>
      <div className="tw-kpi-l">
        <span className="tw-kpi-i" style={{ background: c?.bg ?? "var(--accent-soft)", color: c?.fg ?? "var(--accent-soft-fg)" }}>
          <Icon name={icon} size={15} />
        </span>
        {label}
      </div>
      <div className="tw-kpi-v">{value}</div>
      {sub && <div className="tw-kpi-s">{sub}</div>}
    </button>
  );
}

export function GpuBar({ g, total, height = 8 }: { g: GpuCounts; total?: number; height?: number }) {
  const t = total ?? g.green + g.yellow + g.orange + g.red + g.unknown;
  const seg = (n: number, tlp: Tlp) => (n > 0 ? <span key={tlp} title={`${n} GPUs ${TLP[tlp].label}`} style={{ width: `${(n / Math.max(t, 1)) * 100}%`, background: TLP[tlp].line }} /> : null);
  return (
    <div className="tw-gpubar" style={{ height }}>
      {seg(g.red, "red")}
      {seg(g.orange, "orange")}
      {seg(g.yellow, "yellow")}
      {seg(g.unknown, "unknown")}
      {seg(g.green, "green")}
    </div>
  );
}

export function Money({ v, tlp }: { v: number; tlp?: Tlp }) {
  return <span style={{ fontVariantNumeric: "tabular-nums", color: tlp && v > 0 ? TLP[tlp].fg : undefined, fontWeight: 600 }}>{money(v)}/hr</span>;
}

/** History + forecast + interval + threshold, the way a planner reads it. */
export function ForecastChart({ history, forecast, upper, lower, threshold, width = 320, height = 90, unit }: { history: number[]; forecast?: number[]; upper?: number[]; lower?: number[]; threshold?: number; width?: number; height?: number; unit?: string }) {
  const f = forecast ?? [];
  const all = [...history, ...f, ...(upper ?? []), ...(lower ?? []), ...(threshold ? [threshold] : [])].filter((x) => Number.isFinite(x));
  if (all.length < 2) return <div style={{ height, color: "var(--text-muted)", fontSize: 12 }}>No history yet</div>;
  const lo = Math.min(...all), hi = Math.max(...all);
  const pad = (hi - lo) * 0.08 || 1;
  const n = history.length + f.length;
  const x = (i: number) => (i / Math.max(n - 1, 1)) * (width - 8) + 4;
  const y = (v: number) => height - 6 - ((v - (lo - pad)) / (hi - lo + 2 * pad)) * (height - 12);
  const path = (vals: number[], off: number) => vals.map((v, i) => `${i ? "L" : "M"}${x(i + off).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const h0 = history.length - 1;
  const band =
    upper && lower && upper.length
      ? `M${x(h0)},${y(history[h0])} ` + upper.map((v, i) => `L${x(h0 + 1 + i)},${y(v)}`).join(" ") + " " + [...lower].reverse().map((v, i) => `L${x(h0 + lower.length - i)},${y(v)}`).join(" ") + " Z"
      : "";
  return (
    <svg width={width} height={height} style={{ display: "block", overflow: "visible" }}>
      {threshold !== undefined && Number.isFinite(threshold) && threshold !== 0 && (
        <g>
          <line x1={0} x2={width} y1={y(threshold)} y2={y(threshold)} stroke="#f0712c" strokeDasharray="4 4" strokeWidth={1.2} />
          <text x={width - 2} y={y(threshold) - 4} fontSize={10} fill="#9a3d0b" textAnchor="end">
            limit {threshold.toFixed(1)} {unit}
          </text>
        </g>
      )}
      {band && <path d={band} fill="#685bc71f" />}
      <path d={path(history, 0)} fill="none" stroke="var(--neutral-600)" strokeWidth={1.6} />
      {f.length > 0 && <path d={`M${x(h0)},${y(history[h0])} ` + path(f, h0 + 1).replace(/^M/, "L")} fill="none" stroke="var(--accent)" strokeWidth={2} strokeDasharray="5 3" />}
      <line x1={x(h0)} x2={x(h0)} y1={2} y2={height - 2} stroke="var(--border-default)" />
      <text x={x(h0) + 3} y={11} fontSize={10} fill="var(--text-muted)">now</text>
    </svg>
  );
}

export function Empty({ icon = "circle-check", title, children }: { icon?: string; title: string; children?: React.ReactNode }) {
  return (
    <div className="tw-empty">
      <Icon name={icon} size={28} color="var(--text-muted)" />
      <div style={{ fontWeight: 600 }}>{title}</div>
      {children && <div style={{ color: "var(--text-secondary)", maxWidth: 440 }}>{children}</div>}
    </div>
  );
}

export function Seg<V extends string>({ value, options, onChange }: { value: V; options: { value: V; label: string; icon?: string }[]; onChange: (v: V) => void }) {
  return (
    <div className="tw-seg">
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? "on" : ""} onClick={() => onChange(o.value)}>
          {o.icon && <Icon name={o.icon} size={13} />}
          {o.label}
        </button>
      ))}
    </div>
  );
}
