# DSX facility twin

A digital twin of an NVIDIA DSX-style AI factory hall: power, cooling, network, compute and the
customers on top. A native simulator plays the plant; the analytics, the API and the control panel
run as **WebAssembly components on Cosmonic Control**.

- **augurs** (Rust, compiled to Wasm) forecasts telemetry and flags what is *on trend* to break.
- **petgraph** (Rust, compiled to Wasm) walks the site's dependency graph to color every asset and
  customer by impact, with root causes and ETAs.
- **wasmcloud:nats** gives the components NATS subscriptions and NATS KV; nothing else.

```mermaid
flowchart LR
    sim["<b>twin-sim</b><br/>native simulator, scenario API"]
    ingest["<b>twin-ingest</b> (Wasm)<br/>bind subject to point, evaluate limits"]
    predict["<b>twin-predict</b> (Wasm, augurs)<br/>series to predictions"]
    impact["<b>twin-impact</b> (Wasm, petgraph)<br/>conditions to snapshot"]
    api["<b>twin-api</b> (Wasm, wasi:http)<br/>React control panel, JSON API, login (demo)"]
    kv[("NATS KV<br/>lkv, ts, conditions,<br/>predictions, snapshot")]

    sim -- "DSX BMS telemetry, health feeds,<br/>history, profile" --> ingest
    sim -- "twin.v1.tick.predict" --> predict
    ingest -- "twin.v1.sev.&lt;asset&gt;" --> impact
    predict -- "twin.v1.pred" --> impact
    ingest --> kv
    predict --> kv
    impact --> kv
    kv --> api
    api -- "/api/sim/*" --> sim
```

| Path | What |
| --- | --- |
| `crates/twin-model` | Shared data model: topology, classes and limits, messages, the impact engine (petgraph) |
| `crates/twin-analytics` | augurs forecasting (MSTL + AutoETS) and peer-outlier detection |
| `components/` | `twin-ingest`, `twin-predict`, `twin-impact`, `twin-api` (wasm32-wasip2) |
| `sim/` | `twin-sim`: physics, DSX BMS publisher, 21 failure scenarios, scenario API |
| `ui/` | React control panel. Built on the Cosmonic Desktop design system, which is not part of this repository (see below) |
| `profiles/demo-dsx1` | The demo site: template, tenants, point mapping, classes and limits |
| `deploy/` | Hostgroup values and workload manifests for Control |
| `docs/` | Architecture, data model, customer mapping, scenarios, storage, lab |

## Watch the demo

[![Extending NVIDIA DSX with secure sandboxes: watch the demo on YouTube](docs/media/video-thumbnail.png)](https://youtu.be/fcvekk56-N8)

[Extending NVIDIA DSX with Secure Sandboxes: AI Factory Digital Twin Demo](https://youtu.be/fcvekk56-N8)
(4 minutes): a failure cascading through the hall, the twin predicting a pump failure and tracing
it to customers and revenue, then how the four sandboxed components are built, inspected and
configured.

## Walkthrough

**The hall, healthy.** One live picture: customers, GPUs, revenue, load, redundancy.

![Overview of a healthy hall](docs/media/01-overview-healthy.png)

**The floor and who is on it.** Plant, pods, rows, racks and every compute tray, colored by state or by customer.

| By state | By customer |
| --- | --- |
| ![Hardware floor plan](docs/media/02-hardware-floor-plan.png) | ![Hardware by customer](docs/media/03-hardware-by-customer.png) |

**The dependency graph.** Every asset is mapped to what it depends on.

| Power | Cooling | Network |
| --- | --- | --- |
| ![Power graph](docs/media/04-power-graph.png) | ![Cooling graph](docs/media/05-cooling-graph.png) | ![Network graph](docs/media/06-network-graph.png) |

**Trigger a failure.** Failure scenarios lists 21 scenarios; three are set up as a suggested demo.

![Failure scenarios](docs/media/07-failure-scenarios.png)

### Scenario 1: an alarm, not an outage

A rack outgrows its breaker. The twin forecasts the trip; when the breaker opens, the tap is out but
the rack stays green on its B feed. Redundancy lost, nobody affected.

| Forecast | Trip | Overview |
| --- | --- | --- |
| ![Breaker forecast](docs/media/08-breaker-forecast.png) | ![Breaker trip, rack still green](docs/media/09-breaker-trip-redundancy.png) | ![Redundancy lost, no customers affected](docs/media/10-overview-redundancy-lost.png) |

### Scenario 2: a CDU pump wears out

**Prediction.** Vibration climbs and the twin forecasts the failure well before it happens. The pump
turns yellow in the graph; no customer is affected yet.

| Scenario running | Forecast | Early warning in the graph |
| --- | --- | --- |
| ![Scenario running](docs/media/11-scenario-running.png) | ![Prediction and forecast](docs/media/12-prediction-forecast.png) | ![Cooling graph, early warning](docs/media/13-cooling-early-warning.png) |

**Escalation.** Flow falls; the row's racks and two customers are on trend to be impacted.

![Flow loss forecast](docs/media/14-flow-loss-forecast.png)

**Incident.** The pump trips. The CDU is out, its partner carries the row alone, and the racks degrade.

| Scenario | Cooling graph | On the floor |
| --- | --- | --- |
| ![Pump trip](docs/media/15-scenario-pump-trip.png) | ![Cooling incident](docs/media/16-cooling-incident.png) | ![Hardware during the incident](docs/media/17-hardware-incident.png) |

**Impact.** Walk from the cause up to customers, or from a customer back down to the root cause.

| By hardware | By customer |
| --- | --- |
| ![Impact by hardware](docs/media/18-impact-by-hardware.png) | ![Impact by customer](docs/media/19-impact-by-customer.png) |

**Customers and the business.** GPUs degraded and revenue impacted, per customer and for the hall.

| Customer | Overview |
| --- | --- |
| ![Customers](docs/media/20-customers.png) | ![Overview during the incident](docs/media/21-overview-incident.png) |

**Restore.** Repair the pump and the hall recovers.

![Overview restored](docs/media/22-overview-restored.png)

### Scenario 3: the hall loses facility water

One pump shows an early warning, the lead trips, the standby faults, and every customer goes red.

| Early warning | Cooling cascade | The floor | Overview |
| --- | --- | --- | --- |
| ![Early warning](docs/media/23-hook-warning.png) | ![Cooling cascade](docs/media/24-hook-cooling-cascade.png) | ![Floor plan, all red](docs/media/25-hook-hardware-red.png) | ![Overview, all customers out](docs/media/26-hook-overview-catastrophic.png) |

## Run it (lab VM)

```bash
make up        # builder image, UI, Wasm, simulator image, hostgroup, KV bucket, deploy, expose
make url       # → http://<vm-ip>:8088   (admin / admin)
make qa        # run every scenario end to end; writes tests/qa/report.md
make e2e       # headless screenshots of every view; tests/e2e/out/
```

Requirements: the dsx-lab kind cluster with Cosmonic Control 0.15 (see `docs/lab.md`).

## Suggested demo

Failure scenarios opens with three scenarios to play in order. Each has an early warning, and the
run card says what to point at in each stage. Play sets the clock to 30× so forecasts have
enough readings to show a trend.

1. **Small: Busway tap overload and trip.** augurs forecasts the trip; the breaker opens; 2N
   keeps the rack running. Redundancy lost, nobody affected.
2. **Medium: CDU pump bearing wear.** Vibration forecast first, then flow loss puts Lumen and
   Corvid on trend, then the pump trips and they degrade.
3. **Catastrophic: Facility water pump trip.** One yellow pump; the lead trips, the standby
   takes over, then faults. Every customer goes red.

The stories live in `ui/src/lib/stories.ts`. Scenario QA (`make qa`) drives all 21 scenarios
stage by stage and writes `tests/qa/report.md`.

## Building the UI

`ui/` imports its components and tokens from `ui/src/design-system` (the `@design` alias in
`ui/vite.config.ts`). That directory is the Cosmonic Desktop design system and is not published
here, so `make ui` needs a copy of it in place. The Rust crates, the components and the simulator
build without it; `twin-api` embeds whatever is in `ui/dist`.

## Demo-only choices

- The twin's KV and subjects live on **Control's own NATS** (Nexus) for convenience. In a DSX site
  the twin reads DSX Exchange and keeps its KV beside it: see `docs/storage.md`.
- The login is a static `admin/admin` that sets a cookie. It is not authentication: see
  `docs/security.md`.
- The hostgroup pulls the components from the kind registry over plain HTTP.
