# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns profile storage, translations, tags, handles, visibility
policy, owner reads, audience-bound presentation, summary batching,
self-service GraphQL, `profile.updated`, owner-local backfill, Media-backed image
presentation, and the module-owned storefront.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, search,
broker, receipt, or telemetry aggregate. Public GraphQL, Customer Admin
enrichment, Blog/Forum author cards, and storefront reads evaluate privacy before
localized presentation and hide restricted or unavailable rows as absent.

`followers_only` resolves through authoritative bounded Social Graph owner ports.
Profiles never reads relation tables and never authorizes from events, Index
state, DLQ receipts, broker identifiers, offsets, lag, poison health,
PostgreSQL/Iggy evidence, deduplication observations, or duplicate-scan modes.

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

- Durable command receipts bind a tenant-scoped normalized idempotency key to one
  complete command identity and share a transaction with relation mutation,
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
- Retained packets must omit credentials and delivery-level facts, bind reviewed
  source hashes/configuration digests, and become stale when bound sources
  change.

## Physical DLQ duplicate inspection

Two bounded policies are source-complete:

- `global_budget`: one ordered partition allowlist and one shared cap. A busy
  early partition may prevent later partitions from being polled.
- `fair_window`: one equal cap for every selected partition, checked total
  `partition_count * per_partition_messages <= 10000`, and one combined
  identifier-free classification.

The event-delivery observer keeps `global_budget` as the compatibility default
and accepts explicit `fair_window` only for `outbox_iggy`:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES=<positive cap>
```

`memory` and `outbox_local` remain intentional not-applicable modes and do not
resolve Iggy. Both policies use explicit offsets and `auto_commit=false`. Every
observer cycle reuses the configured start offset; neither policy owns moving
cursors, stored progress, cross-cycle duplicate state, current-tail coverage, or
complete-history semantics.

### Production partition invariant

`IggyDlqPublisher` routes deterministic physical DLQ messages by:

```text
partition = (broker_message_id_as_u128 mod partition_count) + 1
```

Production copies carrying the same broker UUID are therefore colocated in one
partition. The scanner still combines observations from every requested
partition, but production runtime evidence must not claim that
`IggyTransport::move_to_dlq` split one deterministic ID across partitions.

The fair-window runtime source uses a production-reachable fixture instead:

```text
partition 1: A/A ordinary duplicate plus one unique overflow message
partition 2: B1/B2 conflicting-payload duplicate
```

With a cap of two messages per partition, the fair scan observes both duplicate
groups and the conflict. The ordered global request with four total slots reads
three messages from partition 1 and only one from partition 2, producing a
different summary. The fair policy runs twice from offset zero, and stored
standalone-consumer offsets must remain absent on both partitions.

Execution and retained capture remain pending.

Detailed checkpoints:

- `crates/rustok-profiles/docs/poison-duplicate-external-scan-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-fair-window-external-runtime-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md`
- `crates/rustok-iggy/docs/dlq-duplicate-external-scan.md`
- `crates/rustok-iggy/docs/dlq-duplicate-fair-window-external-scan-runtime-evidence.md`
- `crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md`

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

7. **Add retained fair-window capture.**
   Require a clean unchanged commit, the exact one-case Cargo command, no skip,
   two absent-offset partitions, current source hashes, bounded toolchain/server
   labels, and atomic privacy-safe packet writing.

8. **Compose receipt-plus-broker recovery evidence.**
   Prove claim -> exact publish -> durable `published` -> source ack ->
   best-effort `acknowledged`, process loss, acknowledgement-only recovery, and
   multi-replica ownership on PostgreSQL plus real Iggy.

9. **Prove confirmation-window sufficiency.**
   Compare dedup `max_entries`/`expiry` against maximum lease, restart,
   reconnect, and operator recovery horizons before making a stronger duplicate
   guarantee.

10. **Design moving duplicate windows or keep fixed snapshots.**
    A moving per-partition cursor must retain bounded prior identity/digest state
    so copies split across cycles remain related. Fixed snapshots must not be
    presented as current-tail or complete-history evidence.

11. **Retain production operations.**
    Prove bundled mode, restart, TLS/auth/failover, reconnect, rebalance,
    retention, reconciliation, operator cleanup, and bounded replay/rescan
    repair.

## Recheck checkpoint — 2026-07-28

- Rechecked the canonical plan and current `main` after the fair-window server
  integration.
- Reconfirmed privacy-before-presentation, owner-scoped writes, Media ownership,
  fail-closed follower reads, no automatic mutation retry, and the rule that
  operational state never authorizes profile presentation.
- Rechecked production deterministic DLQ partitioning and corrected the planned
  runtime claim: production same-ID copies are colocated and are not a valid
  cross-partition fixture.
- Added a two-partition fair-window source harness that proves equal partition
  budgets by comparing fair and global identifier-free summaries.
- Locked absent standalone-consumer offsets for both partitions across repeated
  fair scans and the compatibility global scan.
- Kept runtime execution, retained fair-window capture, moving-window state,
  production recovery-window sufficiency, bundled/TLS/auth, and multi-replica
  claims open.
- Tests, Cargo commands, formatters, repository source verifiers, PostgreSQL,
  external/bundled Iggy, and multi-replica scenarios were not run per maintainer
  instruction.

## Verification backlog

```text
cargo xtask module validate profiles
cargo xtask module test profiles
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
cargo test -p rustok-iggy dlq_duplicate_external_scan -- --nocapture
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS='host:8090' \
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_fair_window_external_scan -- \
  fair_window_scans_each_partition_and_differs_from_global_budget \
  --exact --nocapture --test-threads=1
cargo test -p rustok-iggy dlq_duplicate_alert_observer -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
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
    claims, credentials, and provider details.
13. Short dedup or scan sequences do not prove production-window sufficiency.
14. Production deterministic broker IDs remain colocated by the publisher's
    one-based modulo partition rule.
15. Moving duplicate windows require bounded cross-cycle identity state.
16. Update Profiles and affected owner docs with every boundary change.
