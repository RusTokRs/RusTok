# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns profile storage, translations, tags, handles, visibility
policy, owner reads, audience-bound presentation, summary batching,
self-service GraphQL, `profile.updated`, owner-local backfill, Media-backed image
presentation, and the module-owned storefront.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, search,
broker, receipt, duplicate-observer, health, or telemetry aggregate. Public
GraphQL, Customer Admin enrichment, Blog/Forum author cards, and storefront
reads evaluate privacy before localized presentation and hide restricted or
unavailable rows as absent.

`followers_only` resolves through authoritative bounded Social Graph owner
ports. Profiles never reads relation tables and never authorizes from events,
Index state, DLQ receipts, broker identifiers, offsets, lag, poison health,
PostgreSQL/Iggy evidence, duplicate-scan modes, alert levels, health snapshots,
or Prometheus metrics.

Media descriptors remain Media-owned. Profiles validates tenant, uploader, and
MIME constraints and exposes only Media-selected descriptors; storage keys,
provider endpoints, and ingress construction remain outside Profiles.

The storefront mounts `/modules/profiles?handle=<handle>`, supports SSR-first
native and explicit GraphQL transports, package i18n, approved avatar/banner
descriptors, and authenticated follow/unfollow with unique idempotency keys,
optimistic revisions, and one read-only conflict refresh without automatic
mutation retry.

## Delivered boundaries

### Social Graph and Index

- Durable command receipts bind one tenant-scoped normalized idempotency key to
  one complete command identity and share a transaction with relation mutation,
  optional event append, response snapshot, and completion.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact.
  No-op and receipt replay emit nothing; event failure rolls owner state back.
- Bounded replay is service/system-only, tenant-scoped, page-atomic, and driven
  by an exclusive UUID cursor. Social Graph persistence remains the repair
  source.
- Index registration and projection use Index-owned contracts and persistence;
  `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- Runtime consumers are default-off, require a worker host plus `outbox_iggy`,
  and reuse the single `Arc<IggyTransport>` owned by `EventRuntime`.
- Index/search projections can improve discovery but never authorize profile
  visibility.

### Raw decode failures and poison receipts

- `PersistentContractConsumerGroup::receive_delivery` returns either a
  validated event or exact-byte `ConsumedContractDecodeFailure` without
  committing the source cursor.
- Stable failure classes are bounded; deterministic RFC 9562 UUIDv8 identity
  derives only from source coordinates and exact payload. No tenant/domain
  identity is invented.
- Neutral durable receipts recognize or reserve work before publication, fence
  claims, retain first diagnostics, and separate `published` from post-source
  commit `acknowledged` bookkeeping.
- The approved order remains receipt recognition/claim, exact-byte DLQ
  publication, durable `published`, exact source acknowledgement, then
  best-effort `acknowledged`.
- Existing terminal receipts remain recoverable after later policy disablement.
- Count-only receipt inspection, stale clearing, metrics, and degraded observer
  health remain operational signals only and never become readiness or
  authorization.

### Physical DLQ duplicate inspection

Three bounded server scan modes are source-complete:

- `global_budget`: one ordered partition allowlist and one shared cap. A busy
  early partition may prevent later partitions from being polled.
- `fair_window`: one equal cap for every selected partition, checked total
  `partition_count * per_partition_messages <= 10000`, and one combined
  identifier-free fixed-snapshot classification.
- `moving_window`: one private process-local next offset per partition, one
  complete equal-budget candidate before mutation, and one atomic rolling-cycle
  update preserving duplicate relationships across advancing cycles.

The event-delivery observer keeps `global_budget` as the compatibility default:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=moving_window
```

`memory` and `outbox_local` are intentional not-applicable modes and do not
resolve Iggy. All modes use explicit offsets and `auto_commit=false`; no broker
consumer offset is stored.

Moving mode has no production defaults and requires reviewed fail-closed
configuration:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_CYCLES
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_OBSERVATIONS_PER_CYCLE
```

A failed moving cycle marks the public alert runtime unavailable but preserves
the connected observer's private cursors and rolling history. A replacement
connection or process restart begins at the reviewed initial offset with empty
rolling history because progress is not persisted.

No scan mode claims restart-safe progress, current-tail coverage, complete
history, production retention sufficiency, or exactly-once delivery.

### Production partition invariant

`IggyDlqPublisher` routes deterministic physical DLQ messages by:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Production copies carrying the same broker UUID are colocated in one partition.
Runtime evidence must not claim that `IggyTransport::move_to_dlq` split one
deterministic ID across partitions.

### Evidence tooling

Source-complete retained tooling includes:

- isolated PostgreSQL poison-receipt concurrency, reclaim fencing, collision
  rollback, diagnostics, clean-commit capture, source hashing, and strict packet
  verification;
- external Iggy reconnect/redelivery, exact-byte DLQ, physical UUID/u128 header,
  partition routing, dedup behavior, global scan, and two-partition fair-window
  scenarios;
- checked dedup recovery-window assessment and retained calibration tooling;
- bounded rolling-window state with complete-cycle eviction;
- process-local moving scanner and explicit server observer composition;
- locked external-Iggy moving-observer cross-cycle fixture and no-clobber capture.

The moving-observer fixture uses one production-selected partition:

```text
cycle 1: first physical copy, unique summary
cycle 2: second identical copy, cross-cycle duplicate summary
cycle 3: no new copy, rolling summary retained
replacement observer: reset to reviewed initial offset
stored consumer offsets: absent at every checkpoint
```

Source-complete paths:

```text
crates/rustok-iggy/tests/
  dlq_duplicate_moving_window_external_observer.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-moving-window-external-observer-runtime-source.json
  dlq-duplicate-moving-window-external-observer-execution-contract.json
scripts/evidence/
  capture-iggy-dlq-duplicate-moving-window-external-observer.mjs
scripts/verify/
  verify-iggy-dlq-duplicate-moving-window-external-observer-runtime.mjs
  verify-iggy-dlq-duplicate-moving-window-external-observer-retained.mjs
```

Canonical execution remains pending and must use reviewed disposable external
Iggy, dedup-disabled configuration, a reviewed reset-frequency decision, a clean
unchanged commit, current source hashes, one exact non-skipped passing case, and
no-clobber packet publication.

### Duplicate alert observability

Identifier-free telemetry and optional health projection are source-complete.

The health projection exposes only:

```text
deployment mode
scan mode
bounded lifecycle state
runtime generation
alert level
physical-duplicate flag
identity-conflict flag
task-finished flag
```

Bounded lifecycle states are `disabled`, `not_applicable`, `starting`,
`available`, `unavailable`, and `stopped`.

Prometheus families are:

```text
rustok_dlq_duplicate_alert_observer_state
rustok_dlq_duplicate_alert_snapshots_total
rustok_dlq_duplicate_alert_evaluation_flags
```

All labels are closed enums for deployment, scan mode, lifecycle state,
availability, level, and evaluation flag. Message identity, tenant, broker
coordinates, offsets, payloads/digests, receipts, credentials, threshold values,
source counts, timestamps, and raw errors are excluded.

Duplicate observability does not participate in `/health/ready`, liveness,
event-delivery gating, module authorization, Profiles policy, notification
routing, or destructive reconciliation.

Source-complete paths:

```text
crates/rustok-telemetry/src/
  dlq_duplicate_alert_metrics.rs
apps/server/src/services/
  event_dlq_duplicate_alert_observer.rs
  event_dlq_duplicate_alert_observability.rs
  server_bootstrap.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-alert-observability-source.json
scripts/verify/
  verify-event-dlq-duplicate-alert-observability.mjs
crates/rustok-iggy/docs/
  dlq-duplicate-alert-observability.md
crates/rustok-profiles/docs/
  poison-duplicate-alert-observability-checkpoint.md
```

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   Retain privacy-before-presentation ordering and owner-port authorization.

2. **Finish compiled/runtime presentation evidence.**
   Execute storefront, Customer Admin, Blog/Forum, Media, privacy, replay, and
   schema-concurrency scenarios without foreign-table reads.

3. **Publish and retain storefront evidence.**
   Cover SSR/hydration/GraphQL, auth, i18n, Media direct/proxy/fallback, follow
   conflict, durable receipts/events, and accessibility.

4. **Keep profile backfill owner-local.**
   Preserve dry-run semantics and owner auth/tenant/customer reads; retain
   runtime proof.

5. **Execute retained PostgreSQL poison evidence.**
   Run the clean-commit capture, review the bounded packet, commit canonical
   JSON, and repeat whenever a bound source hash changes.

6. **Execute external Iggy evidence.**
   Run reconnect/redelivery, physical header/partition, dedup behavior, global
   duplicate scan, and two-partition fair-window cases on reviewed disposable
   brokers.

7. **Execute recovery-window calibration.**
   Review lease, restart, reconnect, operator-response, and per-partition
   distinct-ID bounds; run and retain the sufficient-only packet.

8. **Execute and retain moving-observer evidence.**
   Run the locked cross-cycle case, inspect and commit the no-clobber packet,
   and repeat whenever a bound source or reviewed input changes.

9. **Execute observability evidence.**
   Run focused source tests and verifier, then retain one reviewed Prometheus
   scrape and identifier-free health projection. Confirm no readiness impact.

10. **Compose receipt-plus-broker recovery evidence.**
    Prove claim -> exact publish -> durable `published` -> source ack ->
    best-effort `acknowledged`, process loss, acknowledgement-only recovery,
    and multi-replica ownership on PostgreSQL plus real Iggy.

11. **Define alert delivery separately.**
    Notification routing, cooldown, suppression, acknowledgement, delete,
    replay, and operator authorization remain outside Profiles, scanner,
    telemetry, and alert-policy/runtime code.

12. **Retain production operations.**
    Prove bundled mode, restart, TLS/auth/failover, reconnect, rebalance,
    retention, reconciliation, cleanup, and bounded replay/rescan repair.
    Add persistent cursor ownership only if restart continuity is required.

## Recheck checkpoint — 2026-07-29

- Rechecked current `main` after PR #2431 and confirmed that the locked moving
  cross-cycle fixture is source-complete while canonical execution remains
  pending.
- Reconfirmed privacy-before-presentation, owner-scoped writes, Media ownership,
  fail-closed follower reads, no automatic mutation retry, and the rule that
  operational state never authorizes profile presentation.
- Added bounded Prometheus families to the single `rustok-telemetry` registry.
- Added a count-free health projection for disabled, not-applicable, starting,
  available, unavailable, and stopped observer states.
- Kept metric labels closed and identifier-free; raw errors and inferred failure
  stages are not Prometheus labels.
- Kept duplicate observer health out of readiness/liveness and event-delivery
  gating.
- Kept runtime metrics scrape, health capture, external-Iggy execution,
  canonical packets, persistent cursor ownership, notification routing,
  bundled/TLS/auth/failover, and multi-replica claims open.
- Tests, Cargo commands, formatters, repository source verifiers, server
  startup, metrics scrape, broker scans, retained capture, and multi-replica
  scenarios were not run per maintainer instruction.

## Verification backlog

```text
cargo xtask module validate profiles
cargo xtask module test profiles
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront

RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets
cargo test -p rustok-telemetry dlq_duplicate_alert_metrics -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
cargo test -p rustok-iggy dlq_duplicate_alert_observer --features iggy -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observability -- --nocapture
node scripts/verify/verify-event-dlq-duplicate-alert-observability.mjs

node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-retained.mjs
RUSTOK_IGGY_MOVING_OBSERVER_TEST_ADDRESS='host:8090' \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_CONFIG_PATH=/outside/repository/iggy.toml \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_RESET_REVIEW_PATH=/outside/repository/reset-review.json \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_SERVER_ARTIFACT=reviewed-iggy-build \
node scripts/evidence/capture-iggy-dlq-duplicate-moving-window-external-observer.mjs

node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs
node scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
```

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. Profile media resolves through Media owner ports only.
7. Follow controls use owner ports, unique idempotency, optimistic revision, and no automatic retry.
8. Index/broker/receipt/metric/evidence state never authorizes visibility.
9. Durable workers persist the owner result before source acknowledgement.
10. Exact-byte DLQ publication and terminal receipt persistence precede source acknowledgement.
11. Deterministic IDs bind source identity and exact payload but do not imply exactly-once.
12. Operational telemetry excludes identities, payloads, coordinates, claims, credentials, threshold values, source counts, and raw errors.
13. Short dedup sequences do not prove production-window sufficiency.
14. Production deterministic broker IDs remain colocated by one-based modulo routing.
15. Retained evidence is no-clobber, commit-bound, and stale after bound source/input changes.
16. Cross-cycle duplicate state retains complete bounded cycles; partial silent eviction is forbidden.
17. Moving cursors advance atomically only after every selected partition and rolling cycle succeed.
18. Moving mode is explicit opt-in; `global_budget` remains the compatibility default.
19. Cursor values, partition/message identities, payloads, digests, and observations never surface publicly.
20. Replacement connection or process restart resets moving state unless a persistent owner is reviewed.
21. Duplicate alert health and metrics never participate in readiness or Profiles authorization.
22. Update Profiles and affected owner docs with every boundary change.
