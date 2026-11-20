import React, { useEffect, useState } from "react";
import { Button, Icon } from "@design";
import { api } from "../lib/api.ts";
import { TLP, fmtEta } from "../lib/tlp.ts";
import type { AssetDetail, Customer, Prediction, Topology } from "../lib/types.ts";
import { ImpactTree } from "./Tree.tsx";
import { EtaBadge, ForecastChart, TlpChip, TlpDot } from "./ui.tsx";
import { go } from "../lib/router.ts";

function SeriesSpark({ point, unit }: { point: string; unit: string }) {
  const [vals, setVals] = useState<number[] | null>(null);
  useEffect(() => {
    let alive = true;
    api<{ values: number[] }>(`/api/series?point=${encodeURIComponent(point)}&hours=12`).then((r) => alive && setVals(r.values)).catch(() => {});
    return () => {
      alive = false;
    };
  }, [point]);
  if (!vals || vals.length < 3) return null;
  return <ForecastChart history={vals} width={150} height={30} unit={unit} />;
}

export function AssetDrawer({ id, topo, customers, preds, onClose, onSelect }: { id: string; topo: Topology; customers: Customer[]; preds: Prediction[]; onClose: () => void; onSelect: (id: string) => void }) {
  const [d, setD] = useState<AssetDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setD(null);
    const load = () =>
      api<AssetDetail>(`/api/asset?id=${encodeURIComponent(id)}`)
        .then((x) => alive && (setD(x), setErr(null)))
        .catch((e) => alive && setErr(String(e.message ?? e)));
    load();
    const t = setInterval(load, 4000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [id]);
  const mine = preds.filter((p) => p.asset === id);
  const s = d?.state;
  const tlp = s?.tlp ?? "green";
  const names = new Map(topo.assets.map((a) => [a.id, a.name]));
  return (
    <aside className="tw-drawer">
      <header style={{ borderTop: `4px solid ${TLP[tlp].line}` }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <TlpDot tlp={tlp} size={12} pulse />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 700, fontSize: 16 }}>{d?.asset.name ?? id}</div>
            <div style={{ color: "var(--text-muted)", fontSize: 12, fontFamily: "var(--font-mono)" }}>
              {d?.kind_label} · {id}
            </div>
          </div>
          <Button variant="ghost" size="sm" icon="x" onClick={onClose} aria-label="Close" />
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 10, flexWrap: "wrap", alignItems: "center" }}>
          <TlpChip tlp={tlp} label={TLP[tlp].label} />
          {s?.redundancy_lost && <span className="tw-chip" style={{ background: "var(--amber-50)", color: "var(--amber-700)" }}><Icon name="shield-alert" size={12} /> Redundancy lost</span>}
          <EtaBadge eta={tlp === "yellow" ? s?.eta_min : undefined} />
        </div>
        {s?.reason && <p style={{ margin: "10px 0 0", fontSize: 13, color: "var(--text-secondary)" }}>{s.reason}</p>}
        {s?.causes && s.causes.length > 0 && !(s.causes.length === 1 && s.causes[0] === id) && (
          <div style={{ marginTop: 8, fontSize: 12, display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center" }}>
            <span style={{ color: "var(--text-muted)" }}>Root cause</span>
            {s.causes.map((c) => (
              <button key={c} className="tw-link" onClick={() => onSelect(c)}>
                {names.get(c) ?? c}
              </button>
            ))}
          </div>
        )}
      </header>
      <div className="tw-drawer-body">
        {err && <div className="tw-err">{err}</div>}
        {mine.length > 0 && (
          <section>
            <h4><Icon name="trending-up" size={14} /> augurs forecast</h4>
            {mine.map((p) => (
              <div key={p.point + p.kind} className="tw-pred">
                <div style={{ fontSize: 12.5, marginBottom: 6 }}>{p.text}</div>
                {p.history.length > 0 && <ForecastChart history={p.history} forecast={p.forecast} upper={p.upper} lower={p.lower} threshold={p.kind === "forecast" ? p.threshold : undefined} width={360} height={96} />}
              </div>
            ))}
          </section>
        )}
        {d && d.tenants.length > 0 && (
          <section>
            <h4><Icon name="users" size={14} /> Customers</h4>
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              {d.tenants.map((t) => (
                <button key={t.id} className="tw-cust big" onClick={() => go("customers", { id: t.id })} style={{ borderColor: customers.find((c) => c.id === t.id)?.color }}>
                  <i style={{ background: customers.find((c) => c.id === t.id)?.color }} /> {t.name} <small>{t.tier}</small>
                </button>
              ))}
            </div>
          </section>
        )}
        <section>
          <h4><Icon name="activity" size={14} /> Live readings</h4>
          <table className="tw-table tight">
            <tbody>
              {(d?.readings ?? []).map((r) => (
                <tr key={r.point}>
                  <td style={{ width: 14 }}>{r.tlp && <TlpDot tlp={r.q === 0 ? "unknown" : r.tlp} size={8} />}</td>
                  <td>{r.label}</td>
                  <td className="num" style={{ color: r.tlp && r.tlp !== "green" ? TLP[r.tlp].fg : undefined }}>
                    {r.v === undefined || r.v === null ? "—" : r.state ?? `${fmtNum(r.v)} ${r.unit}`}
                  </td>
                  <td style={{ width: 150 }}>{r.forecast && <SeriesSpark point={r.point} unit={r.unit} />}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        {d?.upstream && (
          <section>
            <h4><Icon name="git-branch" size={14} /> Depends on</h4>
            <ImpactTree root={d.upstream} onSelect={onSelect} customers={customers} openDepth={1} maxSiblings={6} />
          </section>
        )}
        {d?.downstream && d.downstream.children.length > 0 && (
          <section>
            <h4><Icon name="git-fork" size={14} /> Impacts</h4>
            <ImpactTree root={d.downstream} onSelect={onSelect} customers={customers} openDepth={1} maxSiblings={6} />
          </section>
        )}
      </div>
    </aside>
  );
}

export function fmtNum(v: number): string {
  const a = Math.abs(v);
  return a >= 1000 ? v.toFixed(0) : a >= 100 ? v.toFixed(0) : a >= 10 ? v.toFixed(1) : v.toFixed(2);
}
export { fmtEta };
