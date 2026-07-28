# Social Graph Index poison receipt observer

## Purpose

The Social Graph Index worker can persist neutral connector receipts when broker bytes cannot be decoded into a trusted event. `social_graph_index_poison_observer` provides count-only operational visibility over those receipts without entering the delivery recovery path.

The observer is read-only. It does not claim or release publication leases, publish a DLQ entry, acknowledge a source cursor, repair rows, choose retention, or delete receipts. Profiles privacy and Social Graph authorization remain independent of these metrics.

## Startup

The observer starts only when all of the following are true:

- the server host runs background workers;
- the Social Graph Index durable consumer is explicitly enabled;
- an observer handle is not already registered in `ServerRuntimeContext`.

The poll interval is controlled by `RUSTOK_SOCIAL_GRAPH_INDEX_POISON_POLL_MS`.

- default: `5000` milliseconds;
- minimum: `1` millisecond;
- maximum: `300000` milliseconds.

An invalid value records the snapshot as unavailable and leaves projection active.

## Metrics

All metrics use the fixed consumer label `social_graph_index`.

### `rustok_runtime_consumer_poison_receipts`

Gauge labels: `consumer`, `state`.

The `state` label is restricted to:

- `total`;
- `reserved`;
- `publishing`;
- `expired_publishing`;
- `published`;
- `acknowledged`.

`expired_publishing` is a subset of `publishing`. The connector inspector fails closed if known states do not sum to `total` or if expired claims exceed publishing receipts.

### `rustok_runtime_consumer_poison_snapshot_available`

`1` means the latest aggregate query succeeded. `0` means inspection is unavailable. On failure, all receipt count gauges are reset to zero so stale counts cannot appear current.

### `rustok_runtime_consumer_poison_snapshot_timestamp_seconds`

Unix timestamp of the latest successful snapshot. It is reset to zero when inspection is unavailable or the observer stops.

## Cardinality and privacy boundary

Metrics never expose or label by:

- delivery UUID;
- stream, topic, partition, or offset;
- payload or payload hash;
- decode error classification;
- publisher identity or lease timestamp;
- tenant, actor, relation, or event identity.

Logs contain only the same aggregate counts plus a bounded stable error code when inspection fails.

## Operator policy

The repository does not prescribe universal alert thresholds. Operators must choose thresholds from deployment traffic, poll interval, retry policy, and broker deduplication retention.

Useful conditions to evaluate externally include:

- snapshot availability remaining zero beyond the expected database recovery window;
- `expired_publishing` remaining non-zero across multiple polls;
- `reserved` or `publishing` increasing while consumer lag also increases;
- `published` remaining non-zero, which indicates acknowledgement-only recovery work;
- sustained growth in `total` relative to the deployment's receipt retention policy.

Metrics are diagnostic inputs only. An alert or dashboard must not trigger automatic reclaim, acknowledgement, deletion, authorization, or privacy decisions.

## Maintainer verification

```bash
RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets
cargo test -p rustok-telemetry consumer_poison_metrics -- --nocapture
cargo test -p rustok-server social_graph_index_poison_observer -- --nocapture
node scripts/verify/verify-social-graph-index-poison-observer.mjs
```

These commands are maintainer-run. They were not executed when the source slice was authored.
