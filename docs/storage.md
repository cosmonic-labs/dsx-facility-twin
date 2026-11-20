# Storage and messaging

## Demo (this repo)

Everything lives on **Control's own data-plane NATS** (Nexus, `nexus.cosmonic-system:4222`):

- subjects `BMS.v1.PUB.>` and `twin.v1.>`;
- one JetStream KV bucket, `twin` (history 1, max value 1 MiB).

This is a lab convenience only. It puts facility telemetry on the same NATS that runs the
Control plane, and the bucket has no replication.

## Production layout

- **Telemetry** stays on DSX Exchange (the Event Bus). The `twin` hostgroup's `wasmcloud-nats`
  plugin gets `servers:` pointing at Exchange and an NKey/JWT credential with read on
  `BMS.v1.PUB.>` and write on the twin's own subjects only.
- **State** (KV) moves to a JetStream domain beside Exchange with replication 3, or a separate NATS
  cluster for the twin. Rollups (`ts.*`) go to a long-retention bucket or a Parquet historian.
- **Ingest scales out** with a queue group on the core subscription (`subject:queue`) and several
  replicas; impact stays single-writer.
- The UI and API sit behind the site's identity provider; see `docs/security.md`.
