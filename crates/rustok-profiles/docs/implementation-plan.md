# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns public profile storage/translations, tags, handles,
visibility policy, owner reads, audience-bound presentation, summary batching,
GraphQL self-service, `profile.updated`, owner-local backfill, Media-backed image
presentation, and the first module-owned storefront profile surface.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, search, or
broker aggregate. Public GraphQL, Customer Admin enrichment, Blog/Forum author
cards, and storefront reads share privacy-before-presentation ordering and hide
restricted or unavailable rows as absent.

`followers_only` visibility resolves through authoritative Social Graph owner
ports. Profiles never reads relation tables and never authorizes from an event,
Index projection, decoded/raw DLQ receipt, neutral receipt aggregate, broker
identifier, consumer offset, lag metric, poison-receipt health signal, or retained
receipt evidence packet.

Media descriptors remain Media-owned. Profiles validates tenant, uploader, and
MIME constraints and exposes only Media-selected descriptors. Profiles does not
know storage keys, provider endpoints, or ingress construction.

The module-owned storefront mounts `/modules/profiles?handle=<handle>`, supports
SSR-first native and explicit GraphQL transports, renders approved avatar/banner
descriptors, and exposes authenticated follow/unfollow with unique idempotency
keys, optimistic revisions, and one read-only conflict refresh without automatic
mutation retry.

## Social Graph, Index, and broker status

- Durable Social Graph command receipts bind a tenant-scoped normalized
  idempotency key to one complete command identity and share a transaction with
  relation mutation, optional event append, response snapshot, and completion.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact; no-op
  and receipt replay emit nothing, and event failure rolls owner state back.
- Historical replay is service/system-only, bounded, page-atomic, and remains the
  authoritative drift-repair source.
- The generic Index adapter maps active revisions to non-localized upserts,
  inactive revisions to tombstones, relation id to entity id, and revision to
  monotonic `source_version`.
- `SocialGraphIndexProjector` registers the tenant schema through Index-owned
  persistence before Index inbox apply.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- The persistent consumer retains one outstanding delivery and acknowledges only
  after schema/mutation or durable decoded/raw DLQ results exist.
- Runtime execution is default-off through
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED`; explicit enablement requires a
  worker host and `outbox_iggy`.
- Relay and consumer reuse the single `Arc<IggyTransport>` owned by `EventRuntime`.
- Projection, decoded/raw DLQ publication, neutral receipt, and acknowledgement
  failures use bounded retry.
- Migration `m20260727_000004_create_index_dlq_receipts` stores immutable poison
  source coordinates and exact broker bytes under trusted tenant/consumer/event
  identity.
- Decoded receipt states reserve and lease publication, then persist `published`
  before source acknowledgement. Existing `published`/`acknowledged` receipts are
  recognized before projection and enter acknowledgement-only recovery.
- A versioned length-framed SHA-256 construction derives one UUIDv8 from the
  immutable trusted receipt identity and exact payload. Retry count, time,
  publisher identity, and random values are excluded.
- A read-only observer reads every `domain` partition plus the persistent group
  checkpoint. Complete total/max lag is published only when every partition is
  coherent; missing/inconsistent checkpoints clear lag gauges.
- A separate count-only poison observer publishes fixed receipt-state aggregates,
  snapshot availability, and snapshot time. Unavailable inspection clears stale
  values; a missing/stopped observer is degraded health and never blocks projection.
- The explicit append-only migration tail contains both receipt migrations, so the
  previously published migration prefix is preserved.
- An opt-in PostgreSQL receipt harness defines isolated-schema evidence for
  concurrent ownership, lease reclaim/fencing, collision rollback,
  first-diagnostic retention, empty payloads, terminal recognition, and aggregate
  inspection.
- A retained execution contract locks the exact Cargo commands, required cases,
  bounded metadata, source hashes, and canonical packet path. Its clean-commit
  runner and strict verifier are source-complete; the execution JSON is intentionally
  absent until a maintainer runs PostgreSQL successfully.
- Main also contains generic Index partition snapshot/query/mutation evidence.
  That strengthens the shared Index substrate but does not validate this Social
  Graph consumer, its schema registration, or Profiles presentation policy.

## Raw contract decode-failure checkpoint

The previous consumer path deserialized before returning a delivery. Malformed
JSON/MessagePack or a registered-schema failure therefore surfaced only as a
receive error: exact bytes and connector coordinates were unavailable to the
owner worker, and the offset remained uncommitted.

`rustok-iggy` now defines the source contract:

- `PersistentContractConsumerGroup::receive_delivery()` returns either a
  validated event or `ConsumedContractDecodeFailure`;
- stream/topic metadata is checked before decoding;
- decode/schema failures retain exact bytes, partition, offset, and opaque ack
  token without inventing tenant or domain event identity;
- stable codes are limited to `iggy.contract.decode_invalid` and
  `iggy.contract.schema_invalid`;
- a versioned UUIDv8 is derived from stream/topic/partition/offset/exact payload;
  error kind, retry count, time, process identity, connector message identity,
  acknowledgement token, credentials, and randomness are excluded;
- acknowledgement remains a separate explicit call after an approved terminal
  poison result exists;
- compatibility `receive()` still returns a bounded error and performs no
  implicit acknowledgement.

`rustok-iggy-connector` owns the neutral durable result boundary:

- migration `m20260728_000001_create_consumer_poison_receipts` is registered
  through the existing connector migration hook;
- source coordinates `(consumer_group, stream, topic, partition, offset)` are
  unique and bind one deterministic connector delivery UUID plus exact bytes;
- empty payload is valid exact broker input;
- reuse with another UUID, coordinate set, or exact payload fails closed as an
  identity conflict;
- the first stable error code and observed delivery attempt are retained as
  diagnostics, while later decoder classification or retry-count drift does not
  redefine the connector delivery identity;
- states are `reserved`, leased `publishing`, `published`, and `acknowledged`;
- expired publication leases may be reclaimed, while terminal states are
  recognized idempotently;
- the store performs no publish, DLQ routing, source commit, or authorization;
- read-only inspection exposes only fixed consumer-group counts and performs no
  receipt transition, repair, retention, or deletion action.

The Social Graph Index worker is wired to the typed result:

- `SocialGraphIndexConsumer::receive_delivery` exposes events and decode failures
  without committing either;
- the worker checks for an existing neutral receipt before applying current DLQ
  policy, so a previously selected result continues recovery even when new DLQ
  decisions are later disabled;
- a new undecodable delivery remains uncommitted while DLQ is disabled;
- a claimed receipt publishes `failure.to_dlq_entry(1)` with exact bytes and the
  deterministic connector UUID, then persists `published` before source ack;
- `published`/`acknowledged` redelivery skips publication and enters ack-only
  recovery;
- source ack precedes best-effort `mark_acknowledged` bookkeeping;
- the raw path never invokes Index projection and never creates tenant/event facts.

Migration-order reconciliation is complete. The remaining work is execution and
retained proof: compile the current mainline, run the clean-commit PostgreSQL
capture runner, review and commit its canonical evidence JSON, run real-Iggy
publication/ack/restart scenarios, and retain multi-replica/operator evidence
without weakening privacy or exactly-once language.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- The module has a module-owned Leptos storefront package with native/GraphQL
  transports, package i18n, fail-closed transport selection, optimistic recovery,
  and Media/Social Graph owner composition.
- FBA remains `not_started` until compiled/live transport, Media isolation,
  provider identity, public ingress, storage delivery, and retained runtime
  evidence exist.

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   **Status:** source-complete for current consumers. Privacy is evaluated before
   localized summary/tag loading and foreign modules do not read profile tables.
   **Revisit when:** retained production telemetry proves another bounded owner
   projection is necessary.

2. **Finish followers-only and downstream presentation policy.**
   **Status:** source-complete for owner privacy ports, public GraphQL lookups,
   author cards, storefront, Customer Admin enrichment, receipt-aware commands,
   transactional events, cleanup CLI, bounded replay, schema registration,
   result-first Index apply/ack, durable decoded-event and raw-delivery DLQ receipt
   recovery, deterministic broker identity, shared-transport lifecycle, readiness,
   delivery telemetry, broker-backed complete lag observation, and count-only
   neutral receipt health.
   **Remaining:** prove bounded replay/rescan repair and retain compiled/runtime
   evidence for privacy, receipts, cleanup, event relay/replay, schema concurrency,
   broker restart/redelivery/DLQ receipt/header/dedup/position observation,
   storefront, Customer Admin, Blog/Forum, and Media.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice, native/GraphQL transport
   selection, Media presentation, authenticated follow control, and read-only
   conflict recovery.
   **Directory/search decision:** directory/search must use Profiles-owned public
   profile records plus generic Index query contracts. Social Graph relation
   projection may support discovery/ranking inputs but must not replace
   audience-bound profile authorization.
   **Remaining:** execute SSR/hydrate/GraphQL route, auth, i18n, Media
   direct/proxy/fallback, mutation conflict, durable receipt, relation-event, and
   accessibility evidence.

4. **Keep profile backfill owner-local.**
   **Status:** source-complete; compiled/runtime verification pending. The CLI uses
   owner auth/tenant/customer reads and optional Outbox publication while
   preserving dry-run semantics and aggregate telemetry.

5. **Complete audit and operational evidence.**
   **Status:** source-complete for Profiles operations, Social Graph command
   telemetry, durable command/decoded-event/raw-delivery DLQ receipts,
   deterministic DLQ broker identity, maintenance, events, replay, cleanup CLI,
   persisted schema registration, durable terminal recognition, default-off shared
   transport lifecycle, retries, shutdown, readiness, bounded metrics, complete
   lag, count-only poison health, PostgreSQL scenarios, and retained capture tooling.
   **Remaining:** execute and commit PostgreSQL evidence, prove retention/replay/
   rollback, approve deployment retention, prove real broker observer/reconnect/
   TLS/rebalance, deterministic header and dedup disabled/enabled/expiry/capacity
   behavior, confirmation policy, multi-replica behavior, and retained operator
   packets.

6. **Handle undecodable sealed contract deliveries without invented ownership.**
   **Status:** source-complete for typed receive, immutable connector identity,
   neutral durable receipt, exact-byte DLQ publication, durable published-before-ack,
   existing-result recovery, best-effort post-ack bookkeeping, append-only migration
   placement, count-only health, opt-in PostgreSQL scenarios, and a fail-closed
   retained execution contract/runner/verifier.
   **Next:** execute the clean-commit PostgreSQL runner, review and commit the
   canonical packet, then execute real-Iggy failure/restart/dedup/multi-replica
   scenarios.
   **Done when:** malformed bytes are retained, classified, durably terminalized,
   published/recovered, and acknowledged without fabricated tenant/event identity,
   implicit commits, duplicate authorization effects, or exactly-once claims, and
   the behavior is retained by compiled/database/broker evidence.

## Recheck checkpoint — 2026-07-28

- Rechecked the canonical Profiles plan, superseded PR #2237, draft PR #2317,
  replacement PR #2338, and current `main` while using short merge/new-branch cycles
  to reduce conflicts with parallel agents.
- Preserved receipts, cleanup, sealed events, replay, schema registration,
  persistent worker, durable decoded-event DLQ receipts, deterministic broker
  identity, readiness, telemetry, and position observation from PR #2317.
- Reconfirmed privacy-before-presentation, bounded follower reads, owner-scoped
  writes, Media-owned descriptors, no automatic mutation retry, and the rule that
  Index/broker/receipt/metric/evidence state never authorizes profile presentation.
- Added the typed exact-byte decode-failure contract with explicit acknowledgement.
- Added connector-owned neutral receipt DDL/store with private immutable source
  identity, empty exact-byte support, collision detection, first-diagnostic
  retention, leased publication, terminal recognition, and bounded stable errors.
- Wired the Social Graph Index owner adapter and server worker to typed raw delivery.
- Added exact-byte reserve/publish/mark-published/source-ack ordering and ack-only
  redelivery recovery without tenant/event synthesis or Index projection.
- Preserved an existing durable raw result across later DLQ policy disablement while
  leaving a new undecodable delivery uncommitted when no terminal policy is enabled.
- Enabled connector migration/storage API explicitly in the server dependency rather
  than relying on transitive feature unification.
- Registered the connector migration in the truthful `mode: none` backfill ledger.
- Appended both receipt migrations to the explicit platform release-order tail
  without rewriting its published prefix.
- Added count-only receipt inspection, Prometheus metrics, stale-snapshot clearing,
  bounded failure logging, degraded observer-task health, and an operator runbook.
- Added an opt-in isolated-schema PostgreSQL harness for claim ownership, lease
  reclaim/fencing, collision rollback, atomic first-diagnostic retention, terminal
  recognition, and aggregate consistency.
- Added a PostgreSQL metadata test, clean-commit capture runner, atomic canonical
  packet writer, source/output SHA-256 binding, and strict retained-evidence verifier.
- The canonical execution JSON remains absent until a maintainer runs PostgreSQL.
- Tests, Cargo commands, formatters, source verifiers, PostgreSQL, real-broker, and
  multi-replica scenarios were not run, per maintainer instruction.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `cargo test -p rustok-iggy contract_decode_failure --lib -- --nocapture`
- `node scripts/verify/verify-iggy-contract-decode-failure.mjs`
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
9. Operational telemetry excludes presentation copy, email, provider details, identities,
   idempotency keys, cursors, payloads, broker IDs, claims, roles, and channels.
10. Index/search projections may use sealed owner events, generic Index contracts,
    monotonic source versions, and bounded replay, but never authorize visibility.
11. Durable workers persist/recognize the owner result before ack; exact-byte broker
    DLQ publication and terminal receipt persistence precede source ack.
12. Deterministic IDs bind immutable source identity and exact payload but never
    authorize presentation or imply exactly-once without retained broker evidence.
13. Optional enabled workers participate in readiness and bounded telemetry; disabled
    workers do not degrade presentation availability.
14. Publish lag only from a complete partition-qualified broker snapshot; never use
    partition or offset as metric labels.
15. Undecodable bytes must not invent tenant or domain event identity. Use a neutral
    connector poison contract and acknowledge only after its terminal result exists.
16. Existing durable poison choices remain recoverable across later policy disablement;
    new undecodable deliveries remain uncommitted without an enabled terminal policy.
17. Count-only receipt health and retained PostgreSQL evidence never authorize,
    acknowledge, reclaim, repair, retain, or delete production delivery state.
18. Retained packets must be generated from a clean commit, omit credentials and
    delivery-level facts, and become stale when any bound source SHA-256 changes.
19. Update Profiles and affected owner docs with every boundary change.
