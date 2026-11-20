# Bringing a customer site: mapping

The twin only reasons about canonical classes and twin asset ids. A real site brings its own
names. Four files in `profiles/<site>/` carry everything site-specific:

| File | Holds | Typical source |
| --- | --- | --- |
| `site.yaml` | assets, their dependencies and redundancy | DCIM export, single-line diagrams, DSX reference design |
| `tenants.yaml` | customers, tiers, prices, allocations | billing, Run:ai / KAI queues, NICo racks |
| `mapping.yaml` | incoming point types and object ids → classes and assets | BMS integration, DSX metadata |
| `classes.yaml` | units, thresholds, state tables | operations runbooks, OEM limits |

`twin-sim` loads the directory and publishes it on `twin.v1.profile`; a real deployment would run
a small `twinctl publish profiles/<site>` instead (same message, no simulator).

## Topology: template or explicit

`site.yaml` can describe a hall parametrically (`template:` — pods, rows, racks per row, power and
cooling sizing) or list assets explicitly. Both can be combined; explicit assets with the same id
replace template ones. An explicit asset:

```yaml
assets:
  - id: CDU-C-F00-05
    name: CDU 5 (Pod 1C)
    kind: cdu
    parent: P1-R1
    attrs: {capacity_kw: 650, design_flow_lpm: 900}
    requires:
      - {domain: power, label: power A/B, need: 1, of: [SWB-A, SWB-B]}
      - {domain: cooling, label: facility water, need: 1, of: [FWS]}
    points:
      - {id: CDU-C-F00-05.cdu_supply_temp, class: cdu_supply_temp, unit: "°C", forecast: true,
         subject: BMS.v1.PUB.Value.CDU.LiquidTemperature.nvidia.titan.pod1C.C-F00-CDU-05.phaidra.temperature.finalSetpoint.rValue}
```

DSX metadata already carries much of this: `servesId` on power meters gives feeds edges (a busway
serves its taps), `rackLocationId` places points on racks, `objectId` names the asset. An importer
that turns a DSX metadata capture into `site.yaml` is the natural next tool.

## Points: three ways a subject is bound

`twin-ingest` resolves every incoming subject in this order:

1. **Topology index**: the subject is listed on a point in `site.yaml`.
2. **DSX metadata, learned at runtime**: a `BMS.v1.PUB.Metadata.…` message names `objectType`,
   `pointType` and `objectId`. `mapping.yaml` turns them into a class (`point_types`) and an asset
   (`asset_aliases`); the binding is cached in KV as `pmap.<subject>`. Metadata the mapping cannot
   place is kept in `unmapped.*` and listed by `GET /api/unmapped`.
3. **Subject rules** for feeds without metadata (GPU health, network): `twin.v1.health.{class}.{asset}`.

```yaml
point_types:
  Rack/RackLiquidReturnTemperature: liquid_return_temp   # ObjectType/PointType
  LiquidTemperature: cdu_supply_temp                       # bare PointType
asset_aliases:
  C-F00-CDU-05: CDU-P1-R1-A                                # BMS objectId -> twin asset
subject_rules:
  - pattern: "twin.v1.health.{class}.{asset}"
```

## Customers

`tenants.yaml` allocates whole racks or individual trays (`P1-R1-01..04` ranges). In production
the allocations change hourly; feed them from the scheduler (Run:ai, KAI) or NICo instead of the
file. The impact engine joins allocations at query time, so an allocation change never rebuilds the
graph.

## Limits

Thresholds are per class and can be relative to an asset attribute, so one rule covers racks with
different design flows. Adjust `classes.yaml` to the site's OEM limits; the twin picks up a new
profile without a restart.
