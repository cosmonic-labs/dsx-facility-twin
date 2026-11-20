import React, { useEffect, useState } from "react";
import { Button, Icon, Select, Switch } from "@design";
import type { Ctx } from "../App.tsx";
import { post } from "../lib/api.ts";
import { stateOf } from "../lib/model.ts";
import { go } from "../lib/router.ts";
import { TLP, rank } from "../lib/tlp.ts";
import type { AssetState, RunView, ScenarioDef } from "../lib/types.ts";
import { EtaBadge, Panel, Seg, TlpChip } from "../components/ui.tsx";
import { STORIES, storyFor } from "../lib/stories.ts";

/** A rack or row target is a container: its state is the worst of itself and what it holds
 *  (a rack's NVLink domain and trays, a row's racks and CDUs). Returns that state and the ids. */
export function targetState(c: Ctx, id: string): { st: AssetState; ids: string[] } {
  const a = c.ix.byId.get(id);
  const ids = [id];
  if (a && (a.kind === "rack" || a.kind === "row")) for (const k of c.ix.children.get(id) ?? []) ids.push(k.id);
  let st = stateOf(c.snap, id);
  for (const k of ids.slice(1)) {
    const ks = stateOf(c.snap, k);
    if (rank(ks.tlp) > rank(st.tlp)) st = { ...ks, reason: `${c.ix.byId.get(k)?.name ?? k}: ${ks.reason}` };
  }
  return { st, ids };
}

const DOMAIN: Record<string, { label: string; icon: string }> = {
  cooling: { label: "Cooling", icon: "droplets" },
  power: { label: "Power", icon: "zap" },
  grid: { label: "Grid", icon: "utility-pole" },
  network: { label: "Network", icon: "network" },
  compute: { label: "Compute", icon: "cpu" },
  data: { label: "Data", icon: "database" },
};
const VIEW_FOR: Record<string, string> = { cooling: "cooling", power: "power", grid: "power", network: "network", compute: "hardware", data: "hardware" };

export function Scenarios({ c }: { c: Ctx }) {
  const [sel, setSel] = useState<string>(c.params.get("s") ?? "cdu-pump-wear");
  const def = c.scenarios.find((s) => s.id === sel);
  const active = c.runs.filter((r) => !r.done);
  const recent = c.runs.filter((r) => r.done).slice(0, 6);
  const domains = [...new Set(c.scenarios.map((s) => s.domain))];
  return (
    <div className="tw-page">
      <div className="tw-page-h">
        <div>
          <h1>Failure scenarios</h1>
          <p>Each scenario plays out over simulated time: a warning period, the failure, sometimes an escalation, then the restore. Step through it, or let it auto-play.</p>
        </div>
        <ClockControl c={c} />
      </div>
      <SuggestedDemo c={c} onDetails={setSel} />
      {active.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 16 }}>
          {active.map((r) => (
            <RunCard key={r.id} r={r} c={c} />
          ))}
        </div>
      )}
      <h2 className="tw-h2">All scenarios</h2>
      <div className="tw-scen">
        <div className="tw-scen-list">
          {domains.map((d) => (
            <div key={d}>
              <div className="tw-scen-d">
                <Icon name={DOMAIN[d]?.icon ?? "box"} size={14} /> {DOMAIN[d]?.label ?? d}
              </div>
              {c.scenarios
                .filter((s) => s.domain === d)
                .map((s) => (
                  <button key={s.id} className={"tw-scen-i" + (s.id === sel ? " on" : "")} onClick={() => setSel(s.id)}>
                    {s.name}
                    {active.some((r) => r.scenario === s.id) && <span className="tw-dot-live" />}
                  </button>
                ))}
            </div>
          ))}
        </div>
        {def ? <ScenarioDetail key={def.id} def={def} c={c} /> : <Panel title="Loading scenarios…">The simulator's scenario API is not answering yet.</Panel>}
      </div>
      {recent.length > 0 && (
        <Panel title="Recent runs" icon="history" style={{ marginTop: 16 }} pad={false}>
          <table className="tw-table">
            <tbody>
              {recent.map((r) => (
                <tr key={r.id}>
                  <td>#{r.id}</td>
                  <td><b>{r.name}</b> on {r.target_name}</td>
                  <td>ended at stage “{r.stages[r.stage]?.name}”</td>
                  <td className="num">{new Date(r.started_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", timeZone: "America/New_York" })}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Panel>
      )}
    </div>
  );
}

const TIER_TLP = { small: "yellow", medium: "orange", catastrophic: "red" } as const;

/** The three-act demo: small, medium, catastrophic. Play starts the run for manual stepping at 30x. */
function SuggestedDemo({ c, onDetails }: { c: Ctx; onDetails: (id: string) => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const play = async (id: string) => {
    const def = c.scenarios.find((s) => s.id === id);
    if (!def) return;
    setBusy(id);
    setErr(null);
    try {
      // 30x leaves each warning stage several readings long, so the forecasts have a trend to see.
      if (!c.clock || c.clock.paused || c.clock.speed > 30) await post("/api/sim/clock", { speed: 30, paused: false });
      await post("/api/sim/runs", { scenario: id, target: def.default_target, auto: false });
      c.refreshRuns();
      window.scrollTo({ top: 0, behavior: "smooth" });
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  return (
    <div className="tw-demo">
      <div className="tw-demo-h">
        <Icon name="presentation" size={16} color="var(--accent)" />
        <b>Suggested demo</b>
        <span>Play these in order. Each one gives an early warning before anything breaks; click Next stage to move the story along.</span>
      </div>
      <div className="tw-demo-grid">
        {STORIES.map((st, i) => {
          const def = c.scenarios.find((s) => s.id === st.id);
          const running = c.runs.some((r) => !r.done && r.scenario === st.id);
          const t = TIER_TLP[st.tier];
          return (
            <div key={st.id} className="tw-demo-card" style={{ borderTopColor: TLP[t].line }}>
              <div className="tw-demo-top">
                <span className="tw-demo-n">{i + 1}</span>
                <span className="tw-demo-tier" style={{ background: TLP[t].bg, color: TLP[t].fg }}>{st.tierLabel}</span>
                <span className="tw-demo-name">{def?.name ?? st.id}</span>
              </div>
              <h3>{st.headline}</h3>
              <p>{st.pitch}</p>
              {def && (
                <ol className="tw-demo-stages">
                  {def.stages.map((s) => (
                    <li key={s.key}>
                      <b>{s.name}</b> <span className={"tw-kind " + s.kind}>{s.kind}</span>
                    </li>
                  ))}
                </ol>
              )}
              <div className="tw-demo-act">
                <Button variant="primary" size="sm" icon="play" onClick={() => play(st.id)} loading={busy === st.id} disabled={!def || running}>
                  {running ? "Running" : "Play"}
                </Button>
                <Button variant="ghost" size="sm" icon="list" onClick={() => onDetails(st.id)}>
                  Details
                </Button>
                <Button variant="ghost" size="sm" icon="crosshair" onClick={() => def && go(st.view, { focus: def.default_target })}>
                  Watch in {st.view}
                </Button>
              </div>
            </div>
          );
        })}
      </div>
      {err && <div className="tw-err">{err}</div>}
    </div>
  );
}

function ClockControl({ c }: { c: Ctx }) {
  const clock = c.clock;
  const set = (b: object) => post("/api/sim/clock", b).then(c.refreshRuns);
  return (
    <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
      <span style={{ fontSize: 12, color: "var(--text-muted)" }}>Simulation speed</span>
      <Seg
        value={String(clock?.speed ?? 30)}
        onChange={(v) => set({ speed: Number(v) })}
        options={["1", "10", "30", "60", "120"].map((v) => ({ value: v, label: `${v}×` }))}
      />
      <Button size="sm" variant="secondary" icon={clock?.paused ? "play" : "pause"} onClick={() => set({ paused: !clock?.paused })}>
        {clock?.paused ? "Resume" : "Pause"}
      </Button>
    </div>
  );
}

function ScenarioDetail({ def, c }: { def: ScenarioDef; c: Ctx }) {
  const [target, setTarget] = useState(def.default_target);
  const [auto, setAuto] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => setTarget(def.default_target), [def.id]);
  const start = async () => {
    setBusy(true);
    setErr(null);
    try {
      await post("/api/sim/runs", { scenario: def.id, target, auto });
      c.refreshRuns();
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const ts = targetState(c, target).st;
  return (
    <Panel title={def.name} icon={DOMAIN[def.domain]?.icon}>
      <p style={{ marginTop: 0, color: "var(--text-secondary)" }}>{def.summary}</p>
      <div className="tw-scen-form">
        <Select label="Target" value={target} onChange={(e) => setTarget(e.target.value)} options={def.candidates.map((x) => ({ value: x.id, label: `${x.name} (${x.id})` }))} />
        <div style={{ display: "flex", alignItems: "center", gap: 8, paddingTop: 18 }}>
          <Switch checked={auto} onChange={(e) => setAuto(e.target.checked)} label="Auto-play stages" />
        </div>
        <div style={{ paddingTop: 18, display: "flex", gap: 8 }}>
          <Button variant="primary" icon="play" onClick={start} loading={busy}>
            Start
          </Button>
          <Button variant="ghost" icon="crosshair" onClick={() => go(VIEW_FOR[def.domain] ?? "hardware", { focus: target })}>
            Show target
          </Button>
        </div>
      </div>
      <div style={{ fontSize: 12, color: "var(--text-muted)", margin: "6px 0 14px", display: "flex", gap: 8, alignItems: "center" }}>
        Target now <TlpChip tlp={ts.tlp} size="sm" /> {ts.reason}
      </div>
      {err && <div className="tw-err">{err}</div>}
      <div className="tw-timeline">
        {def.stages.map((s, i) => (
          <div key={s.key} className={"tw-stage " + s.kind}>
            <div className="tw-stage-n">{i + 1}</div>
            <div>
              <div className="tw-stage-h">
                <b>{s.name}</b>
                <span className="tw-kind">{s.kind}</span>
                <small>{s.minutes} sim min</small>
              </div>
              <div className="tw-stage-d">{s.detail}</div>
              <div className="tw-stage-e">
                <Icon name="eye" size={12} /> Twin should show: {s.expect}
              </div>
            </div>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function RunCard({ r, c }: { r: RunView; c: Ctx }) {
  const cur = r.stages[r.stage];
  const next = r.stages[r.stage + 1];
  const act = (path: string, body?: object) => post(`/api/sim/runs/${r.id}/${path}`, body).then(c.refreshRuns);
  const { st: ts, ids } = targetState(c, r.target);
  const mine = (x: string[] | undefined) => !!x?.some((y) => ids.includes(y));
  const warnings = (c.snap?.warnings ?? []).filter((w) => ids.includes(w.asset) || mine(c.snap?.assets[w.asset]?.causes));
  const affected = Object.values(c.snap?.assets ?? {}).filter((s) => s.tlp !== "green" && mine(s.causes)).length;
  const custs = (c.snap?.customers ?? []).filter((cs) => cs.tlp !== "green");
  return (
    <div className="tw-run" style={{ borderLeftColor: TLP[ts.tlp].line }}>
      <div className="tw-run-h">
        <Icon name="flask-conical" size={16} color="var(--accent)" />
        <b>{r.name}</b>
        <span>on {r.target_name}</span>
        <TlpChip tlp={ts.tlp} size="sm" />
        <div style={{ flex: 1 }} />
        <Switch checked={r.auto} onChange={(e) => act("auto", { auto: e.target.checked })} label="Auto-play" size="sm" />
      </div>
      <div className="tw-steps">
        {r.stages.map((s, i) => (
          <div key={s.key} className={"tw-step " + s.kind + (i < r.stage ? " done" : i === r.stage ? " cur" : "")}>
            <span className="tw-step-n">{i < r.stage ? <Icon name="check" size={12} /> : i + 1}</span>
            <span>{s.name}</span>
            {i === r.stage && (
              <div className="tw-step-bar">
                <i style={{ width: `${Math.round(r.progress * 100)}%` }} />
              </div>
            )}
          </div>
        ))}
      </div>
      <div className="tw-run-b">
        <div style={{ flex: 1, minWidth: 260 }}>
          <div style={{ fontSize: 13 }}>
            <b>{cur?.name}</b> · {Math.min(r.elapsed_min, 999).toFixed(0)} of {cur?.minutes} sim min. {cur?.detail}
          </div>
          <div className="tw-stage-e">
            <Icon name="eye" size={12} /> Twin should show: {cur?.expect}
          </div>
          {cur && storyFor(r.scenario)?.stages[cur.key] && (
            <div className="tw-say">
              <Icon name="message-square-quote" size={13} /> {storyFor(r.scenario)!.stages[cur.key]}
            </div>
          )}
          <div style={{ fontSize: 12, marginTop: 6, color: "var(--text-secondary)", display: "flex", gap: 12, flexWrap: "wrap" }}>
            <span>Twin now: <b style={{ color: TLP[ts.tlp].fg }}>{TLP[ts.tlp].label}</b>{ts.tlp === "yellow" && <EtaBadge eta={ts.eta_min} />}</span>
            <span>{affected} downstream assets affected</span>
            <span>{custs.length} customers not green</span>
            <span>{warnings.length} related warnings</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          {next && cur?.kind !== "restore" && (
            <Button variant="primary" icon="skip-forward" onClick={() => act("next")}>
              {next.kind === "restore" ? "Restore" : `Next: ${next.name}`}
            </Button>
          )}
          {cur?.kind !== "restore" && next?.kind !== "restore" && (
            <Button variant="secondary" icon="wrench" onClick={() => act("restore")}>
              Restore
            </Button>
          )}
          <Button variant="ghost" icon="git-fork" onClick={() => go("impact", { root: r.target })}>
            Impact
          </Button>
          <Button variant="ghost" icon="crosshair" onClick={() => go(VIEW_FOR[r.domain] ?? "hardware", { focus: r.target })}>
            Show
          </Button>
          <Button variant="danger-ghost" icon="square" onClick={() => act("cancel")}>
            Stop
          </Button>
        </div>
      </div>
    </div>
  );
}
