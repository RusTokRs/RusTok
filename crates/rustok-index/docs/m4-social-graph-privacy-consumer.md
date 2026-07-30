# M4 Social Graph privacy parity shadow

Date: 2026-07-30

Status: `source_complete_metrics_execution_pending`

This slice provides the first production-shaped consumer of `SharedIndexQueryRuntime`
without granting an eventually consistent projection privacy authority. Social Graph remains
the sole decision source for notification block/mute policy; Index is queried only as a
comparison shadow.

The shadow is default-off and activates only with
`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED=true`. It exists to collect operational
parity signals before any authoritative cutover is considered.

## Typed Index adapter

`IndexSocialGraphPrivacyReadPort` implements the existing
`SocialGraphPrivacyReadPort` contract. It receives only the neutral
`SharedIndexQueryRuntime`; it does not receive a database connection, construct
`PostgresIndexQueryPort`, or read `social_graph_relations`.

The adapter builds typed queries against the owner-published
`rustok-social-graph::relation@1` schema:

- block checks `relation_kind = block` and accept a block in either direction;
- mute remains directional from notification recipient to actor;
- point follow remains directional and retains source-actor authorization;
- bounded follow batches retain the 100-target maximum, deduplicate input, use typed `In`,
  validate every projected UUID, and return deterministic sorted IDs.

Inactive relations are absent because the owner projection publishes an Index tombstone for
an inactive revision. Relation revision remains Index `source_version`; the adapter does not
invent a selectable revision field.

## Non-authoritative shadow and observation contract

`IndexShadowSocialGraphPrivacyReadPort` wraps the authoritative owner port plus the typed
Index projection adapter. Every method executes the owner read first. If that read fails, the
owner error is returned and no projection result can replace it. After owner success, the
shadow executes the equivalent Index read and always returns the owner result.

The shadow measures the original port call before entering the owner read and gives the
projected comparison only the remaining caller deadline budget. If the owner consumes the
budget or the projected query does not finish in the remaining time, the comparison becomes
the existing retryable `social_graph.index_privacy_unavailable` observation. The owner result
is still returned, so a non-authoritative Index comparison cannot extend notification policy
past the caller's declared budget.

The Social Graph crate classifies the comparison into a neutral
`IndexPrivacyShadowObservation`. The observation contains only a fixed operation, fixed
outcome, comparison duration, optional bounded failure code, and optional retryability. The
owner crate does not import `rustok-telemetry` and does not know about Prometheus.

Boolean outcomes are `match_positive`, `match_negative`, `false_negative`, and
`false_positive`. A `false_negative` means authoritative Social Graph returned true while
Index returned false; this is the critical negative-result safety signal for a future privacy
cutover.

Follow-batch IDs are compared as sets and classified as `match_batch_empty`,
`match_batch_nonempty`, `batch_missing`, `batch_extra`, or `batch_mixed`. `batch_missing`
means Index omitted at least one owner-confirmed target; `batch_extra` means Index returned
only additional targets; `batch_mixed` contains both directions of drift.

Index mismatch, missing tenant schema, storage failure, deadline exhaustion, or contract error
remains observational. It never authorizes, suppresses, widens, or otherwise changes
notification policy. Logs contain operation, fixed outcome, booleans/counts, bounded failure
code, retryability, and comparison duration. They contain no tenant, user, relation, entity,
payload, SQL, or storage details.

## Host-owned Prometheus adapter

`rustok-server` owns the adapter from `IndexPrivacyShadowObservation` to
`rustok-telemetry`. This keeps the Social Graph owner contract telemetry-neutral while using
the single Prometheus registry for the process.

The collector publishes four metric families:

- `rustok_social_graph_index_privacy_shadow_observations_total{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_failures_total{operation,error_code,retryable}`;
- `rustok_social_graph_index_privacy_shadow_comparison_duration_seconds{operation,outcome}`;
- `rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds{operation,outcome}`.

Projection failures use the observation outcome `error` plus a failure counter. Error labels
are bounded to `social_graph.index_privacy_unavailable`,
`social_graph.index_privacy_contract_invalid`, or `other`. Tenant and user identifiers,
relation/entity IDs, payloads, SQL, and raw storage errors are forbidden labels.

Enabling the shadow requires successful collector registration. Metrics-disabled or
uninitialized telemetry fails shadow activation rather than silently running an unmeasured
evidence mode. These metrics are measurement source only; this slice does not define a scrape
window, minimum sample count, pass threshold, retained capture bundle, review report, or
admission receipt.

## Server composition

The final server facade:

1. completes ordinary provider composition;
2. materializes `SharedIndexQueryRuntime` from the source-owned registry and host database;
3. parses the default-off shadow flag;
4. when disabled, publishes the unchanged owner-backed notification policy;
5. when enabled, requires successful privacy-shadow metrics registration;
6. requires the shared query runtime and recomposes the policy with
   `IndexShadowSocialGraphPrivacyReadPort` plus the host-owned Prometheus adapter;
7. preserves explicitly registered `NotificationBlockReadRuntime` and
   `NotificationMuteReadRuntime` overrides.

An invalid flag, unavailable metrics registry, or missing runtime while shadow is enabled
fails bootstrap. The shadow still uses the authoritative owner result for every policy
decision.

## Why authoritative cutover remains blocked

A successful Index query can still be stale without returning an error. Therefore an absent
projected block or mute cannot safely prove absence in authoritative Social Graph storage.
Schema readiness alone does not provide per-tenant catch-up, bounded lag, or negative-result
safety.

Before enabling the shadow broadly or considering cutover, the owner must retain evidence for:

- current per-tenant projection watermark and bounded lag;
- replay/repair behavior after missing or corrupted projection state;
- positive and negative block/mute result parity;
- behavior during worker outage, reconnect, restart, and backlog catch-up;
- acceptable shadow latency and resource overhead;
- an explicit fail-closed freshness policy for negative projection results;
- one reviewed and admitted scrape/capture window from the bounded metrics above.

## Boundary

This slice does not:

- make Index authoritative for notification privacy;
- change the policy result based on Index mismatch, timeout, or failure;
- allow the projected comparison to exceed the remaining caller deadline budget;
- activate the shadow by default;
- move Social Graph source authority or replay ownership into Index;
- change commands, revision checks, event publication, projection, or DLQ handling;
- use Index for revision-bearing `SocialGraphFollowReadPort` results;
- cut profile privacy, presentation authorization, GraphQL, storefront, or admin reads over;
- enable the Social Graph projection worker or notification candidate worker by default;
- claim PostgreSQL execution, live parity, freshness, latency, or retained evidence;
- define alert thresholds or authorize an authoritative cutover;
- add many-link aggregate ordering or another source schema.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-telemetry social_graph_index_privacy_shadow_metrics -- --nocapture
cargo test -p rustok-social-graph --features index index_privacy -- --nocapture
cargo test -p rustok-server host_materializes_index_query_runtime_after_source_registry -- --nocapture
cargo check -p rustok-telemetry --all-targets
cargo check -p rustok-social-graph --features index --all-targets
cargo check -p rustok-server --all-targets
node scripts/verify/verify-index-social-graph-privacy-consumer.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
cargo xtask module validate social_graph
```
