# `rustok-notifications` Documentation

## Purpose

`rustok-notifications` is the notification owner for user notifications, templates, delivery channels, and preference management.

## Scope

- Notification templates, queues, delivery status, and user preferences;
- Multi-channel delivery (in-app, email, push);
- Read/unread tracking and batching.

## Integration

- Consumes domain events from Outbox to trigger notification delivery;
- Provides GraphQL and native ports for in-app inbox and settings.

## Verification

- `cargo test -p rustok-notifications`
- `cargo xtask module validate notifications`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)

# `rustok-notifications` live contract

## Responsibility zone

Notifications owns inbox/read state, exact unread counts, bounded mark-all,
selected-ID state commands, bounded exact-group state commands, durable grouping,
exact-group reads, bounded group summaries, authenticated native/GraphQL storefront reads
and open authorization, the module-owned grouped storefront UI, preferences, bounded fanout,
digests, retention, delivery attempts, intake receipts/quarantine, and replay/reconciliation. Source modules own semantic state, subscriptions, audience
facts, visibility, target authorization, and routes. Profiles and Social Graph own
recipient privacy. Delivery modules own channel transports.

## Integration boundary

`rustok-notifications-api` is the neutral source contract. Producers register
`NotificationSourceProviderFactory` values through `ModuleRuntimeExtensions`; the
server materializes them with `HostRuntimeContext`. Duplicate slugs, source identity
mismatches, and build failures are startup errors.

The owner does not decode platform envelopes and does not read producer-private or
Modules-private tables. The executable server injects envelope decoding and
cross-owner policy ports. Producer transactions remain independent from notification
availability.

Notifications remains absent from `settings.default_enabled`; tenants must have an
effective `notifications` capability before provider materialization, audience
resolution, or candidate processing.

## Persistence

Seven ordered module-local PostgreSQL/SQLite migrations own notification/read
lifecycle, delivery attempts, fanout jobs/items and leases, durable source inbox
state, outbox intake receipts/quarantine, preferences, digests, encrypted push
subscriptions, group-key population, and the group-summary access path:

1. `m20260721_000010_create_notification_persistence`;
2. `m20260722_000011_create_notification_source_inbox`;
3. `m20260722_000012_add_candidate_processing`;
4. `m20260723_000013_add_outbox_intake_receipts`;
5. `m20260723_000014_add_outbox_intake_rejections`;
6. `m20260726_000015_populate_notification_group_keys`;
7. `m20260726_000016_add_notification_group_summary_index`.

Accepted and rejected intake outcomes are keyed by outbox event ID and mutually
exclusive. Source inbox and accepted receipt commit in one transaction. Permanent
invalid envelopes are quarantined; retryable failures retain no terminal intake
record. Accepted replay re-decodes the current envelope and must match persisted
source identity. The intake consumer neither depends on nor mutates relay status.

Missing notification group keys are assigned as
`g1:{target_owner}:{target_id}`. Existing null values are backfilled and explicit
non-null keys remain authoritative. PostgreSQL assigns missing keys before insert;
SQLite assigns them inside the inserting transaction. The partial
`idx_notifications_group_summary` index orders non-archived grouped rows by exact
recipient latest activity. Existing `idx_notifications_group` supports exact group
scans, bounded group-state selection, and stored counts.

The schema stores no source-private payload, rendered HTML, contact address, phone
number, or plaintext push endpoint. Global server migration composition remains a
maintainer verification gate.

## Runtime pipeline

### Durable outbox intake

`NotificationOutboxIntakeWorker` selects supported committed `sys_events` rows in
stable `created_at/id` order, 32 by default and 64 maximum. Accepted receipts and
permanent rejections are anti-joined, preventing invalid head-of-line starvation.
The loop is default-off behind `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

### Durable source fanout

`NotificationFanoutService` is the canonical lease and persistence authority.
`NotificationFanoutWorker` selects tenant-scoped source/job work without acquiring
leases; the default/hard batch is 32/64 and one audience page is capped at 256.
Before each source or job call, the server resolves effective `notifications`
capability.

Disabled tenant work is moved to `retryable_error` for 300 seconds; temporary policy
lookup failure is deferred for 30 seconds. Owner CAS transitions increment attempt
count, set `next_attempt_at`, clear lease fields, and persist stable error metadata
before any producer call. The loop is default-off behind
`RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED` and creates only pending candidates.

A bounded producer scan may return zero recipients with a next cursor. The cursor
must differ from the claimed cursor; the owner persists it under the same lease CAS,
creates no candidates, and keeps the job pending. Oversized pages and stalled
cursors fail closed.

### Candidate policy and lifecycle-serialized inbox creation

`NotificationCandidateWorker` selects bounded tenant-scoped work without acquiring
a lease. Before canonical claim, the server calls
`EffectiveModulePolicyService::resolve_snapshot`, requires `notifications`, and
captures the deterministic policy revision plus the manifest default-enabled module
set. Disabled or unresolved work receives the 300/30-second owner CAS backoff
without invoking recipient privacy or source providers.

Enabled work is processed in this order:

1. claim or recover the candidate lease;
2. resolve exact preferences before wildcards;
3. evaluate Profiles/Social Graph recipient policy;
4. reauthorize the target for the recipient;
5. open the final notification transaction and validate the lease;
6. invoke `NotificationTenantCapabilityCommitGuard` with the observed policy
   revision and manifest defaults;
7. recheck preferences;
8. insert or validate one notification and complete the candidate under the same
   lease CAS.

The commit guard delegates to `SeaOrmModulePolicyRevisionConsumer`. The Modules
owner locks the `module.lifecycle` cursor and resolves `tenant_modules` on the
candidate transaction using already-observed manifest defaults. Current
`notifications` enablement and the observed revision must both match.

On PostgreSQL, the cursor uses `FOR UPDATE`, serializing final candidate commit with
tenant enable/disable commits. SQLite evidence covers transaction-bound resolution
and rollback only; it does not claim PostgreSQL contention evidence. Active-manifest,
artifact-security, maintenance, and node-readiness changes are not yet serialized by
this cursor.

The loop remains default-off behind
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`, requires ready recipient-policy
ports and `ModuleRegistry`, and never creates channel delivery attempts.

### Inbox open-time authorization

`NotificationInboxOpenService` loads one exact notification/tenant/recipient row.
Missing, cross-tenant, and cross-recipient rows return `Unavailable` before policy or
source calls. For an owned row, recipient privacy runs before source target
authorization. The service returns only a fresh route or `Unavailable`; it does not
expose the stored row, mutate inbox state, or enqueue delivery attempts.

### Bounded authorized inbox listing

`NotificationInboxListService` scans one exact tenant/recipient page in
`created_at DESC, id DESC` order. Requests default to 20 rows, are capped at 64, and
may apply one exact state filter. The versioned `i1` cursor preserves nanoseconds and
the UUID tie-breaker.

Every raw row passes through `NotificationInboxOpenService`. The read model exposes
typed source/template data, actor, priority, state, and inbox timestamps, but no
dedicated route or structural target fields. Continuation derives from the last raw
row, so suppression may produce an empty advancing page. Retryable owner failures
abort without a partial result. Listing mutates no state or delivery attempt.

### Exact and bounded inbox state commands

`NotificationInboxStateService` owns exact `mark_seen`, `mark_read`, `mark_unread`,
and `archive` transitions. The forward order is
`unread → seen → read → archived`; mark-unread reopens only seen/read, while archived
remains terminal. Idempotent or protected commands return `changed=false` without
rewriting timestamps.

`NotificationInboxMarkAllReadService`,
`NotificationInboxMarkAllUnreadService`, and
`NotificationInboxMarkAllArchiveService` select bounded exact-recipient pages and
delegate every row to the exact state owner. `NotificationInboxSelectedStateService`
applies one of the four commands to at most 64 explicit IDs in input order. Missing,
foreign, already-satisfied, and protected selected rows are all reported only as
`not_changed`, reducing existence-oracle detail.

`NotificationInboxGroupStateService` applies one bounded `mark_read`, `mark_unread`,
or `archive` action to one exact tenant/recipient/group. It validates the same opaque
191-byte group-key boundary as exact-group listing, selects only action-eligible rows
in `created_at DESC, id DESC` order, reuses the shared 20/64 bounds and `i1` cursor,
and delegates every selected identity to `NotificationInboxStateService`. Its page
returns only `scanned`, `changed`, `next_cursor`, and `has_more`.

These state owners call no privacy/source/target/delivery owner and create no
delivery attempt. Earlier per-row transitions remain durable and idempotent if a
later operation fails.

### Exact unread count

`NotificationInboxUnreadCountService` counts stored `unread` rows for one exact
non-nil tenant and recipient. Tenant, recipient, and state filters precede
aggregation, and the query reuses `idx_notifications_inbox`. Missing or foreign
scopes return zero. The count reflects stored state and converges after
reconciliation archives unavailable rows.

### Bounded inbox reconciliation

`NotificationInboxReconcileService` scans one bounded non-archived exact-recipient
page using the shared 20/64 bounds and `i1` cursor. Every row reuses open-time
privacy/source authorization. Allowed rows stay unchanged; `Unavailable` rows are
archived through the exact state owner. Foreign calls run after raw selection and
outside a notification transaction. Retryable failures stop the page, while earlier
archives remain durable and retry-safe.

### Durable group keys and exact-group listing

`m20260726_000015_populate_notification_group_keys` makes group identity durable at
the persistence boundary. `NotificationInboxGroupListService` reads one exact
tenant/recipient/group with an optional state filter, shared page bounds, the `i1`
cursor, and current open-time authorization. Suppressed rows produce sparse
advancing pages. The read changes no state or delivery attempt.

### Bounded group summaries

`NotificationInboxGroupSummaryService` returns groups with at least one non-archived
row, ordered by their latest non-archived `created_at DESC, id DESC` row. Requests
default to 20 raw groups and are capped at 64. Each result contains the opaque group
key, exact stored non-archived `item_count`, exact stored `unread_count`, and the
typed latest inbox item without a route.

The latest row passes current recipient privacy before source authorization.
Suppressed groups are omitted while continuation advances from the last raw group.
Retryable failures abort without a partial result. Counts intentionally reflect
stored owner state and converge after reconciliation. The read mutates no inbox
state or delivery attempt.

### Bounded group state commands

`NotificationInboxGroupStateService` completes the owner-side grouped command set.
`mark_read` selects unread/seen rows, `mark_unread` selects seen/read rows, and
`archive` selects all non-archived rows for one exact group. Direct unread-to-read,
seen-history preservation, mark-unread timestamp clearing, and terminal archive are
inherited from the exact state owner. Missing, foreign, and already-satisfied groups
return an empty page without notification identity.

SQLite source evidence is `tests/inbox_group_state_sqlite.rs`; the static contract is
`scripts/verify/verify-forum-notification-inbox-group-state.mjs`.

### Authenticated storefront ports, transports, and UI

`NotificationInboxStorefrontPort` derives owner scope from a human-user `PortContext` and
exposes unread count, bounded group summaries/items, fresh open authorization, and bounded
group-state commands. Read calls require a deadline; write calls require deadline and
idempotency admission. Tenant and recipient identity never appear in storefront request DTOs.

Native Leptos server functions are selected for SSR/hydrate. The feature-gated GraphQL query
root is selected for CSR/headless unread count, grouped reads, and fresh open authorization.
The GraphQL runtime receives the host database, materialized source registry, and current
recipient-policy runtime, then exposes only the shared owner port. No path falls back to the
other. GraphQL group-state mutations now delegate to the same owner port and require
explicit idempotency plus deadline admission.

The grouped Notifications view uses owner-backed SSR bootstrap, bounded pages, exact-group
expansion, stale-response guards, authoritative post-command refresh, and allowed-only route
navigation. It automatically reloads its bootstrap when the reactive auth token or tenant
changes and uses the same resolved transport context for exact unread count plus the first
bounded summary page. A manifest-driven generic header action exposes the localized route and
exact unread badge without a host import of the Notifications UI. Optional failures degrade by
omitting the action rather than breaking the storefront shell.

The server starts workers in intake → fanout → candidate order. Invalid or unreadable
flags remain disabled.

## Forum sources

Forum supports `forum.topic.created` and `forum.mention.user_added`. Its provider
accepts legacy journal UUID/sequence references and semantic source identities from
committed envelopes. With exact recipient context, active initially non-public topic-created
events materialize identifier-only descriptors and still reauthorize every bounded subscription
candidate. Mention handling verifies immutable relation and current target visibility. Pending
replies are retryable; closed, hidden, deleted, self-mentioned, or restricted sources fail closed.
Moderator audience expansion remains deferred.

## Pending capabilities

- serialize active-manifest, artifact-security, maintenance, and node-readiness
  policy changes with final candidate commits;
- PostgreSQL cursor/lease contention evidence and operational health/lag metrics;
- bounded moderator directory expansion;
- tenant-wide scheduled reconciliation and payload redaction;
- channel delivery enqueue and transports with delivery-time authorization;
- retention, quarantine replay/purge, and administrative repair.

## Maintainer verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-modules --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-modules --test policy_commit_guard_sqlite -- --nocapture
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sparse_page_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_open_authorization_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_listing_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_state_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_count_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_read_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_unread_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_archive_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_selected_state_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_reconcile_sqlite -- --nocapture
cargo test -p rustok-notifications --test group_key_population_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_group_listing_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_group_summary_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_group_state_sqlite -- --nocapture
cargo test -p rustok-notifications --test outbox_intake_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_policy_deferral_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-notifications-fanout-worker.mjs
node scripts/verify/verify-forum-notification-inbox-open-authorization.mjs
node scripts/verify/verify-forum-notification-inbox-open-privacy.mjs
node scripts/verify/verify-forum-notification-inbox-listing.mjs
node scripts/verify/verify-forum-notification-inbox-state-mutations.mjs
node scripts/verify/verify-forum-notification-inbox-reconciliation.mjs
node scripts/verify/verify-forum-notification-inbox-mark-unread.mjs
node scripts/verify/verify-forum-notification-inbox-unread-count.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-read.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-unread.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-archive.mjs
node scripts/verify/verify-forum-notification-inbox-selected-state.mjs
node scripts/verify/verify-forum-notification-group-key-population.mjs
node scripts/verify/verify-forum-notification-inbox-group-listing.mjs
node scripts/verify/verify-forum-notification-inbox-group-summaries.mjs
node scripts/verify/verify-forum-notification-inbox-group-state.mjs
node scripts/verify/verify-forum-notification-inbox-storefront-port.mjs
node scripts/verify/verify-forum-notification-inbox-native-storefront-adapter.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs
node scripts/verify/verify-forum-notification-navigation-badge.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-open-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
cargo xtask module validate notifications
```

These commands were not run while publishing
`NOTIFY-03D/03E/03F/03G/03H/03I` or
`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO/20AP`.

## Related documents

- [Module README](../README.md)
- [Implementation gates](implementation-plan.md)
- [Outbox intake contract](../contracts/notifications-outbox-intake.json)
- [Fanout worker contract](../contracts/notifications-fanout-worker.json)
- [Candidate worker contract](../contracts/notifications-candidate-worker.json)
- Canonical roadmap: `crates/rustok-forum/docs/implementation-plan.md`
