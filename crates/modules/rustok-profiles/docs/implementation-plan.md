# Implementation plan for `rustok-profiles`

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`

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

`followers_only` resolves through authoritative bounded Social Graph owner ports.
Profiles never reads relation tables and never authorizes from events, Index
state, DLQ receipts, broker identifiers, offsets, lag, poison health,
PostgreSQL/Iggy evidence, deduplication observations, duplicate-scan modes,
alert levels, duplicate-observer health, or Prometheus metrics.

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

- `PersistentContractConsumerGroup::receive_delivery` returns either a validated
  event or exact-byte `ConsumedContractDecodeFailure` without committing the
  source cursor.
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

### Evidence tooling

- Isolated PostgreSQL concurrency, reclaim fencing, collision rollback,
  diagnostic retention, aggregate consistency, clean-commit capture, source
  hashing, and strict retained-packet verification are source-complete;
  canonical execution is pending.
- External Iggy reconnect/redelivery, exact-byte DLQ, physical UUID/u128 header,
  one-based partition, and four dedup behavior scenarios are source-complete and
  runtime-pending.
- The compatibility global duplicate-scan harness is source-complete for one
  partition.
- The fair-window duplicate-scan harness is source-complete for two partitions
  and compares equal per-partition budgets with the ordered global budget.
- Fair-window clean-commit retained capture is source-complete: exact-case
  execution, reviewed dedup-disabled configuration projection, current source
  hashes, aggregate two-partition absent-offset assertions, and no-clobber packet
  publication are locked.
- A pure Iggy dedup recovery-window policy is source-complete. It checked-sums
  caller-reviewed publication-lease, restart, reconnect, and operator-recovery
  bounds; requires an explicit maximum distinct-ID count per physical partition;
  distinguishes disabled, expiry, capacity, combined, and sufficient states; and
  contains no production default or exactly-once claim.
- Recovery-window retained calibration tooling is source-complete. A versioned
  external bounds file and reviewed enabled Iggy configuration are reduced to
  canonical privacy-safe projections; one exact Rust case must report a
  sufficient assessment from a clean unchanged commit before a no-clobber packet
  can be published.
- A bounded physical-DLQ rolling window is source-complete. It retains complete
  opaque cycles under a checked 10,000-observation cap, rejects oversized input
  transactionally, and permanently marks truncation after cycle eviction.
- The moving-window scanner integration is source-complete. It owns independent
  process-local per-partition cursors and updates all cursors plus rolling state
  only after one complete equal-budget cycle succeeds.
- The moving-window server observer composition is source-complete. `moving_window`
  is explicit opt-in, `global_budget` remains default, and reviewed fail-closed
  configuration is required for initial offset, per-partition cap, batch size,
  rolling maximum cycles, and rolling per-cycle observations.
- The locked moving-observer external-Iggy capture is source-complete. It places
  identical physical copies in advancing cycles of one production-selected
  partition, checks absent stored offsets, and records replacement-observer reset
  semantics without claiming restart-safe progress.
- Identifier-free duplicate-alert observability is source-complete. A read-only
  companion projects the existing latest-value observer handle into bounded
  Prometheus series and an optional health snapshot without readiness coupling.
- Canonical retained packets remain pending and must omit credentials and
  delivery-level facts, bind reviewed source/configuration/input digests, and
  become stale when any bound source or reviewed input changes.

## Physical DLQ duplicate inspection

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

`outbox` remains an intentional not-applicable mode and does not resolve
Iggy. All modes use explicit offsets and `auto_commit=false`; no broker
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

The full checked fair-cycle budget must fit one rolling cycle. A failed moving
cycle marks the public alert runtime unavailable but preserves the connected
observer's process-local cursors and rolling history for the next attempt. A new
process or replacement connection starts at the reviewed initial offset with
empty rolling history because progress is not persisted.

No mode claims restart-safe progress, current-tail coverage, complete history,
production retention sufficiency, or exactly-once delivery.

### Production partition invariant

`IggyDlqPublisher` routes deterministic physical DLQ messages by:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Production copies carrying the same broker UUID are therefore colocated in one
partition. Runtime evidence must not claim that
`IggyTransport::move_to_dlq` split one deterministic ID across partitions.

The fair-window fixture remains production-reachable:

```text
partition 1: A/A ordinary duplicate plus one unique overflow message
partition 2: B1/B2 conflicting-payload duplicate
```

The moving external-Iggy cross-cycle fixture splits copies across advancing
cycles in the same production-selected partition, not across partitions.

### Fair-window retained capture

Source-complete paths:

```text
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-fair-window-external-scan-runtime-source.json
  dlq-duplicate-fair-window-external-scan-execution-contract.json
scripts/evidence/
  capture-iggy-dlq-duplicate-fair-window-external-scan.mjs
scripts/verify/
  verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
  verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
```

The runner requires a clean unchanged commit, one exact passing case, no skip,
unchanged bound-source hashes, reviewed external Iggy and dedup-disabled
configuration labels, and a clean worktree after the test. Publication is
no-clobber. Runtime execution and the canonical packet remain pending.

### Recovery-window retained calibration

Source-complete paths:

```text
crates/rustok-iggy/contracts/evidence/
  dedup-recovery-window-policy-source.json
  dedup-recovery-window-calibration-execution-contract.json
crates/rustok-iggy/tests/
  dedup_recovery_window_calibration.rs
scripts/evidence/
  capture-iggy-dedup-recovery-window-calibration.mjs
scripts/verify/
  verify-iggy-dedup-recovery-window-policy.mjs
  verify-iggy-dedup-recovery-window-retained.mjs
```

The sufficient-only capture binds reviewed recovery bounds, reviewed enabled Iggy
configuration, clean unchanged source, canonical privacy-safe projections, and
no-clobber publication. Runtime calibration and the canonical packet remain
pending.

### Bounded rolling, moving scanner, and server observer

Source-complete paths:

```text
crates/rustok-iggy/src/
  dlq_duplicate_rolling_window.rs
  dlq_duplicate_moving_window_scan.rs
  dlq_duplicate_alert_observer.rs
apps/server/src/services/
  event_dlq_duplicate_alert_observer.rs
crates/rustok-iggy/contracts/evidence/
  dlq-duplicate-rolling-window-source.json
  dlq-duplicate-moving-window-scan-source.json
  dlq-duplicate-alert-server-observer-source.json
scripts/verify/
  verify-iggy-dlq-duplicate-rolling-window.mjs
  verify-iggy-dlq-duplicate-moving-window-scan.mjs
  verify-event-dlq-duplicate-alert-server-observer.mjs
```

The moving scanner keeps partition IDs, cursor values, message identities,
payloads/digests, and opaque observations private. The server reduces successful
moving snapshots to the existing count-only duplicate summary before alert
policy evaluation.

### Moving-observer retained capture

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

The locked fixture records one unique first cycle, a cross-cycle ordinary
physical duplicate after the second copy, an unchanged empty third cycle, a
replacement observer reset to the reviewed initial offset, and absent stored
consumer offsets at every checkpoint. Canonical execution is pending.

### Duplicate-alert observability

The server-owned companion reads only the existing public observer mode, latest
runtime snapshot, and task-finished state. It exports no identifiers or source
counts and does not modify scanner configuration, retries, moving cursors, or
event delivery.

Bounded health states:

```text
disabled
not_applicable
starting
available
unavailable
stopped
```

Prometheus families:

```text
rustok_dlq_duplicate_alert_observer_state
rustok_dlq_duplicate_alert_snapshots_total
rustok_dlq_duplicate_alert_evaluation_flags
```

Labels are closed deployment, scan-mode, state, availability, level, and
evaluation-flag domains. State is recorded only on transition; snapshot counters
and flags are recorded only when the runtime generation changes. The companion
does not infer connect/scan/publish failure stages from an unavailable snapshot.

The health projection reports `affects_readiness = false` and is not inserted
into `/health/ready`, liveness, Profiles authorization, or event-delivery gating.

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

Detailed checkpoints:

- `crates/rustok-profiles/docs/poison-duplicate-external-scan-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-fair-window-external-runtime-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md`
- `crates/rustok-profiles/docs/poison-dedup-recovery-window-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-rolling-window-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-moving-window-scan-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-moving-window-external-observer-runtime-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-alert-observability-checkpoint.md`
- `crates/rustok-iggy/docs/dlq-duplicate-external-scan.md`
- `crates/rustok-iggy/docs/dlq-duplicate-fair-window-external-scan-runtime-evidence.md`
- `crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md`
- `crates/rustok-iggy/docs/dlq-duplicate-moving-window-external-observer-runtime-evidence.md`
- `crates/rustok-iggy/docs/dlq-duplicate-alert-observability.md`
- `crates/rustok-iggy/docs/dedup-recovery-window-policy.md`
- `crates/rustok-iggy/docs/dlq-duplicate-rolling-window.md`
- `crates/rustok-iggy/docs/dlq-duplicate-moving-window-scan.md`

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   Source-complete for current consumers; retain privacy-before-presentation
   ordering.

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
   Run reconnect/redelivery, physical header/partition, dedup behavior,
   compatibility global duplicate scan, and the two-partition fair-window case
   on reviewed disposable brokers. Retain separate privacy-safe packets and
   reviewed configuration digests.

7. **Execute and retain the fair-window packet.**
   Run the locked capture on one clean unchanged commit, inspect the generated
   no-clobber packet, commit it, and rerun the retained verifier. Source changes
   invalidate the packet.

8. **Compose receipt-plus-broker recovery evidence.**
   Prove claim -> exact publish -> durable `published` -> source ack ->
   best-effort `acknowledged`, process loss, acknowledgement-only recovery, and
   multi-replica ownership on PostgreSQL plus real Iggy.

9. **Execute recovery-window calibration.**
   Review the production lease, restart, reconnect, operator-response, and
   per-partition distinct-ID bounds; run the locked sufficient-only capture
   against a reviewed enabled Iggy configuration; inspect and commit the
   no-clobber packet; and repeat whenever a bound source, configuration, or input
   changes. A packet covers only that supplied model.

10. **Execute moving duplicate observer evidence and retain it.**
    Run the locked external-Iggy cross-cycle capture, inspect and commit the
    no-clobber packet, and repeat whenever a bound source or reviewed input
    changes. Review initial offset and acceptable reset frequency per deployment.

11. **Execute observability evidence.**
    Run focused source tests and verifier, then retain one reviewed Prometheus
    scrape and identifier-free health projection. Confirm no readiness impact.

12. **Retain production operations.**
    Prove bundled mode, restart, TLS/auth/failover, reconnect, rebalance,
    retention, reconciliation, operator cleanup, and bounded replay/rescan
    repair. Add persistent cursor ownership only if restart continuity is
    required, and define alert routing/cooldown/suppression separately.

## Recheck checkpoint — 2026-07-29

- Rechecked current `main` after PR #2431 and confirmed the locked moving
  cross-cycle fixture is source-complete while canonical execution remains
  pending.
- Reconfirmed privacy-before-presentation, owner-scoped writes, Media ownership,
  fail-closed follower reads, no automatic mutation retry, and the rule that
  operational state never authorizes profile presentation.
- Added three bounded Prometheus families to the single `rustok-telemetry`
  registry and kept every label inside a closed enum.
- Added a separate read-only companion for disabled, not-applicable, starting,
  available, unavailable, and stopped health states.
- Recorded state only on transitions and snapshots only after generation changes;
  did not invent a failure stage from an unavailable latest-value snapshot.
- Kept duplicate-observer health out of readiness/liveness and event-delivery
  gating, and left the observer's moving-state preservation unchanged.
- Kept runtime metrics scrape, health capture, external-Iggy execution,
  canonical packets, persistent cursor ownership, alert routing,
  bundled/TLS/auth/failover, and multi-replica claims open.
- Tests, Cargo commands, formatters, repository source verifiers, server startup,
  metrics scrape, broker scans, retained capture, and multi-replica scenarios
  were not run per maintainer instruction.

## Verification backlog

```text
cargo xtask module validate profiles
cargo xtask module test profiles
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront
RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets
cargo test -p rustok-telemetry dlq_duplicate_alert_metrics -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
cargo test -p rustok-iggy dlq_duplicate_rolling_window -- --nocapture
cargo test -p rustok-iggy dlq_duplicate_moving_window_scan --features iggy -- --nocapture
cargo test -p rustok-iggy dlq_duplicate_alert_observer --features iggy -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observability -- --nocapture
node scripts/verify/verify-event-dlq-duplicate-alert-observability.mjs
node scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
cargo test -p rustok-iggy dedup_recovery_window_policy -- --nocapture
cargo test -p rustok-iggy --test dedup_recovery_window_calibration
node scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs
node scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs
RUSTOK_IGGY_DEDUP_RECOVERY_BOUNDS_PATH=/outside/repository/bounds.json \
RUSTOK_IGGY_DEDUP_RECOVERY_CONFIG_PATH=/outside/repository/iggy.toml \
RUSTOK_IGGY_DEDUP_RECOVERY_SERVER_ARTIFACT=reviewed-iggy-build \
node scripts/evidence/capture-iggy-dedup-recovery-window-calibration.mjs
cargo test -p rustok-iggy dlq_duplicate_external_scan -- --nocapture
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS='host:8090' \
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_fair_window_external_scan -- \
  fair_window_scans_each_partition_and_differs_from_global_budget \
  --exact --nocapture --test-threads=1
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-moving-window-external-observer-retained.mjs
RUSTOK_IGGY_MOVING_OBSERVER_TEST_ADDRESS='host:8090' \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_CONFIG_PATH=/outside/repository/iggy.toml \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_RESET_REVIEW_PATH=/outside/repository/reset-review.json \
RUSTOK_IGGY_MOVING_OBSERVER_TEST_SERVER_ARTIFACT=reviewed-iggy-build \
node scripts/evidence/capture-iggy-dlq-duplicate-moving-window-external-observer.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
```

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain
   internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. Profile media resolves through Media owner ports only.
7. Follow controls use owner ports, unique idempotency, optimistic revision, and
   no automatic retry.
8. Index/broker/receipt/metric/evidence state never authorizes visibility.
9. Durable workers recognize/persist the owner result before source
   acknowledgement.
10. Exact-byte DLQ publication and terminal receipt persistence precede source
    acknowledgement.
11. Deterministic IDs bind immutable source identity and exact payload but do not
    imply exactly-once without retained broker evidence.
12. Operational telemetry excludes identities, payloads, broker coordinates,
    claims, credentials, threshold values, source counts, timestamps, and raw
    errors.
13. Short dedup sequences do not prove production-window sufficiency; use a
    checked additive recovery horizon and an explicit per-partition capacity
    bound.
14. Production deterministic broker IDs remain colocated by the publisher's
    one-based modulo partition rule.
15. Retained evidence is no-clobber, commit-bound, and stale after any bound
    source or reviewed input change.
16. Cross-cycle duplicate state retains complete bounded cycles; partial silent
    eviction is forbidden and any eviction permanently marks truncation.
17. Moving cursors advance atomically only after every selected partition and the
    complete rolling cycle succeed.
18. Moving mode is explicit opt-in with reviewed fail-closed configuration;
    `global_budget` remains the compatibility default.
19. Cursor values, partition identities, message identities, payloads, digests,
    and observations never surface in public moving or alert snapshots.
20. Replacement connection or process restart resets moving state to one reviewed
    initial offset unless a separately reviewed persistent owner is added.
21. A sufficient recovery-window assessment covers only the supplied reviewed
    model and never authorizes Profiles or proves exactly-once.
22. Duplicate-alert metrics use only closed labels and health never affects
    readiness, liveness, event delivery, or Profiles authorization.
23. Update Profiles and affected owner docs with every boundary change.
