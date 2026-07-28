# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns public profile storage and translations, tags, handles,
visibility policy, owner reads, audience-bound presentation, summary batching,
GraphQL self-service, `profile.updated`, owner-local backfill, Media-backed image
presentation, and the module-owned profile storefront.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, search, or
broker aggregate. Public GraphQL, Customer Admin enrichment, Blog/Forum author cards,
and storefront reads share privacy-before-presentation ordering and hide restricted
or unavailable rows as absent.

`followers_only` resolves through authoritative Social Graph owner ports. Profiles
never reads relation tables and never authorizes from an event, Index projection,
decoded/raw DLQ receipt, neutral receipt aggregate, broker identifier, consumer
offset, lag metric, poison health signal, PostgreSQL evidence packet, physical Iggy
header observation, or any real-Iggy evidence harness.

Media descriptors remain Media-owned. Profiles validates tenant, uploader, and MIME
constraints and exposes only Media-selected descriptors. It does not know storage
keys, provider endpoints, or ingress construction.

The storefront mounts `/modules/profiles?handle=<handle>`, supports SSR-first native
and explicit GraphQL transports, renders approved avatar/banner descriptors, and
exposes authenticated follow/unfollow with unique idempotency keys, optimistic
revisions, and one read-only conflict refresh without automatic mutation retry.

## Delivered Social Graph and Index boundary

- Durable Social Graph command receipts bind a tenant-scoped normalized idempotency
  key to one complete command identity and share a transaction with relation mutation,
  optional event append, response snapshot, and completion.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact. No-op and
  receipt replay emit nothing; event failure rolls owner state back.
- Bounded relation-event replay is service/system-only, tenant-scoped, page-atomic,
  and driven by an exclusive UUID cursor. Social Graph persistence remains the
  authoritative drift-repair source.
- The generic Index adapter maps active revisions to non-localized upserts, inactive
  revisions to tombstones, relation id to entity id, and relation revision to
  monotonic revision/source-version semantics.
- `SocialGraphIndexProjector` registers the tenant schema through Index-owned
  persistence before durable inbox apply.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- Runtime execution is default-off through
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED`; explicit enablement requires a worker
  host and `outbox_iggy`.
- Relay, worker, and observers reuse the single `Arc<IggyTransport>` owned by
  `EventRuntime`; no worker starts another bundled broker.
- Decoded-event and raw-delivery receipt state is recognized before projection or a
  new terminal-policy choice. Existing durable work remains recoverable when creation
  of new DLQ decisions is later disabled.
- The explicit append-only platform tail contains both
  `m20260727_000004_create_index_dlq_receipts` and
  `m20260728_000001_create_consumer_poison_receipts` without rewriting its published
  prefix.

## Delivered raw contract decode-failure boundary

`PersistentContractConsumerGroup::receive_delivery` returns either a validated event
or `ConsumedContractDecodeFailure` without committing the cursor.

- stream/topic metadata is validated before deserialization;
- malformed or schema-invalid deliveries retain exact bytes, partition, offset, and
  opaque acknowledgement token;
- stable classifications are limited to `iggy.contract.decode_invalid` and
  `iggy.contract.schema_invalid`;
- a versioned RFC 9562 UUIDv8 derives only from stream, topic, partition, offset, and
  exact payload;
- retry count, classification, time, process identity, credentials, connector message
  identity, acknowledgement token, and randomness cannot drift the delivery identity;
- no tenant, actor, relation, or domain event identity is invented;
- compatibility `receive()` returns a bounded error and performs no implicit ack.

`rustok-iggy-connector` owns the neutral durable result:

- source coordinates and deterministic delivery UUID bind exact payload, including an
  empty payload;
- UUID/source/payload reuse with different facts fails closed;
- first error code and observed attempt are retained as one diagnostic pair;
- states are `reserved`, leased `publishing`, terminal `published`, and
  post-source-commit `acknowledged`;
- the store performs no broker publication, source commit, authorization, reclaim
  policy, retention, repair, or deletion choice;
- read-only inspection exposes fixed consumer-group counts only.

The Social Graph Index worker composes the approved order:

1. recognize or reserve the neutral receipt;
2. claim publication ownership;
3. publish `failure.to_dlq_entry(1)` with exact bytes;
4. persist `published`;
5. acknowledge the exact source cursor;
6. record `acknowledged` as best-effort bookkeeping.

A terminal receipt enters acknowledgement-only recovery. The raw path never invokes
Index projection and never affects Profiles privacy.

## Operational and evidence checkpoint

Source-complete operational visibility includes:

- bounded delivery/retry/failure/DLQ/lifecycle metrics;
- complete-partition consumer lag only when every broker checkpoint is coherent;
- count-only poison receipt gauges for `total`, `reserved`, `publishing`,
  `expired_publishing`, `published`, and `acknowledged`;
- stale count clearing and snapshot availability/timestamp;
- degraded health when the read-only observer task is missing or stopped, without
  making receipt counts a readiness or authorization decision.

PostgreSQL evidence tooling is source-complete:

- unique schema per scenario and independent one-connection pools;
- concurrent claim ownership and `Busy` loser;
- lease reclaim with old-publisher `ClaimLost` fencing;
- collision rollback and unchanged original row;
- atomic first-diagnostic retention;
- terminal aggregate consistency;
- clean-commit runner, bounded PostgreSQL/toolchain metadata, source/output SHA-256,
  atomic packet writing, and strict current-source verification.

The canonical PostgreSQL execution JSON remains absent until a maintainer executes the
runner successfully.

External real-Iggy cursor evidence is source-complete and runtime-pending:

- one unique disposable/operator-cleaned stream and one partition;
- two non-empty malformed payloads injected only through fixture-level
  `ExternalConnector::publish`;
- production typed receive retains exact first bytes, offset, ack token, stable code,
  and deterministic UUID without source ack;
- production `IggyTransport::move_to_dlq` publishes the exact first bytes;
- first transport shutdown without source ack followed by a new transport and the same
  group must redeliver the same offset, bytes, and UUID;
- explicit source ack must then expose the second malformed payload at a greater
  offset;
- an independent real DLQ cursor verifies both payloads byte-for-byte.

A separate external physical-header harness is also source-complete and runtime-pending:

- one production `ConsumedContractDecodeFailure` creates one `DlqEntry`;
- one production `IggyTransport::move_to_dlq` call publishes the entry;
- a probe-only SDK consumer opened before publication reads exactly one physical DLQ
  message;
- physical `message.header.id` must equal the connector UUID as `u128`;
- physical partition must equal `(uuid_as_u128 mod 3) + 1` and remain one-based;
- physical payload must remain exact;
- only the probe's physical header offset is committed.

The SDK probe cannot publish, acknowledge a source cursor, modify a receipt, delete a
stream, or change deduplication. The lifecycle harness does not prove the physical
header; the header harness does not prove source cursor lifecycle. Neither proves
PostgreSQL ordering, deduplication, bundled mode, TLS/auth, multi-replica behavior, or
physical exactly-once.

## FFA/FBA boundary

- FFA status: `in_progress`.
- FBA status: `not_started`.
- Structural shape: `core_transport_ui`.
- The Leptos storefront has native/GraphQL transports, package i18n, fail-closed
  transport selection, Media/Social Graph owner composition, and optimistic conflict
  recovery.
- FBA remains blocked on compiled/live transport, Media isolation, provider identity,
  public ingress/storage delivery, and retained runtime evidence.

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   **Status:** source-complete for current consumers. Privacy is evaluated before
   localized summary/tag loading; foreign modules do not read profile tables.

2. **Finish followers-only and downstream presentation policy.**
   **Status:** source-complete for owner privacy ports, public GraphQL, author cards,
   storefront, Customer Admin enrichment, command receipts, transactional events,
   cleanup, replay, schema registration, result-first Index apply/ack, durable decoded
   and raw DLQ recovery, shared-transport lifecycle, metrics, lag, and count-only poison
   health.
   **Remaining:** compiled/runtime evidence for privacy, schema concurrency, replay
   repair, storefront, Customer Admin, Blog/Forum, and Media.

3. **Publish and retain storefront evidence.**
   Execute SSR/hydrate/GraphQL route, auth, i18n, Media direct/proxy/fallback, follow
   conflict, durable receipt, relation-event, and accessibility scenarios.

4. **Keep profile backfill owner-local.**
   The CLI remains source-complete and uses owner auth/tenant/customer reads plus
   optional Outbox publication while preserving dry-run semantics and aggregate
   telemetry. Compiled/runtime proof remains open.

5. **Execute retained PostgreSQL poison evidence.**
   Run the clean-commit capture runner, review the bounded packet, commit the canonical
   JSON, and repeat whenever a bound source hash changes.

6. **Execute external cursor and header evidence.**
   Run both harnesses on a disposable broker and retain separate packets for source
   reconnect/redelivery and physical UUID/header/partition observation.

7. **Compose receipt-plus-broker evidence.**
   Prove reserve/claim -> exact publish -> durable `published` -> source ack ->
   best-effort `acknowledged`, process loss, acknowledgement-only recovery, and
   multi-replica claim ownership on PostgreSQL plus real Iggy.

8. **Prove duplicate and confirmation behavior separately.**
   Exercise publisher reconnect and the same deterministic UUID with deduplication
   disabled, enabled, capacity-evicted, and expired. Choose an enforced dedup window or
   stronger outbox/transaction mechanism before any stronger duplicate guarantee.

9. **Retain production operations.**
   Prove bundled mode, restart, TLS/auth/failover, position reconnect, rebalance,
   retention/reconciliation, operator cleanup, and bounded replay/rescan repair.

## Recheck checkpoint — 2026-07-28

- Rechecked the canonical plan and current `main` using short merge/new-branch cycles
  because multiple agents change the repository in parallel.
- Reconfirmed privacy-before-presentation, bounded follower reads, owner-scoped writes,
  Media-owned descriptors, no automatic mutation retry, and the rule that
  Index/broker/receipt/metric/evidence state never authorizes profile presentation.
- Preserved command receipts, transactional sealed events, bounded replay, Index schema
  registration, result-first consumption, decoded/raw DLQ receipts, deterministic
  delivery identity, shared transport, readiness, metrics, and position observation.
- Closed append-only migration-tail reconciliation.
- Added count-only receipt inspection/metrics, stale clearing, degraded observer-task
  health, and the operator runbook.
- Added isolated PostgreSQL scenarios and clean-commit retained evidence tooling; the
  execution packet remains pending.
- Added a versioned external-Iggy cursor contract, opt-in real cursor/DLQ harness,
  no-ack transport reconnect/redelivery, exact-byte DLQ checks, explicit cursor
  advancement, and a static verifier.
- Added a separate physical-header contract and harness for one production publication,
  exact UUID/u128 header mapping, one-based partition routing, exact payload, probe-only
  SDK access, and probe offset commit.
- Kept duplicate suppression, database ordering, bundled/TLS/auth, and multi-replica
  claims explicitly open.
- Tests, Cargo commands, formatters, source verifiers, PostgreSQL, external/bundled Iggy,
  and multi-replica scenarios were not run, per maintainer instruction.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `cargo test -p rustok-iggy contract_decode_failure --lib -- --nocapture`
- `RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='host:8090' cargo test -p rustok-iggy --features iggy --test contract_poison_external_iggy -- --nocapture --test-threads=1`
- `RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS='host:8090' cargo test -p rustok-iggy --features iggy --test contract_poison_external_iggy_header -- --nocapture --test-threads=1`
- `node scripts/verify/verify-iggy-contract-decode-failure.mjs`
- `node scripts/verify/verify-iggy-contract-poison-external-evidence.mjs`
- `node scripts/verify/verify-iggy-contract-poison-external-header-evidence.mjs`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy-connector --features iggy,migrations --all-targets`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_receipt -- --nocapture`
- `cargo test -p rustok-iggy-connector --features migrations consumer_poison_inspection -- --nocapture`
- `RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL='postgresql://…' cargo test -p rustok-iggy-connector --features migrations --test consumer_poison_receipt_postgres -- --nocapture`
- `RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL='postgresql://…' node scripts/evidence/capture-iggy-consumer-poison-postgres.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-receipts.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-inspection.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-postgres-evidence.mjs`
- `node scripts/verify/verify-iggy-consumer-poison-retained-evidence.mjs`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-social-graph-index-dlq-receipts.mjs`
- `node scripts/verify/verify-social-graph-index-poison-observer.mjs`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- `node scripts/verify/verify-social-graph-command-receipts.mjs`
- `node scripts/verify/verify-social-graph-receipt-cleanup.mjs`
- `node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs`
- `node scripts/verify/verify-social-graph-relation-outbox.mjs`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `node scripts/verify/verify-profiles-storefront-boundary.mjs`
- `rustok-cli profiles backfill --tenant-id <uuid> --dry-run`
- `rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run`

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain owner-internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. Profile media resolves through Media owner ports only.
7. Module UI stays package-owned with explicit transports and package i18n.
8. Follow controls use owner ports, unique idempotency, optimistic revision, and no automatic retry.
9. Operational telemetry excludes presentation copy, identities, idempotency keys,
   cursors, payloads, broker IDs, claims, roles, credentials, and provider details.
10. Index/search projections may use sealed owner events, generic Index contracts,
    monotonic source versions, and bounded replay, but never authorize visibility.
11. Durable workers persist/recognize the owner result before ack; exact-byte DLQ
    publication and terminal receipt persistence precede source ack.
12. Deterministic IDs bind immutable source identity and exact payload but never
    authorize presentation or imply exactly-once without retained broker evidence.
13. Publish lag only from a complete partition-qualified broker snapshot; partition and
    offset are values, never metric labels.
14. Undecodable bytes must not invent tenant or domain event identity; acknowledge only
    after an approved terminal result exists.
15. Existing durable poison choices remain recoverable across later policy disablement;
    new undecodable deliveries remain uncommitted without enabled terminal policy.
16. Count-only health and PostgreSQL/Iggy evidence never authorize, acknowledge, reclaim,
    repair, retain, or delete production delivery state.
17. Retained packets require a clean commit, omit credentials/delivery-level facts, and
    become stale when a bound source SHA-256 changes.
18. Real-Iggy fixture injection may create malformed bytes, but production receive, DLQ,
    and acknowledgement must use reviewed transport APIs.
19. Direct SDK probes may only observe explicitly scoped broker facts and commit their
    own probe offsets; they must not publish, choose policy, or mutate receipts.
20. Do not claim deduplication, database ordering, bundled, TLS/auth, multi-replica, or
    exactly-once proof from one-message physical-header evidence.
21. Update Profiles and affected owner docs with every boundary change.
