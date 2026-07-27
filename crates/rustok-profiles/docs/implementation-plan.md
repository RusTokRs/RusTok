# Implementation plan for `rustok-profiles`

## Current state

`rustok-profiles` owns public profile storage/translations, tags, handle and
visibility policy, `ProfileService`, raw owner reads, audience-bound
`ProfilePresentationService`, summary batching, GraphQL self-service surfaces,
`profile.updated`, owner-local backfill, Media-backed image presentation, and the
first module-owned storefront profile surface.

It is not an auth, customer, seller, or staff aggregate. Presentation consumers
must use the audience-bound service; raw reads remain owner-internal. Public
GraphQL, Customer Admin enrichment, Blog/Forum author cards, and storefront reads
share privacy-before-presentation ordering and hide restricted/unavailable rows as
absent.

Profiles composes `followers_only` through Social Graph owner ports. Follow state,
relation persistence, durable command receipts, receipt maintenance, relation
events, event replay, and operational cleanup remain owned by
`rustok-social-graph`; Profiles never reads those tables.

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
bounded, dry-run capable, and page-atomic; consumers remain optional and must apply
by relation id plus monotonic revision while keeping Social Graph authoritative.

The owner-local `rustok-social-graph-cli` provider now exposes
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
   transactional events, bounded receipt cleanup, historical event replay, and the
   owner-local cleanup CLI.
   **Remaining:** name concrete relation-event consumers and prove durable
   monotonic apply/ack; collect compiled and runtime evidence for privacy, receipts,
   cleanup CLI, event relay/replay, storefront, Customer Admin, Blog/Forum, and
   Media provider/delivery behavior.
   **Done when:** all presentation consumers expose one policy with retained
   evidence and no direct foreign-domain reads.

3. **Publish module-owned profile storefront UI.**
   **Status:** source-complete for the first Leptos slice, native/GraphQL transport
   selection, Media capability presentation, authenticated follow control, and
   read-only conflict recovery.
   **Remaining:** execute SSR/hydrate/GraphQL route, auth, i18n, Media
   direct/proxy/fallback, mutation conflict, durable receipt, relation-event, and
   accessibility evidence; decide whether directory/search is required.

4. **Keep profile backfill owner-local.**
   **Status:** source-complete; compiled/runtime verification pending. The Profiles
   CLI provider uses owner auth/tenant/customer reads and optional Outbox event
   publishing, preserving dry-run semantics and aggregate telemetry.
   **Next verification:** exercise success, dry-run, owner-read failure,
   profile-plan/create failure, and event-publication failure.

5. **Add audit and operational capabilities from owner contracts.**
   **Status:** source-complete for stable Profiles operations, Social Graph command
   telemetry, durable receipts, bounded receipt maintenance, transactional events,
   bounded event replay, and owner-local receipt-cleanup CLI composition.
   **Remaining:** concrete consumer durable apply/ack; deployment
   retention-window/cadence approval; PostgreSQL concurrency, retention, replay and
   rollback evidence; and retained runtime evidence.
   **Done when:** operations have typed owner ports, safe recovery guidance,
   retained evidence, and no auth/customer/receipt leakage.

## Recheck checkpoint — 2026-07-27

- Reconciled PR #2237 as one commit directly on current `main`, retaining current
  Translation topology/event-plan changes.
- Rechecked privacy-before-presentation, bounded followers-only reads, owner-scoped
  follow writes, Media-owned descriptors, optimistic revision recovery, and no
  automatic write retry.
- Added durable Social Graph receipts with exact response replay and mismatched-key
  conflict.
- Added service/system-only bounded cleanup with explicit cutoff, dry-run,
  all-candidate validation, retained-floor reporting, SQLite evidence, and safe
  aggregate telemetry.
- Added sealed transactional relation-state events, host-composed GraphQL/native
  buses, no-op/replay suppression, and rollback evidence.
- Added service/system-only bounded historical relation-event replay with exclusive
  UUID cursor, dry-run, page-atomic Outbox append, aggregate telemetry, and SQLite
  rollback evidence.
- Added `rustok-social-graph-cli` selected-distribution provider. Receipt cleanup
  now requires explicit tenant and retention days, derives cutoff in the owner-local
  adapter, delegates to the maintenance port, and enables no scheduler/default
  retention policy.
- Synchronized the central Profiles FFA/FBA row with the module-owned
  storefront and retained `not_started` FBA status pending runtime evidence.
- Regenerated and committed the combined Translation/Social Graph Events digest and
  refreshed the combined workspace lockfile through constrained automatic jobs.
- Compilation, tests, formatters, source-verifier execution, and runtime evidence
  remain maintainer-run and were not executed manually.

## Verification

- `cargo run -p rustok-events --example event_contract_digests -- --write`
- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- `cargo check -p rustok-profiles-storefront --all-targets`
- `cargo test -p rustok-profiles-storefront`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets`
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
12. Update Profiles and affected owner docs with every boundary change.
