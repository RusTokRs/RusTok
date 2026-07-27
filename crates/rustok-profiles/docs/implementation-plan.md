# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns public profile storage/translations, tags, handles,
visibility policy, owner reads, audience-bound presentation, summary batching,
GraphQL self-service, `profile.updated`, owner-local backfill, Media-backed image
presentation, and the first module-owned storefront profile surface.

Profiles is not an auth, customer, seller, staff, Social Graph, Index, or search
aggregate. Public GraphQL, Customer Admin enrichment, Blog/Forum author cards, and
storefront reads share privacy-before-presentation ordering and hide restricted or
unavailable rows as absent.

`followers_only` visibility resolves through authoritative Social Graph owner ports.
Profiles never reads relation tables and never authorizes from an event or Index
projection. The Social Graph → Index worker, its broker position observer, and lag
metrics are optional discovery/query operations only.

Media descriptors remain Media-owned. Profiles validates tenant, uploader, and MIME
constraints and exposes only Media-selected descriptors. Profiles does not know
storage keys, provider endpoints, or ingress construction.

The module-owned storefront mounts `/modules/profiles?handle=<handle>`, supports
SSR-first native and explicit GraphQL transports, renders approved avatar/banner
descriptors, and exposes authenticated follow/unfollow with unique idempotency keys,
optimistic revisions, and one read-only conflict refresh without automatic mutation
retry.

## Social Graph and Index status

- Durable Social Graph receipts bind a tenant-scoped normalized idempotency key to one
  complete command identity and share a transaction with relation mutation, optional
  event append, response snapshot, and completion.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact; no-op and
  receipt replay emit nothing, and event failure rolls owner state back.
- Historical replay is service/system-only, bounded, page-atomic, and remains the
  authoritative drift-repair source.
- The generic Index adapter maps active revisions to non-localized upserts, inactive
  revisions to tombstones, relation id to entity id, and revision to monotonic
  `source_version`.
- `SocialGraphIndexProjector` registers the tenant schema through Index-owned
  `PostgresSchemaRegistrationStore` before Index inbox apply.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- The persistent consumer retains one outstanding delivery and acknowledges only after
  schema and mutation results are durable.
- Runtime execution is default-off through
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED`; explicit enablement requires a worker
  host and `outbox_iggy`.
- Relay and consumer reuse the single `Arc<IggyTransport>` owned by `EventRuntime`.
- Projection failures use bounded retry. Before a durable result, permanent/exhausted
  failures may publish exact broker bytes to DLQ before source ack.
- DLQ publish and source ack are staged. Once Index or DLQ has a terminal result, only
  acknowledgement is retried in-process.
- Shared `StopHandle` controls shutdown and `SocialGraphIndexWorkerHandle` participates
  in `runtime_guardrails`, `/health/ready`, and aggregate guardrail metrics only when
  explicitly enabled.
- Shared Prometheus delivery telemetry covers received/terminal outcomes, retries,
  bounded failures, DLQ results, processing duration, starts/terminations, in-flight
  state, and last success.
- A separate read-only observer connects to the already-running configured Iggy endpoint
  and reads every `domain` partition plus the persistent group checkpoint. It never
  starts/stops a broker and never mutates offsets.
- Position metrics expose snapshot timestamp, partition count, completeness, and exact
  `total`/`max` offset lag only when every partition is coherent. Empty partitions
  contribute zero; missing/inconsistent checkpoints make the snapshot incomplete and
  clear lag gauges.
- Metrics use bounded labels and expose no tenant, event, relation, partition, offset,
  payload, ack-token, credential, or raw error-message values.
- Observer failures are operationally visible but do not stop projection, change worker
  readiness, or affect profile presentation.
- PostgreSQL concurrency, real-Iggy recovery/position evidence, the DLQ acknowledgement
  window, and multi-replica behavior remain pending.
- None of this moves privacy policy or relation authority out of Social Graph.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- The module has a module-owned Leptos storefront package with native/GraphQL
  transports, package i18n, fail-closed transport selection, optimistic recovery, and
  Media/Social Graph owner composition.
- FBA remains `not_started` until compiled/live transport, Media isolation, provider
  identity, public ingress, storage delivery, and retained runtime evidence exist.

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   **Status:** source-complete for current consumers. Privacy is evaluated before
   localized summary/tag loading and foreign modules do not read profile tables.
   **Revisit when:** retained production telemetry proves another bounded owner
   projection is necessary.

2. **Finish followers-only and downstream presentation policy.**
   **Status:** source-complete for owner privacy ports, public GraphQL lookups, author
   cards, storefront, Customer Admin enrichment, receipt-aware commands,
   transactional events, cleanup CLI, bounded replay, schema registration,
   result-first Index apply/ack, shared-connector lifecycle, readiness, delivery
   telemetry, and broker-backed complete lag observation.
   **Remaining:** prove bounded replay/rescan repair and retain compiled/runtime
   evidence for privacy, receipts, cleanup, event relay/replay, schema concurrency,
   broker restart/redelivery/DLQ/position observation, storefront, Customer Admin,
   Blog/Forum, and Media.
   **Done when:** every presentation consumer exposes one policy with retained
   evidence and no direct foreign-domain reads or projection-based authorization.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice, native/GraphQL transport
   selection, Media presentation, authenticated follow control, and read-only conflict
   recovery.
   **Directory/search decision:** directory/search must use Profiles-owned public
   profile records plus generic Index query contracts. Social Graph relation
   projection may support discovery/ranking inputs but must not replace audience-bound
   profile authorization. Runtime UI/query work waits for the Index query port and a
   Profiles-owned public-profile source schema.
   **Remaining:** execute SSR/hydrate/GraphQL route, auth, i18n, Media
   direct/proxy/fallback, mutation conflict, durable receipt, relation-event, and
   accessibility evidence.

4. **Keep profile backfill owner-local.**
   **Status:** source-complete; compiled/runtime verification pending. The CLI uses
   owner auth/tenant/customer reads and optional Outbox publication while preserving
   dry-run semantics and aggregate telemetry.

5. **Complete audit and operational evidence.**
   **Status:** source-complete for Profiles operations, Social Graph command telemetry,
   durable receipts, maintenance, events, replay, cleanup CLI, sealed conversion,
   persisted schema registration, durable terminal recognition, default-off shared
   connector lifecycle, retries, staged DLQ ordering, shutdown, readiness, bounded
   consumer metrics, and partition-qualified complete lag observation.
   **Remaining:** deployment retention approval, PostgreSQL concurrency/retention/
   replay/rollback, real broker observer/reconnect/TLS/rebalance and multi-replica
   evidence, durable DLQ identity decision, and retained operator packets.

## Recheck checkpoint — 2026-07-27

- Preserved receipts, cleanup, event, replay, CLI, migration, topology, and guard work
  from superseded draft PR #2237 in PR #2317.
- Rechecked privacy-before-presentation, bounded follower reads, owner-scoped writes,
  Media-owned descriptors, and no automatic mutation retry.
- Approved Index as the first relation-event consumer while rejecting foreign
  relation-table reads and projection-based authorization.
- Added Index-owned schema registration, staged persistent consumption, default-off
  lifecycle, strict `outbox_iggy` gating, one shared Iggy connector, shutdown, bounded
  retry, exact-byte DLQ-before-ack, and acknowledgement-only recovery.
- Added enabled-worker readiness and shared bounded Prometheus consumer telemetry.
- Added a read-only every-partition broker snapshot and completeness-gated total/max
  lag while retaining partition/offset values outside metric labels.
- Kept position observation independent from Profiles privacy, projection execution,
  and readiness.
- Tests, formatters, Cargo commands, source verifiers, PostgreSQL, real-broker, and
  multi-replica scenarios remain maintainer-run or pending.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `cargo test -p rustok-telemetry`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets`
- `cargo test -p rustok-index schema_registration --lib -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets`
- `cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture`
- `cargo test -p rustok-server runtime_guardrails --lib -- --nocapture`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
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
   idempotency keys, cursors, payloads, claims, roles, and channels.
10. Index/search projections may use sealed owner events, generic Index contracts,
    monotonic source versions, and bounded replay, but never authorize visibility.
11. Durable workers persist/recognize the owner result before ack; permitted DLQ
    publication precedes source ack.
12. Optional enabled workers participate in readiness and bounded telemetry; disabled
    workers do not degrade presentation availability.
13. Publish lag only from a complete partition-qualified broker snapshot; never use
    partition or offset as metric labels.
14. Position observation remains operational only and cannot authorize Profiles reads.
15. Update Profiles and affected owner docs with every boundary change.
