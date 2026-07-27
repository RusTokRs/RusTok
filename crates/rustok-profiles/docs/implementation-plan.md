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
projection. The delivered Social Graph → Index worker is optional discovery/query
infrastructure only.

Media descriptors remain Media-owned. Profiles revalidates tenant, uploader, and
MIME constraints and exposes only Media-selected descriptors. Profiles does not know
storage keys, provider endpoints, or ingress construction.

The module-owned storefront mounts `/modules/profiles?handle=<handle>`, supports
SSR-first native and explicit GraphQL transports, renders approved avatar/banner
descriptors, and exposes authenticated follow/unfollow with unique idempotency keys,
optimistic revisions, and one read-only conflict refresh without automatic mutation
retry.

## Social Graph and Index status

- Durable Social Graph receipts bind a tenant-scoped normalized idempotency key to
  one complete command identity.
- Receipt reservation, relation mutation, optional event append, response snapshot,
  completion, and commit share one transaction.
- `social_graph.relation.state_changed` is a sealed persisted-revision fact; no-op and
  receipt replay emit nothing, and event failure rolls owner state back.
- Historical replay is service/system-only, tenant/cursor bounded, dry-run capable,
  page-atomic, and remains the authoritative drift-repair source.
- The approved generic Index adapter maps active revisions to non-localized upserts,
  inactive revisions to tombstones, relation id to entity id, and relation revision
  to monotonic `source_version`.
- `SocialGraphIndexProjector` registers the tenant schema through Index-owned
  `PostgresSchemaRegistrationStore` before applying through the Index inbox.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable results.
- The persistent consumer retains one outstanding broker delivery and acknowledges
  only after schema and mutation results are durable.
- The server lifecycle is source-complete and default-off through
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED`.
- Explicit enablement requires a worker host and `outbox_iggy`.
- Relay and consumer reuse the one `Arc<IggyTransport>` owned by `EventRuntime`; the
  worker never starts or stops a second bundled broker process.
- Projection failures use bounded retry. Before a durable result, permanent/exhausted
  failures may move exact original broker bytes to DLQ before source ack when the
  reviewed DLQ setting is enabled.
- After a durable result, only acknowledgement is retried; ack failure is never DLQed.
- Shared `StopHandle` controls shutdown and `SocialGraphIndexWorkerHandle` exposes
  task readiness state.
- Explicitly enabled missing/stopped/invalid worker state is critical in
  `runtime_guardrails`, reaches `/health/ready`, and changes aggregate guardrail
  metrics under the existing observe/enforce policy. Disabled execution is healthy.
- Dedicated consumer throughput/retry/DLQ/lag metrics, PostgreSQL concurrency,
  real-Iggy restart/redelivery, DLQ failure, and multi-replica evidence remain pending.
- None of this moves privacy policy or relation authority out of Social Graph.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- The module has a module-owned Leptos storefront package with native/GraphQL
  transports, package i18n, explicit fail-closed transport selection, optimistic
  recovery, and Media/Social Graph owner composition.
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
   transactional events, cleanup CLI, bounded replay, tenant schema registration,
   result-first Index apply/ack, shared-connector server lifecycle, shutdown, and
   enabled-worker readiness/aggregate guardrail metrics.
   **Remaining:** add dedicated consumer metrics, prove bounded replay/rescan repair,
   and retain compiled/runtime evidence for privacy, receipts, cleanup CLI, event
   relay/replay, schema concurrency, Index restart/redelivery/DLQ, storefront,
   Customer Admin, Blog/Forum, and Media behavior.
   **Done when:** every presentation consumer exposes one policy with retained
   evidence and no direct foreign-domain reads or projection-based authorization.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice, native/GraphQL transport
   selection, Media presentation, authenticated follow control, and read-only
   conflict recovery.
   **Directory/search decision:** directory/search must use Profiles-owned public
   profile records plus generic Index query contracts. Social Graph relation
   projection may support discovery/ranking inputs but cannot replace audience-bound
   authorization. Runtime UI/query work waits for the Index query port and a
   Profiles-owned public-profile source schema; the relation schema is not that
   profile source.
   **Remaining:** execute SSR/hydrate/GraphQL route, auth, i18n, Media
   direct/proxy/fallback, mutation conflict, durable receipt, relation-event, and
   accessibility evidence.

4. **Keep profile backfill owner-local.**
   **Status:** source-complete; compiled/runtime verification pending. The CLI uses
   owner auth/tenant/customer reads and optional Outbox publication while preserving
   dry-run semantics and aggregate telemetry.
   **Next verification:** success, dry-run, owner-read failure, plan/create failure,
   and event-publication failure.

5. **Complete audit and operational evidence.**
   **Status:** source-complete for Profiles operations, Social Graph command
   telemetry, durable receipts, bounded maintenance, transactional events, bounded
   replay, cleanup CLI, sealed event conversion, persisted schema registration,
   durable Index terminal recognition, default-off shared-connector lifecycle,
   bounded retry, DLQ ordering, graceful shutdown, readiness, and aggregate guardrail
   metrics.
   **Remaining:** dedicated consumer metrics, deployment retention approval,
   PostgreSQL concurrency/retention/replay/rollback, real broker and multi-replica
   evidence, and retained operator packets.

## Recheck checkpoint — 2026-07-27

- Preserved the receipts, cleanup, event, replay, CLI, migration, topology, and
  verification work from superseded draft PR #2237 in PR #2317.
- Rechecked privacy-before-presentation, bounded follower reads, owner-scoped writes,
  Media-owned descriptors, optimistic conflict recovery, and no automatic mutation
  retry.
- Rechecked receipt replay/conflict, completed-only cleanup, sealed transactional
  events, no-op/replay suppression, page-atomic replay, and explicit retention input.
- Approved Index as the first relation-event consumer while rejecting privacy caches,
  Notifications projections, and foreign relation-table reads as authorization.
- Added generic Index-owned schema registration and a transport-neutral projector.
- Added staged persistent consumption with durable schema/apply/terminal recognition
  before broker acknowledgement.
- Added default-off lifecycle, strict `outbox_iggy` gating, one shared EventRuntime
  connector, shared shutdown, bounded retry, exact-byte DLQ-before-ack, and
  acknowledgement-only recovery after durable apply.
- Added enabled-worker readiness through `runtime_guardrails`, `/health/ready`, and
  aggregate guardrail metrics without degrading disabled execution.
- Tests, formatters, Cargo commands, source verifiers, PostgreSQL, real-broker, and
  multi-replica scenarios remain maintainer-run or pending.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets`
- `cargo test -p rustok-index schema_registration --lib -- --nocapture`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index --all-targets`
- `cargo test -p rustok-social-graph --features index index::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets`
- `cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture`
- `cargo test -p rustok-server runtime_guardrails --lib -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph-cli --all-targets`
- `cargo test -p rustok-social-graph-cli -- --nocapture`
- `cargo test -p rustok-social-graph --test command_receipts_sqlite -- --nocapture`
- `cargo test -p rustok-social-graph --test receipt_cleanup_sqlite -- --nocapture`
- `cargo test -p rustok-social-graph --test relation_outbox_sqlite -- --nocapture`
- `cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-social-graph-command-receipts.mjs`
- `node scripts/verify/verify-social-graph-receipt-cleanup.mjs`
- `node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs`
- `node scripts/verify/verify-social-graph-relation-outbox.mjs`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-profiles-storefront-boundary.mjs`
- `rustok-cli profiles backfill --tenant-id <uuid> --dry-run`
- `rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run`

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain owner-internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. GraphQL hosts bind audience-aware loaders to the request.
7. Profile media resolves through Media owner ports only.
8. Remote Media selection is injected; Profiles does not know transport endpoints.
9. Module UI stays package-owned with explicit transports and package i18n.
10. Follow controls use owner ports, unique idempotency, optimistic revision, and no automatic retry.
11. Operational telemetry excludes presentation copy, email, Media/storage/provider details,
    idempotency keys, expected revisions, raw cursors/payloads, claims, roles, and channels.
12. Index/search projections may use sealed owner events, generic Index contracts,
    monotonic source versions, and bounded replay, but never authorize visibility.
13. Durable projection workers register the tenant schema and persist/recognize the
    owner result before ack. DLQ publication, when permitted, precedes source ack.
14. Enabled durable workers participate in readiness; disabled optional workers do not
    degrade presentation availability.
15. Update Profiles and affected owner docs with every boundary change.
