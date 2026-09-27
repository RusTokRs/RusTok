# Bounded tenant cardinality for shared metrics

- Date: 2026-09-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-telemetry
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Shared Prometheus metrics are process-wide observability contracts. Tenant UUIDs have deployment-scale cardinality and are not bounded by the application schema or operator configuration.

The shared telemetry crate currently uses raw tenant identifiers as metric label values for several counters. A new tenant can therefore create new Prometheus time series without any fixed upper bound. This is unsafe for a reusable shared library because metric cardinality becomes a resource-consumption property of the deployment.

The existing telemetry documentation already treats high-cardinality warnings as an operational concern, but the metric contract did not enforce a bounded tenant dimension.

## Decision

1. Shared Prometheus metrics MUST NOT expose raw tenant identifiers as label values.
2. Metrics that need a tenant-relative dimension MAY use the canonical `tenant_bucket` label.
3. `tenant_bucket` is a deterministic process-independent bucket derived from the canonical tenant identifier and has exactly 256 possible values (`0` through `255`).
4. The bucket function is deterministic for the same canonical tenant identifier across processes and restarts.
5. Shared telemetry MUST NOT use arbitrary request/user-controlled strings as a tenant label.
6. Exact tenant-level attribution remains the responsibility of logs/traces or bounded external observability dimensions; this decision does not require exact tenant identity in Prometheus labels.

## Sources of truth and ownership

- Canonical metric definitions and label contracts: `crates/libs/rustok-telemetry/src/metrics.rs`.
- Canonical bucket derivation: the private telemetry helper in the same crate.
- Tenant identity source: the already resolved tenant identifier supplied by the owning server/module call site.
- Prometheus is a derived observability store; metric labels do not participate in application correctness.

## Invariants

- Every shared tenant metric has a finite maximum of 256 tenant buckets.
- The same tenant identifier always maps to the same bucket.
- No metric label contains the raw tenant UUID after this cutover.
- Non-tenant labels remain unchanged unless a separate decision explicitly changes them.

### Allowed states

- Tenant-aware metric observation using a canonical 0..255 `tenant_bucket` value.
- Tenant-unaware metric observation when exact tenant segmentation is not operationally useful.

### Forbidden states

- A raw tenant UUID in a Prometheus label.
- An unbounded tenant label derived from arbitrary client metadata.
- A bucket algorithm whose mapping changes between process restarts.

## Non-goals

- This decision does not remove tenant identity from structured logs or traces.
- This decision does not change tenant authorization or persistence behavior.
- This decision does not prescribe a particular long-term metrics backend.
- This decision does not retrofit historical Prometheus series.

## Data, transaction, and concurrency boundary

Not applicable. Metrics are derived runtime observations and do not own transactional state.

## Context dimensions

Tenant participates only through the deterministic bounded bucket. Principal, locale, channel, currency and timezone are non-applicable to this metric-label contract unless explicitly represented by another bounded metric dimension.

## Events and projections

Not applicable. Metrics observe existing events and operations; no event or projection becomes authoritative because of this decision.

## Failure semantics

The bucket function must be total for any string accepted by the existing telemetry helper surface. It must never panic, allocate unbounded state, or fall back to the raw identifier.

## Migration and cutover

The metric label name changes from `tenant_id` to `tenant_bucket` for the affected shared metrics. Existing historical series remain in the metrics backend until their normal retention expires.

No compatibility alias with the old raw-tenant label is retained, because retaining it would preserve the unsafe unbounded cardinality contract.

## Alternatives considered

- Keep exact `tenant_id` labels: rejected because cardinality is unbounded.
- Remove tenant dimension completely from every metric: rejected because it unnecessarily removes useful tenant-relative operational visibility.
- Configurable bucket count: rejected because operator-selected cardinality would make the safety bound deployment-dependent.
- Per-tenant allowlists for metrics: rejected because shared library safety would depend on mutable runtime configuration.

## Verification

The implementation must provide regression evidence that:
- tenant identifiers map deterministically to one of 256 buckets;
- raw tenant identifiers are absent from affected metric label values;
- affected metric definitions expose `tenant_bucket`, not `tenant_id`;
- all existing non-tenant label names remain unchanged.

## Consequences

Prometheus series cardinality becomes bounded for tenant-aware shared metrics, at the cost of losing exact tenant identity from metric labels. Exact tenant investigation remains available through logs and traces.

The implementation is intentionally local to `rustok-telemetry`, so module owners do not need to know or reproduce the bucketing algorithm.
