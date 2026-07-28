# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns profile storage, translations, tags, handles, visibility policy,
owner reads, audience-bound presentation, summary batching, self-service GraphQL,
`profile.updated`, owner-local backfill, Media-backed image presentation, and the
module-owned storefront.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, search, broker,
receipt, or telemetry aggregate. Public GraphQL, Customer Admin enrichment,
Blog/Forum author cards, and storefront reads must evaluate privacy before localized
presentation and must hide restricted or unavailable rows as absent.

`followers_only` resolves through authoritative bounded Social Graph owner ports.
Profiles never reads relation tables and never authorizes from events, Index state,
DLQ receipts, broker identifiers, offsets, lag, poison health, PostgreSQL/Iggy evidence,
deduplication observations, or duplicate-scan modes.

Media descriptors remain Media-owned. Profiles validates tenant, uploader, and MIME
constraints and exposes only Media-selected descriptors; storage keys, provider
endpoints, and ingress construction remain outside Profiles.

The storefront mounts `/modules/profiles?handle=<handle>`, supports SSR-first native
and explicit GraphQL transports, package i18n, approved avatar/banner descriptors,
and authenticated follow/unfollow with unique idempotency keys, optimistic revisions,
and one read-only conflict refresh without automatic mutation retry.

## Delivered boundaries

### Social Graph and Index

- Durable command receipts bind a tenant-scoped normalized idempotency key to one
  complete command identity and share a transaction with relation mutation, optional
  event append, response snapshot, and completion.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact. No-op and
  receipt replay emit nothing; event failure rolls owner state back.
- Bounded replay is service/system-only, tenant-scoped, page-atomic, and driven by an
  exclusive UUID cursor. Social Graph persistence remains the repair source.
- Index registration and projection use Index-owned contracts and persistence;
  `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- Runtime consumers are default-off, require a worker host plus `outbox_iggy`, and
  reuse the single `Arc<IggyTransport>` owned by `EventRuntime`.
- Index/search projections can improve discovery but never authorize profile visibility.

### Raw decode failures and poison receipts

- `PersistentContractConsumerGroup::receive_delivery` returns either a validated event
  or exact-byte `ConsumedContractDecodeFailure` without committing the source cursor.
- Stable failure classes are bounded; deterministic RFC 9562 UUIDv8 identity derives
  only from source coordinates and exact payload. No tenant/domain identity is invented.
- Neutral durable receipts recognize or reserve work before publication, fence claims,
  retain first diagnostics, and separate `published` from post-source-commit
  `acknowledged` bookkeeping.
- The approved order remains receipt recognition/claim, exact-byte DLQ publication,
  durable `published`, exact source acknowledgement, then best-effort `acknowledged`.
- Existing terminal receipts remain recoverable after later policy disablement.
- Count-only receipt inspection, stale clearing, metrics, and degraded observer-task
  health remain operational signals only and never become readiness or authorization.

### Evidence tooling

- Isolated PostgreSQL concurrency, reclaim fencing, collision rollback, diagnostic
  retention, aggregate consistency, clean-commit capture, source hashing, and strict
  retained-packet verification are source-complete; canonical execution is pending.
- External Iggy reconnect/redelivery, exact-byte DLQ, physical UUID/u128 header,
  one-based partition, and four dedup behavior scenarios are source-complete and
  runtime-pending.
- Retained packets must omit credentials and delivery-level facts, bind reviewed source
  hashes/configuration digests, and become stale when bound sources change.

## Physical DLQ duplicate inspection

Two bounded policies are source-complete:

- `global_budget`: one ordered partition allowlist and one shared cap. A busy early
  partition may prevent later partitions from being polled.
- `fair_window`: one equal cap for every selected partition, checked total
  `partition_count * per_partition_messages <= 10000`, and all observations combined
  before duplicate classification.

The event-delivery server observer keeps `global_budget` as the compatibility default
and accepts explicit `fair_window` only for `outbox_iggy`:

```text
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=global_budget
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE=fair_window
RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES=<positive cap>
```

`memory` and `outbox_local` remain intentional not-applicable modes and do not resolve
Iggy. Both policies use explicit offsets and `auto_commit=false`. Every observer cycle
reuses the configured start offset; neither policy owns moving cursors, stored progress,
cross-cycle duplicate state, current-tail coverage, or complete-history semantics.

A successful fair-window scan attempts every configured partition under the same cap
and preserves cross-partition duplicate/conflicting-payload groups in the aggregate.
External multi-partition execution, retained server evidence, and moving-window design
remain pending. No duplicate observation or scan state may become a Profiles input.

Detailed checkpoints:

- `crates/rustok-profiles/docs/poison-duplicate-external-scan-checkpoint.md`
- `crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md`
- `crates/rustok-iggy/docs/dlq-duplicate-external-scan.md`
- `crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md`

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   Source-complete for current consumers; retain privacy-before-presentation ordering.

2. **Finish compiled/runtime presentation evidence.**
   Execute storefront, Customer Admin, Blog/Forum, Media, privacy, replay, and schema
   concurrency scenarios without introducing foreign-table reads.

3. **Publish and retain storefront evidence.**
   Cover SSR/hydration/GraphQL, auth, i18n, Media direct/proxy/fallback, follow conflict,
   durable receipts/events, and accessibility.

4. **Keep profile backfill owner-local.**
   Preserve dry-run semantics and owner auth/tenant/customer reads; retain runtime proof.

5. **Execute retained PostgreSQL poison evidence.**
   Run the clean-commit capture, review the bounded packet, commit canonical JSON, and
   repeat whenever a bound source hash changes.

6. **Execute external Iggy evidence.**
   Run reconnect/redelivery, physical header/partition, dedup behavior, compatibility
   global duplicate scan, and multi-partition fair-window harnesses on reviewed
   disposable brokers; retain separate privacy-safe packets and config digests.

7. **Compose receipt-plus-broker recovery evidence.**
   Prove claim -> exact publish -> durable `published` -> source ack -> best-effort
   `acknowledged`, process loss, acknowledgement-only recovery, and multi-replica
   ownership on PostgreSQL plus real Iggy.

8. **Prove confirmation-window sufficiency.**
   Compare dedup `max_entries`/`expiry` against maximum lease, restart, reconnect, and
   operator recovery horizons before making any stronger duplicate guarantee.

9. **Design moving duplicate windows or keep fixed snapshots.**
   A moving per-partition cursor must retain bounded prior identity/digest state so
   copies split across cycles remain related. Fixed snapshots must not be presented as
   current-tail or complete-history evidence.

10. **Retain production operations.**
    Prove bundled mode, restart, TLS/auth/failover, reconnect, rebalance, retention,
    reconciliation, operator cleanup, and bounded replay/rescan repair.

## Recheck checkpoint — 2026-07-28

- Rechecked the canonical plan and current `main` after the previous short PR cycle.
- Reconfirmed privacy-before-presentation, owner-scoped writes, Media ownership,
  fail-closed follower reads, no automatic mutation retry, and the rule that operational
  state never authorizes profile presentation.
- Rechecked and merged the bounded fair per-partition scanner while preserving the
  compatibility global-budget request.
- Added explicit server scan-mode selection: `global_budget` remains default;
  `fair_window` requires an explicit per-partition cap and remains fixed-offset.
- Corrected the stale external-scan source contract verifier and documentation that had
  not been actualized with the fair-window API.
- Kept runtime execution, retained evidence, moving-window/cross-cycle state,
  production recovery-window sufficiency, bundled/TLS/auth, and multi-replica claims open.
- Tests, Cargo commands, formatters, repository source verifiers, PostgreSQL,
  external/bundled Iggy, and multi-replica scenarios were not run per maintainer instruction.

## Verification backlog

```text
cargo xtask module validate profiles
cargo xtask module test profiles
cargo check -p rustok-profiles-storefront --all-targets
cargo test -p rustok-profiles-storefront
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
cargo test -p rustok-iggy dlq_duplicate_external_scan -- --nocapture
cargo test -p rustok-iggy dlq_duplicate_alert_observer -- --nocapture
cargo test -p rustok-server event_dlq_duplicate_alert_observer -- --nocapture
node scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs
node scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs
node scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
```

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. Profile media resolves through Media owner ports only.
7. Follow controls use owner ports, unique idempotency, optimistic revision, and no
   automatic retry.
8. Index/broker/receipt/metric/evidence state never authorizes visibility.
9. Durable workers recognize/persist the owner result before source acknowledgement.
10. Exact-byte DLQ publication and terminal receipt persistence precede source ack.
11. Deterministic IDs bind immutable source identity and exact payload but do not imply
    exactly-once without retained broker evidence.
12. Operational telemetry excludes identities, payloads, broker coordinates, claims,
    credentials, and provider details.
13. Short dedup or scan sequences do not prove production-window sufficiency.
14. Moving duplicate windows require bounded cross-cycle identity state.
15. Update Profiles and affected owner docs with every boundary change.
