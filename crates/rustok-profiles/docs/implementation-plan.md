# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns public profile storage/translations, tags, handle and
visibility policy, `ProfileService`, raw owner reads, audience-bound
`ProfilePresentationService`, summary batching, GraphQL self-service surfaces,
`profile.updated`, owner-local backfill, Media-backed image presentation, and the
first module-owned storefront profile surface.

It is not an auth, customer, seller, staff, Social Graph, Index, or search
aggregate. Presentation consumers must use the audience-bound service; raw reads
remain owner-internal. Public GraphQL, Customer Admin enrichment, Blog/Forum author
cards, and storefront reads share privacy-before-presentation ordering and hide
restricted/unavailable rows as absent.

Profiles composes `followers_only` through authoritative Social Graph owner ports.
Follow state, relation persistence, durable command receipts, receipt maintenance,
relation events, event replay, and operational cleanup remain owned by
`rustok-social-graph`; Profiles never reads those tables or treats an event
projection as an authorization source.

Media descriptors remain Media-owned. Profiles revalidates tenant/uploader/MIME
and exposes only Media-selected descriptors. Host-selected embedded/grpc providers
are shared by GraphQL and native storefront; Profiles does not know storage keys,
provider endpoints, or ingress construction.

The module-owned storefront mounts `/modules/profiles?handle=<handle>`, provides
SSR-first native and explicit GraphQL compatibility transports, renders approved
avatar/banner descriptors, and exposes authenticated follow/unfollow with unique
idempotency keys, optimistic revisions, and one read-only conflict refresh without
automatic mutation retry.

Profiles and Social Graph operations use separate owner telemetry. Profiles writes
and backfill exclude display copy, source email, generated handle, locale values,
Media ids/URLs, and provider details. Social Graph command, receipt-cleanup, and
relation-event replay telemetry is aggregate or identity-bounded as documented by
the Social Graph owner and excludes receipt payloads, idempotency keys, raw replay
cursors, claims, roles, channel, and request correlation.

Social Graph durable receipts bind one tenant-scoped normalized idempotency key to
the complete relation command. Exact replay returns the committed response without
rewinding live state; mismatched reuse fails with a typed conflict. Receipt
reservation, relation mutation, event append when state changes, response snapshot,
and completion share one transaction.

`social_graph.relation.state_changed` is a sealed typed persisted-revision fact.
Live no-op and receipt replay emit nothing. Event failure rolls relation and receipt
back together. Historical owner replay is service/system-only, tenant/cursor
bounded, dry-run capable, and page-atomic.

The first approved consumer is the generic `rustok-index` relation projection. A
feature-gated Social Graph owner adapter maps the sealed event to an `IndexMutation`:
active revisions upsert a non-localized relation record, inactive revisions create
a tombstone, and the relation revision is the Index `source_version`. This gives
Index-owned inbox dedupe and stale-revision suppression without moving privacy
policy or relation authority out of Social Graph. Durable broker composition,
schema registration, result-first acknowledgement, and replay-driven drift repair
remain pending runtime work.

The owner-local `rustok-social-graph-cli` provider exposes
`social_graph receipt-cleanup`. Operators must supply tenant and positive retention
days; the adapter derives the cutoff and calls the Social Graph maintenance port.
Only batch size has a bounded default. No scheduler or automatic retention policy
is enabled.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- The module has a module-owned Leptos storefront package with native/GraphQL
  transports, package i18n, explicit fail-closed transport selection, optimistic
  recovery, and Media/Social Graph owner composition.
- FBA remains `not_started` until compiled/live transport, Media isolation,
  provider identity, public ingress, storage delivery, and runtime evidence exist.

## Results and next work

1. **Keep owner reads separate from audience-bound presentation.**
   **Status:** source-complete for current consumers. Privacy is evaluated before
   localized summary/tag loading, and foreign modules do not read profile tables.
   **Revisit when:** production telemetry proves a need for another bounded owner
   projection.

2. **Finish followers-only and downstream presentation policy.**
   **Status:** source-complete for owner privacy ports, public GraphQL lookups,
   author cards, storefront, and Customer Admin enrichment. Social Graph provides
   directional follow reads, revision-bearing state, receipt-aware commands,
   transactional events, bounded receipt cleanup, historical event replay, the
   owner-local cleanup CLI, and the first owner-published Index mutation adapter.
   **Remaining:** compose the durable Index consumer group and schema registration,
   persist/recognize the Index result before broker acknowledgement, prove bounded
   replay/rescan drift repair, and collect compiled/runtime evidence for privacy,
   receipts, cleanup CLI, event relay/replay, storefront, Customer Admin,
   Blog/Forum, and Media behavior.
   **Done when:** all presentation consumers expose one policy with retained
   evidence and no direct foreign-domain reads or projection-based authorization.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice, native/GraphQL transport
   selection, Media capability presentation, authenticated follow control, and
   read-only conflict recovery.
   **Directory/search decision:** a directory/search capability is required, but it
   must be built on Profiles-owned public profile records plus generic Index query
   contracts. Social Graph relation projection may support discovery/ranking
   inputs; it must not replace audience-bound profile authorization. Runtime UI and
   query work waits for the Index query port and source schema registration.
   **Remaining:** execute SSR/hydrate/GraphQL route, auth, i18n, Media
   direct/proxy/fallback, mutation conflict, durable receipt, relation-event, and
   accessibility evidence.

4. **Keep profile backfill owner-local.**
   **Status:** source-complete; compiled/runtime verification pending. The Profiles
   CLI provider uses owner auth/tenant/customer reads and optional Outbox event
   publishing, preserving dry-run semantics and aggregate telemetry.
   **Next verification:** exercise success, dry-run, owner-read failure,
   profile-plan/create failure, and event-publication failure.

5. **Add audit and operational capabilities from owner contracts.**
   **Status:** source-complete for stable Profiles operations, Social Graph command
   telemetry, durable receipts, bounded receipt maintenance, transactional events,
   bounded event replay, owner-local receipt-cleanup CLI composition, and the pure
   sealed-event-to-Index mutation adapter.
   **Remaining:** durable Index consumer registration/apply/ack, deployment
   retention-window/cadence approval, PostgreSQL concurrency/retention/replay and
   rollback evidence, and retained runtime evidence.
   **Done when:** operations have typed owner ports, safe recovery guidance,
   retained evidence, and no auth/customer/receipt leakage.

## Recheck checkpoint — 2026-07-27

- Reconciled superseded draft PR #2237 and preserved its receipts, cleanup, event,
  replay, CLI, migration, topology, lockfile, and plan work in a fresh branch.
- Rechecked the new branch against current `main`; the three intervening commits
  affecting Forum, Index, Commerce, and Inventory touch disjoint paths and remain
  outside this change.
- Rechecked privacy-before-presentation, bounded followers-only reads, owner-scoped
  follow writes, Media-owned descriptors, optimistic revision recovery, and no
  automatic write retry.
- Rechecked durable Social Graph receipt replay/conflict, completed-only cleanup,
  sealed transactional relation events, no-op/replay suppression, page-atomic
  replay, and explicit retention input at source level.
- Approved `rustok-index` as the first concrete relation-event consumer and rejected
  Profiles privacy caching, Notifications privacy projection, or any foreign
  relation table read as substitutes for authoritative owner ports.
- Added feature-gated Social Graph schema/mutation conversion using relation id as
  entity identity and positive relation revision as Index source version. Active
  state upserts; inactive state writes a tombstone.
- Durable consumer-group composition, schema registration, broker acknowledgement,
  replay-driven repair, compilation, tests, formatters, source verifiers, and
  runtime evidence remain maintainer-run or pending.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index --all-targets`
- `cargo test -p rustok-social-graph --features index index::tests -- --nocapture`
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
- `node scripts/verify/verify-profiles-storefront-boundary.mjs`
- `rustok-cli profiles backfill --tenant-id <uuid> --dry-run`
- `rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run`

## Change rules

1. Keep profile policy and storage in Profiles.
2. Keep privacy reads independent from localized presentation and foreign tables.
3. Public GraphQL/storefront reads must use the canonical visibility matrix.
4. Presentation consumers use `ProfilePresentationService`; raw readers remain
   owner-internal.
5. `followers_only` resolves through bounded fail-closed Social Graph ports.
6. GraphQL hosts bind audience-aware loaders to the request.
7. Profile media resolves through Media owner ports only.
8. Remote Media selection is injected; Profiles does not know transport endpoints.
9. Module UI stays package-owned with explicit transports and package i18n.
10. Follow controls use owner ports, unique idempotency, optimistic revision, and no
    automatic write retry.
11. Operational telemetry and events may contain only documented stable identities,
    aggregate counters, modes, limits, cursor presence, revisions, outcomes, and
    durations; never presentation copy, emails, Media/storage/provider details,
    idempotency keys, expected revisions, raw cursors, claims, roles, channels, or
    receipt payloads.
12. Index/search projections are optional consumers. They may use sealed owner events,
    generic Index contracts, monotonic source versions, and bounded owner replay;
    they must never authorize profile visibility or read Social Graph tables.
13. Update Profiles and affected owner docs with every boundary change.
