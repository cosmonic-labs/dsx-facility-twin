import React, { useState } from "react";
import { Icon, SearchField } from "@design";
import type { Ctx } from "../App.tsx";
import { go } from "../lib/router.ts";
import { TLP, fmtEta } from "../lib/tlp.ts";
import type { Warning, WarningKind } from "../lib/types.ts";
import { Empty, ForecastChart, Seg, TlpChip } from "../components/ui.tsx";

const KIND: Record<WarningKind, { label: string; icon: string }> = {
  out: { label: "Out", icon: "circle-x" },
  degraded: { label: "Degraded", icon: "circle-alert" },
  prediction: { label: "Prediction", icon: "trending-up" },
  watch: { label: "Early warning", icon: "eye" },
  redundancy_lost: { label: "Redundancy lost", icon: "shield-alert" },
  data_quality: { label: "No data", icon: "wifi-off" },
};

export function Warnings({ c }: { c: Ctx }) {
  const [kind, setKind] = useState<string>(c.params.get("kind") ?? "all");
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const all = c.snap?.warnings ?? [];
  const list = all.filter((w) => (kind === "all" || w.kind === kind) && (!q || (w.title + w.detail + w.asset_name).toLowerCase().includes(q.toLowerCase())));
  const count = (k: string) => all.filter((w) => k === "all" || w.kind === k).length;
  const predFor = (w: Warning) => (c.preds?.items ?? []).find((p) => w.id.startsWith(p.point + ":pred"));
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Warnings &amp; predictions</h1>
          <p>Every root cause, early warning and augurs forecast, most severe first. Click one to open it where it is best seen.</p>
        </div>
        <SearchField value={q} onChange={(e: React.ChangeEvent<HTMLInputElement>) => setQ(e.target.value)} placeholder="Filter" />
      </div>
      <Seg
        value={kind}
        onChange={setKind}
        options={[
          { value: "all", label: `All ${count("all")}` },
          { value: "out", label: `Out ${count("out")}`, icon: "circle-x" },
          { value: "degraded", label: `Degraded ${count("degraded")}`, icon: "circle-alert" },
          { value: "prediction", label: `Predictions ${count("prediction")}`, icon: "trending-up" },
          { value: "watch", label: `Early warnings ${count("watch")}`, icon: "eye" },
          { value: "redundancy_lost", label: `Redundancy ${count("redundancy_lost")}`, icon: "shield-alert" },
          { value: "data_quality", label: `No data ${count("data_quality")}`, icon: "wifi-off" },
        ]}
      />
      <div className="tw-panel" style={{ marginTop: 14 }}>
        {list.length === 0 ? (
          <Empty title="Nothing here" icon="circle-check">The twin sees no warnings of this kind right now.</Empty>
        ) : (
          <table className="tw-table">
            <thead>
              <tr>
                <th style={{ width: 110 }}>State</th>
                <th>Warning</th>
                <th style={{ width: 120 }}>Kind</th>
                <th style={{ width: 90 }}>When</th>
                <th>Customers</th>
                <th className="num" style={{ width: 70 }}>GPUs</th>
                <th style={{ width: 120 }}></th>
              </tr>
            </thead>
            <tbody>
              {list.map((w) => {
                const p = predFor(w);
                const isOpen = open === w.id;
                return (
                  <React.Fragment key={w.id}>
                    <tr className="click" onClick={() => (p ? setOpen(isOpen ? null : w.id) : go(w.view, { focus: w.asset }))}>
                      <td><TlpChip tlp={w.tlp} size="sm" /></td>
                      <td>
                        <div style={{ fontWeight: 600 }}>{w.title}</div>
                        <div className="why" title={w.detail}>{w.detail}</div>
                      </td>
                      <td>
                        <span className="tw-kind"><Icon name={KIND[w.kind].icon} size={13} /> {KIND[w.kind].label}</span>
                      </td>
                      <td style={{ color: TLP[w.tlp].fg, fontWeight: 600 }}>{w.eta_min !== undefined && w.eta_min > 0 ? fmtEta(w.eta_min) : "now"}</td>
                      <td>
                        {w.customers.map((id) => {
                          const cu = c.ix.customers.get(id);
                          return (
                            <button key={id} className="tw-cust" style={{ borderColor: cu?.color }} onClick={(e) => (e.stopPropagation(), go("customers", { id }))}>
                              <i style={{ background: cu?.color }} />
                              {cu?.name ?? id}
                            </button>
                          );
                        })}
                      </td>
                      <td className="num">{w.gpus || ""}</td>
                      <td>
                        <button className="tw-link" onClick={(e) => (e.stopPropagation(), go(w.view, { focus: w.asset }))}>
                          Open in {w.view} <Icon name="arrow-right" size={12} />
                        </button>
                      </td>
                    </tr>
                    {isOpen && p && (
                      <tr className="tw-subrow">
                        <td></td>
                        <td colSpan={6}>
                          <div style={{ display: "flex", gap: 20, alignItems: "center", flexWrap: "wrap" }}>
                            {p.history.length > 0 && <ForecastChart history={p.history} forecast={p.forecast} upper={p.upper} lower={p.lower} threshold={p.kind === "forecast" ? p.threshold : undefined} width={460} height={110} />}
                            <div style={{ maxWidth: 420, fontSize: 13, color: "var(--text-secondary)" }}>
                              <div>{p.text}</div>
                              <div style={{ marginTop: 8, fontSize: 11.5, color: "var(--text-muted)" }}>
                                {p.kind === "forecast" && "augurs MSTL (daily seasonality) with an AutoETS trend, fitted in the twin-predict Wasm component on the last 3 simulated days."}
                                {p.kind === "peer_outlier" && "augurs MAD peer detector on the row's physics residuals (return − supply − heat/flow)."}
                                {p.kind === "runtime" && "UPS runtime at the present load, from the module's own estimate."}
                              </div>
                              <button className="tw-link" style={{ marginTop: 8 }} onClick={() => go(w.view, { focus: w.asset })}>
                                Show {w.asset_name} in {w.view} <Icon name="arrow-right" size={12} />
                              </button>
                            </div>
                          </div>
                        </td>
                      </tr>
                    )}
                  </React.Fragment>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
