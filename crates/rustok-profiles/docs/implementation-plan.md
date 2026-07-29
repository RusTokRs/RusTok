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
- Canonical retained packets remain pending and must omit credentials and
  delivery-level facts, bind reviewed source/configuration digests, and become
  stale when bound sources change.

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
partition. Runtime evidence must not claim that
`IggyTransport::move_to_dlq` split one deterministic ID across partitions.

The fair-window fixture is production-reachable:

```text
partition 1: A/A ordinary duplicate plus one unique overflow message
partition 2: B1/B2 conflicting-payload duplicate
```

With a cap of two messages per partition, the fair scan observes both duplicate
groups and the conflict. The ordered global request with four total slots reads
three messages from partition 1 and one from partition 2, producing a different
summary. The fair policy runs twice from offset zero, and stored
standalone-consumer offsets must remain absent on both partitions.

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
configuration labels, and a clean worktree after the test.

The packet retains fair/global summaries, four zero stored-offset counts over two
checked partitions, bounded artifact/toolchain labels, source hashes,
timestamps, and test-output digest/size. It excludes endpoints, paths,
credentials, raw output, stream/partition/offset/UUID/payload facts, ack tokens,
and raw Iggy errors.

Publication is no-clobber: an exclusive temporary file is hard-linked to the
canonical path, so existing reviewed evidence cannot be replaced.

Runtime execution and the canonical packet remain pending.

Detailed checkpoints:

- `crates/rustok-profiles/docs/poison-duplicate-external-scan-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-fair-window-external-runtime-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md`
- `crates/rustok-profiles/docs/poison-dedup-recovery-window-checkpoint.md`
- `crates/rustok-iggy/docs/dlq-duplicate-external-scan.md`
- `crates/rustok-iggy/docs/dlq-duplicate-fair-window-external-scan-runtime-evidence.md`
- `crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md`
- `crates/rustok-iggy/docs/dedup-recovery-window-policy.md`

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

9. **Calibrate confirmation-window sufficiency.**
   The source policy now compares reviewed dedup `expiry` with the checked sum of
   lease, restart, reconnect, and operator-recovery bounds, and compares
   `max_entries` with an explicit per-partition distinct-ID bound. Supply and
   review those production inputs, bind them to configuration digests, execute
   the focused tests/verifier, and retain a clean-commit assessment before making
   a stronger duplicate-suppression statement.

10. **Design moving duplicate windows or keep fixed snapshots.**
    A moving per-partition cursor must retain bounded prior identity/digest state
    so copies split across cycles remain related. Fixed snapshots must not be
    presented as current-tail or complete-history evidence.

11. **Retain production operations.**
    Prove bundled mode, restart, TLS/auth/failover, reconnect, rebalance,
    retention, reconciliation, operator cleanup, and bounded replay/rescan
    repair.

## Recheck checkpoint — 2026-07-29

- Rechecked the canonical plan and current `main` after the two-partition
  fair-window harness and retained-capture tooling.
- Reconfirmed privacy-before-presentation, owner-scoped writes, Media ownership,
  fail-closed follower reads, no automatic mutation retry, and the rule that
  operational state never authorizes profile presentation.
- Reconfirmed deterministic same-ID colocation and the production-reachable
  fair/global comparison.
- Rechecked the existing external-Iggy dedup cases and confirmed that immediate,
  capacity, and expiry sequences do not establish a production recovery window.
- Added a pure fail-closed recovery-window policy with caller-reviewed additive
  time bounds and explicit per-partition capacity; disabled or insufficient
  configuration cannot report sufficient.
- Locked stable statuses/codes, focused source tests, an owner guide, a Profiles
  checkpoint, and a static source verifier.
- Kept active configuration readback, reviewed production calibration, runtime
  execution, canonical retained packets, moving-window state, bundled/TLS/auth,
  failover, and multi-replica claims open.
- Tests, Cargo commands, repository source verifiers, external/bundled Iggy,
  retained capture, and multi-replica scenarios were not run per maintainer
  instruction.

## Verification backlog

```text
cargo xtask module validate profiles
cargo xtask module test profiles
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
cargo test -p rustok-iggy dedup_recovery_window_policy -- --nocapture
cargo test -p rustok-iggy dlq_duplicate_external_scan -- --nocapture
RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS='host:8090' \
cargo test -p rustok-iggy --features iggy \
  --test dlq_duplicate_fair_window_external_scan -- \
  fair_window_scans_each_partition_and_differs_from_global_budget \
  --exact --nocapture --test-threads=1
cargo test -p rustok-iggy dlq_duplicate_alert_observer -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
node scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs
node scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs
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
13. Short dedup sequences do not prove production-window sufficiency; use a
    checked additive recovery horizon and an explicit per-partition capacity
    bound.
14. Production deterministic broker IDs remain colocated by the publisher's
    one-based modulo partition rule.
15. Retained evidence is no-clobber, commit-bound, and stale after any bound
    source change.
16. Moving duplicate windows require bounded cross-cycle identity state.
17. A sufficient recovery-window assessment covers only the supplied reviewed
    model and never authorizes Profiles or proves exactly-once.
18. Update Profiles and affected owner docs with every boundary change.
