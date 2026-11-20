# Architecture

## Pieces

| Piece | Runs as | Reads | Writes |
| --- | --- | --- | --- |
| `twin-sim` | native container (demo harness) | its profile, scenario API calls | DSX BMS values and metadata, health feeds, history backfill, profile, predict ticks |
| `twin-ingest` | Wasm component, `wasmcloud:nats` core-handler | `BMS.v1.PUB.>`, `twin.v1.health.>`, `twin.v1.backfill`, `twin.v1.profile` | `lkv.*`, `ts.*`, `pmap.*`, `unmapped.*`, `topology`, `config`; `twin.v1.sev.<asset>` |
| `twin-predict` | Wasm component | `twin.v1.tick.predict`; `ts.*`, `lkv.*`, `conditions` | `predictions`; `twin.v1.pred` |
| `twin-impact` | Wasm component | `twin.v1.sev.>`, `twin.v1.pred`, `twin.v1.profile.applied`; `topology`, `config`, `conditions`, `predictions` | `conditions`, `snapshot`; `twin.v1.snapshot.updated` |
| `twin-api` | Wasm component, `wasi:http@0.3.0` | every KV key above; the simulator API | — |

Each Wasm workload gets only the subjects and the bucket it needs (`subject-allow`, `bucket-allow`
on the `wasmcloud:nats` binding). `twin-api` may open one outbound connection: the simulator.

## Flow of one reading

1. `twin-sim` publishes `{"value":47.2,"timestamp":…,"quality":1}` on
   `BMS.v1.PUB.Value.Rack.RackLiquidReturnTemperature.cosmonic.dsx1.P1.P1-R2-03.liquid_return_temp.value`.
2. `twin-ingest` resolves the subject to point `P1-R2-03.liquid_return_temp` (topology index, else
   DSX metadata learned at runtime, else subject rules), evaluates it against the class limits
   (`watch 43 / degraded 47 / out 56`), stores `lkv.<point>`, folds it into `ts.<point>.<day>` and,
   because the condition changed from green to orange, publishes `twin.v1.sev.P1-R2-03`.
3. `twin-impact` folds the event into `conditions` and recomputes the snapshot: two passes over the
   site graph in dependency order (now, and future with forecasts applied), customer rollups,
   warnings.
4. `twin-api` serves the snapshot; the UI polls it every two seconds.

## Impact semantics

Every asset has requirement groups: *needs `need` of these providers*. Per group, in dependency
order:

- fewer than `need` available → **red** (or **yellow** if the group is *bridged* by stored energy or
  thermal mass, or **orange** if it is a *partial* capacity group with something left);
- fewer than `need` healthy → **orange**;
- exactly `need` healthy of a larger set → *redundancy lost* (flag, not a color).

The future pass assumes every forecast comes true and every yellow leading indicator degrades.
Anything fine now but not in the future is **yellow: on trend to be impacted**, with the ETA at
which its group would fail. Peer-outlier detections warn but never color the graph: they usually
mean a sensor, not the equipment.

## Forecast hygiene

`twin-predict` fits augurs MSTL (daily period) with an AutoETS trend to up to three days of
5-minute history, and takes the worse of that and the last hour's least-squares trend. Five
guards keep it from crying wolf. All came out of scenario QA; the statistical ones have regression tests in
`crates/twin-analytics`:

- **Past incidents are clipped out of the history** before fitting: samples older than two hours
  are held within 5·MAD of the typical value for their time of day (the median across days of
  the samples within an hour either side). Otherwise the seasonal component learns yesterday's
  pump trip and forecasts it again at the same time today. `tests/replay.rs` replays the real
  facility-water history from the lab that exposed this.
- **A step is not a trend.** When one jump explains most of the last hour (a breaker trips and the
  other cord picks up the rack), only the samples after the jump are fitted.
- **A recovery ramp is not a trend.** A series climbing back from far below its daily level is
  left to augurs alone.
- **Assets that are already out are not forecast**: there is nothing left to predict.
- **Triage first.** Only series within one daily swing of their limit, or heading there inside the
  horizon, are fitted at all (`worth_fitting`).

UPS runtime ETAs come from the `OnBattery` state alone; an alarm is yellow but is not a
countdown.

## Why Wasm components here

- The analytics are small, sandboxed and swappable: a new forecaster is a new component, not a
  new service with its own image, network policy and patch cycle.
- Grants are explicit and narrow per workload; `twin-predict` cannot reach the network at all.
- The same binaries run at an edge site, the hub, or an air-gapped cloud.
