# Data model

Defined in `crates/twin-model` (Rust) and mirrored in `ui/src/lib/types.ts`.

## Topology

- **Asset**: `id`, `name`, `kind`, `parent` (physical containment), `requires` (dependency groups),
  `attrs` (design values: `rated_kw`, `design_flow_lpm`, `capacity_kw`, `current_limit_a`, …),
  `points`, `tags`.
- **Requirement**: `domain`, `label`, `need`, `of` (provider ids), `bridged`, `partial`.
- **Point**: `id` (`<asset>.<class>`), `class`, `unit`, `subject`, `forecast`.
- **Customer**: `id`, `name`, `tier` (premium / standard / spot), `rate_per_gpu_hr`, `sla_pct`,
  `allocations` (racks or compute trays with GPU counts).

Asset kinds: site, hall, pod, row · grid, utility feed, generator, transfer switch, transformer,
UPS module, BESS, switchboard, busway, tap-off · dry cooler, facility pump, facility water loop,
trim chiller, CRAH, CDU · spine, leaf · rack (GB200 NVL72), NVLink fabric, compute tray.

The demo hall: 2 pods × 2 rows × 8 racks = 32 racks, 576 compute trays, 2,304 GPUs; 2N power with
N+1 UPS modules per side; two in-row CDUs per row; three dry coolers (need two); 768 assets and
1,338 points.

## Classes and limits

`profiles/<site>/classes.yaml`: one entry per canonical class, with unit, the DSX
`ObjectType/PointType` it maps from, thresholds (`watch` → yellow, `degraded` → orange,
`out` → red; `high`, `low` or `band`, optionally `relative_to` an asset attribute), or a state
table for enumerated points. Types prefixed `x-` are the twin's extensions to the DSX BMS schema.

## KV layout (bucket `twin`)

| Key | Written by | Value |
| --- | --- | --- |
| `topology`, `config`, `profile.version` | twin-ingest | expanded profile |
| `lkv.<point>` | twin-ingest | `{v, t, q, tlp}` latest reading |
| `ts.<point>.<day>` | twin-ingest | 288 five-minute buckets (running mean) |
| `pmap.<subject>` | twin-ingest | binding learned from DSX metadata |
| `unmapped.<subject>` | twin-ingest | metadata the mapping could not place |
| `conditions` | twin-impact | non-green point conditions by asset |
| `predictions` | twin-predict | forecasts, peer outliers, runtime ETAs |
| `snapshot` | twin-impact | asset states, customers, warnings, KPIs |

Every value stays under the server's 1 MB payload (topology ≈ 450 KB, worst-case snapshot ≈ 360 KB).
