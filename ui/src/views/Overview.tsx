import React from "react";
import { Icon } from "@design";
import type { Ctx } from "../App.tsx";
import { go } from "../lib/router.ts";
import { TLP, fmtEta, money, rank } from "../lib/tlp.ts";
import { EtaBadge, GpuBar, Kpi, Money, Panel, TlpChip, TlpDot, Empty } from "../components/ui.tsx";
import { HallMap } from "./Hardware.tsx";
import { usePoll } from "../lib/api.ts";
import type { Warning } from "../lib/types.ts";

export function Overview({ c }: { c: Ctx }) {
  const s = c.snap;
  const k = s?.kpis;
  const [site] = usePoll<Record<string, { v: number }>>("/api/lkv?points=dsx1.site_it_kw,dsx1.site_facility_kw,dsx1.site_pue,GRID.grid_import_kw,BESS-1.bess_soc", 5000);
  const customers = (s?.customers ?? []).map((cs) => ({ cs, cu: c.ix.customers.get(cs.id)! })).filter((x) => x.cu);
  const atRisk = customers.filter((x) => x.cs.tlp !== "green").sort((a, b) => rank(b.cs.tlp) - rank(a.cs.tlp) || (a.cs.eta_min ?? 1e9) - (b.cs.eta_min ?? 1e9));
  const incidents = (s?.warnings ?? []).filter((w) => w.kind === "out" || w.kind === "degraded").slice(0, 6);
  const predictions = (s?.warnings ?? []).filter((w) => w.kind === "prediction" || w.kind === "watch").slice(0, 6);
  const running = c.runs.filter((r) => !r.done);
  const g = k?.gpus ?? { green: 0, yellow: 0, orange: 0, red: 0, unknown: 0 };
  const it = site?.["dsx1.site_it_kw"]?.v;
  const pue = site?.["dsx1.site_pue"]?.v;
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Overview</h1>
          <p>What the hall means to the business right now, and what is about to change.</p>
        </div>
      </div>
      <div className="tw-kpis">
        <Kpi icon="users" label="Customers affected" value={k?.customers_affected ?? "–"} sub={`of ${k?.customers_total ?? 0} · degraded or out`} tlp={k?.customers_affected ? "orange" : undefined} onClick={() => go("customers")} />
        <Kpi icon="user-search" label="Customers at risk" value={k?.customers_at_risk ?? "–"} sub="on trend to be impacted" tlp={k?.customers_at_risk ? "yellow" : undefined} onClick={() => go("warnings")} />
        <Kpi icon="cpu" label="GPUs impacted" value={(g.orange + g.red).toLocaleString()} sub={`${g.red} out · ${g.orange} degraded · ${g.yellow} at risk`} tlp={g.red ? "red" : g.orange ? "orange" : undefined} />
        <Kpi icon="badge-dollar-sign" label="Revenue impacted" value={<Money v={k?.revenue_impacted_hr ?? 0} tlp="orange" />} sub={<span>at risk <Money v={k?.revenue_at_risk_hr ?? 0} tlp="yellow" /></span>} tlp={(k?.revenue_impacted_hr ?? 0) > 0 ? "orange" : undefined} />
        <Kpi icon="gauge" label="IT load" value={it ? `${(it / 1000).toFixed(2)} MW` : "–"} sub={pue ? `PUE ${pue.toFixed(2)} · grid ${((site?.["GRID.grid_import_kw"]?.v ?? 0) / 1000).toFixed(2)} MW` : ""} />
        <Kpi icon="shield-alert" label="Redundancy lost" value={k?.redundancy_lost ?? 0} sub="assets on a single path" tlp={k?.redundancy_lost ? "yellow" : undefined} onClick={() => go("warnings", { kind: "redundancy_lost" })} />
      </div>

      <div className="tw-grid2">
        <Panel title="Customers at risk" icon="user-round-x" actions={<button className="tw-link" onClick={() => go("customers")}>All customers</button>} pad={false}>
          {atRisk.length === 0 ? (
            <Empty title="Every customer is green">No asset any customer depends on is degraded, out, or forecast to be.</Empty>
          ) : (
            <table className="tw-table">
              <thead>
                <tr>
                  <th>Customer</th>
                  <th>State</th>
                  <th>GPUs</th>
                  <th className="num">Impacted</th>
                  <th className="num">At risk</th>
                  <th>When</th>
                  <th>Why</th>
                </tr>
              </thead>
              <tbody>
                {atRisk.map(({ cs, cu }) => (
                  <tr key={cs.id} className="click" onClick={() => go("customers", { id: cs.id })}>
                    <td>
                      <span className="tw-cust" style={{ borderColor: cu.color }}>
                        <i style={{ background: cu.color }} />
                        {cu.name}
                      </span>{" "}
                      <small className="tw-tier">{cu.tier}</small>
                    </td>
                    <td><TlpChip tlp={cs.tlp} size="sm" /></td>
                    <td style={{ minWidth: 120 }}>
                      <GpuBar g={cs.gpus} />
                      <small>{cs.gpus.red + cs.gpus.orange + cs.gpus.yellow} of {cu.allocations.reduce((a, b) => a + b.gpus, 0)}</small>
                    </td>
                    <td className="num"><Money v={cs.revenue_impacted_hr} tlp="orange" /></td>
                    <td className="num"><Money v={cs.revenue_at_risk_hr} tlp="yellow" /></td>
                    <td>{cs.tlp === "yellow" ? <EtaBadge eta={cs.eta_min} /> : <span style={{ color: TLP[cs.tlp].fg, fontWeight: 600 }}>now</span>}</td>
                    <td className="why" title={cs.reason}>{cs.reason}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Panel>
        <Panel title="GPUs by state" icon="cpu">
          <GpuBar g={g} height={14} />
          <div className="tw-legend">
            {(["red", "orange", "yellow", "unknown", "green"] as const).map((t) => (
              <span key={t}>
                <TlpDot tlp={t} /> {TLP[t].label} <b>{g[t].toLocaleString()}</b>
              </span>
            ))}
          </div>
          <div style={{ height: 14 }} />
          <HallMap c={c} compact />
        </Panel>
      </div>

      <div className="tw-grid3">
        <Panel title="Active incidents" icon="siren" actions={<button className="tw-link" onClick={() => go("warnings")}>All warnings</button>} pad={false}>
          {incidents.length === 0 ? <Empty title="No equipment degraded or out" /> : <WarnList list={incidents} c={c} />}
        </Panel>
        <Panel title="Predicted and early warnings" icon="trending-up" actions={<button className="tw-link" onClick={() => go("warnings", { kind: "prediction" })}>All predictions</button>} pad={false}>
          {predictions.length === 0 ? <Empty icon="sparkles" title="Nothing on trend">augurs re-fits every few simulated minutes; forecasts that cross a limit within two hours show up here.</Empty> : <WarnList list={predictions} c={c} />}
        </Panel>
        <Panel title="Failure scenarios" icon="flask-conical" actions={<button className="tw-link" onClick={() => go("scenarios")}>Open control panel</button>}>
          {running.length === 0 ? (
            <Empty icon="flask-conical" title="No scenario running">Start a leak, a grid event or an equipment failure from the control panel and watch it propagate.</Empty>
          ) : (
            running.map((r) => (
              <div key={r.id} className="tw-run-mini" onClick={() => go("scenarios", { run: String(r.id) })}>
                <b>{r.name}</b> <span>on {r.target_name}</span>
                <div className="tw-steps mini">
                  {r.stages.map((st, i) => (
                    <span key={st.key} className={(i < r.stage ? "done " : i === r.stage ? "cur " : "") + st.kind}>{st.name}</span>
                  ))}
                </div>
              </div>
            ))
          )}
        </Panel>
      </div>
    </div>
  );
}

export function WarnList({ list, c }: { list: Warning[]; c: Ctx }) {
  return (
    <div className="tw-wlist">
      {list.map((w) => (
        <button key={w.id} className="tw-w" onClick={() => go(w.view, { focus: w.asset })} style={{ borderLeftColor: TLP[w.tlp].line }}>
          <div className="tw-w-t">
            <TlpDot tlp={w.tlp} />
            <b>{w.title}</b>
            {w.eta_min !== undefined && w.eta_min > 0 && <EtaBadge eta={w.eta_min} />}
          </div>
          <div className="tw-w-d">{w.detail}</div>
          {w.customers.length > 0 && (
            <div className="tw-w-c">
              <Icon name="users" size={12} /> {w.customers.map((id) => c.ix.customers.get(id)?.name ?? id).join(", ")} · {w.gpus} GPUs
            </div>
          )}
        </button>
      ))}
    </div>
  );
}
export { fmtEta, money };
