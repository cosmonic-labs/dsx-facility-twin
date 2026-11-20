import React, { useMemo, useState } from "react";
import { BrandMark, Icon } from "@design";
import { post, usePoll } from "./lib/api.ts";
import { indexTopo } from "./lib/model.ts";
import { go, useRoute } from "./lib/router.ts";
import { TLP } from "./lib/tlp.ts";
import type { Clock, PredictionSet, RunView, ScenarioDef, Snapshot, Topology } from "./lib/types.ts";
import { AssetDrawer } from "./components/AssetDrawer.tsx";
import { Login } from "./views/Login.tsx";
import { Overview } from "./views/Overview.tsx";
import { Hardware } from "./views/Hardware.tsx";
import { DomainView } from "./views/DomainView.tsx";
import { Customers } from "./views/Customers.tsx";
import { Impact } from "./views/Impact.tsx";
import { Warnings } from "./views/Warnings.tsx";
import { Scenarios } from "./views/Scenarios.tsx";

export interface Ctx {
  topo: Topology;
  ix: ReturnType<typeof indexTopo>;
  snap: Snapshot | null;
  preds: PredictionSet | null;
  clock: Clock | null;
  runs: RunView[];
  scenarios: ScenarioDef[];
  refreshRuns: () => void;
  select: (id: string | null) => void;
  selected: string | null;
  params: URLSearchParams;
}

const NAV: { page: string; label: string; icon: string }[] = [
  { page: "overview", label: "Overview", icon: "layout-dashboard" },
  { page: "warnings", label: "Warnings & predictions", icon: "siren" },
  { page: "customers", label: "Customers", icon: "users" },
  { page: "impact", label: "Impact explorer", icon: "git-fork" },
  { page: "hardware", label: "Hardware", icon: "server" },
  { page: "power", label: "Power", icon: "zap" },
  { page: "cooling", label: "Cooling", icon: "droplets" },
  { page: "network", label: "Network", icon: "network" },
  { page: "scenarios", label: "Failure scenarios", icon: "flask-conical" },
];

export function App() {
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [me] = usePoll<{ user: string }>(authed === false ? null : "/api/me", 60_000, () => setAuthed(false));
  if (me && authed === null) setAuthed(true);
  if (authed === false) return <Login onDone={() => setAuthed(true)} />;
  if (!me) return <div className="tw-splash"><BrandMark size={40} color="var(--accent)" /></div>;
  return <Shell onLogout={() => post("/api/logout").finally(() => setAuthed(false))} onUnauthorized={() => setAuthed(false)} />;
}

function Shell({ onLogout, onUnauthorized }: { onLogout: () => void; onUnauthorized: () => void }) {
  const route = useRoute();
  const [topo, topoErr, refetchTopo] = usePoll<Topology>("/api/topology", 60_000, onUnauthorized);
  const [snap, snapErr] = usePoll<Snapshot>("/api/snapshot", 2_000, onUnauthorized);
  const [preds] = usePoll<PredictionSet>("/api/predictions", 5_000, onUnauthorized);
  const [clock] = usePoll<Clock>("/api/sim/clock", 3_000, onUnauthorized);
  const [runs, , refreshRuns] = usePoll<RunView[]>("/api/sim/runs", 2_500, onUnauthorized);
  const [scenarios] = usePoll<ScenarioDef[]>("/api/sim/scenarios", 120_000, onUnauthorized);
  const [selected, setSelected] = useState<string | null>(null);
  const ix = useMemo(() => (topo ? indexTopo(topo) : null), [topo]);
  if (topo && snap && snap.topo_version !== topo.version && snap.topo_version !== 0) setTimeout(refetchTopo, 0);

  // ?focus= on any page opens that asset
  const focus = route.params.get("focus");
  React.useEffect(() => {
    if (focus) setSelected(focus);
  }, [focus]);

  const active = (runs ?? []).filter((r) => !r.done);
  const warnCount = snap?.warnings.filter((w) => w.tlp !== "green").length ?? 0;
  const worstTlp = snap?.kpis.customers_affected ? "orange" : snap?.kpis.customers_at_risk ? "yellow" : "green";

  if (!topo || !ix) {
    return (
      <div className="tw-splash">
        <BrandMark size={40} color="var(--accent)" />
        <div>{topoErr ? `Waiting for the twin: ${topoErr}` : "Loading the site model…"}</div>
      </div>
    );
  }
  const ctx: Ctx = {
    topo,
    ix,
    snap,
    preds,
    clock,
    runs: runs ?? [],
    scenarios: scenarios ?? [],
    refreshRuns,
    select: setSelected,
    selected,
    params: route.params,
  };
  const page = route.page;
  const simTime = clock ? new Date(clock.sim_ms) : null;
  return (
    <div className={"tw-app" + (selected && ix.byId.has(selected) ? " drawer-open" : "")}>
      <header className="tw-header">
        <BrandMark size={22} color="#fff" />
        <div className="tw-title">
          <b>DSX Facility Twin</b>
          <span>{topo.site.name}</span>
        </div>
        <div className="tw-hstat" title="Overall state">
          <span className="tw-hdot" style={{ background: TLP[worstTlp].line }} />
          {snap ? `${snap.kpis.customers_affected} affected · ${snap.kpis.customers_at_risk} at risk` : "connecting…"}
        </div>
        {active.length > 0 && (
          <button className="tw-hchip" onClick={() => go("scenarios")}>
            <Icon name="flask-conical" size={13} /> {active.length} scenario{active.length > 1 ? "s" : ""} running
          </button>
        )}
        <div style={{ flex: 1 }} />
        {simTime && (
          <div className="tw-hclock" title="Simulated site time">
            <Icon name="clock" size={13} />
            {simTime.toLocaleDateString(undefined, { month: "short", day: "numeric", timeZone: "America/New_York" })}{" "}
            {simTime.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", timeZone: "America/New_York" })}
            <span>{clock!.paused ? "paused" : `${clock!.speed}×`}</span>
          </div>
        )}
        <div className="tw-live" title={snapErr ?? "Live"}>
          <span className={snapErr ? "" : "on"} /> {snapErr ? "reconnecting" : "live"}
        </div>
        <button className="tw-hbtn" onClick={onLogout} title="Sign out">
          <Icon name="log-out" size={15} /> admin
        </button>
      </header>
      <nav className="tw-side">
        {NAV.map((n) => (
          <button key={n.page} className={"tw-nav" + (page === n.page ? " on" : "")} onClick={() => go(n.page)}>
            <Icon name={n.icon} size={17} />
            <span>{n.label}</span>
            {n.page === "warnings" && warnCount > 0 && <em>{warnCount}</em>}
            {n.page === "scenarios" && active.length > 0 && <em className="acc">{active.length}</em>}
          </button>
        ))}
        <div className="tw-side-foot">
          <div>Analytics on Cosmonic Control</div>
          <div className="mono">augurs · petgraph · wasmcloud:nats</div>
          {snap && <div className="mono">impact {Math.max(1, Math.round(snap.took_us / 1000))} ms · fit {preds?.fitted ?? 0} series</div>}
        </div>
      </nav>
      <main className="tw-main">
        {snapErr && (
          <div className="tw-banner">
            <Icon name="loader-circle" size={14} className="tw-spin" /> {snapErr} The views keep the last good data until the twin answers again.
          </div>
        )}
        {page === "overview" && <Overview c={ctx} />}
        {page === "warnings" && <Warnings c={ctx} />}
        {page === "customers" && <Customers c={ctx} />}
        {page === "impact" && <Impact c={ctx} />}
        {page === "hardware" && <Hardware c={ctx} />}
        {(page === "power" || page === "cooling" || page === "network") && <DomainView key={page} c={ctx} domain={page} />}
        {page === "scenarios" && <Scenarios c={ctx} />}
      </main>
      {selected && ix.byId.has(selected) && (
        <AssetDrawer id={selected} topo={topo} customers={topo.customers} preds={preds?.items ?? []} onClose={() => setSelected(null)} onSelect={setSelected} />
      )}
    </div>
  );
}
