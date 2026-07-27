# `rustok-notifications`

## Purpose

`rustok-notifications` owns inbox state, recipient preferences, bounded fanout,
grouping, digests, retention, and delivery-attempt lifecycle. The implemented
pipeline now covers durable outbox intake, source materialization, bounded
audience expansion, recipient policy, one idempotent in-app notification, exact
open-time authorization, bounded authorized inbox listing, exact
seen/read/mark-unread/archive state APIs, exact unread counting, bounded
mark-all-read, bounded mark-all-unread, bounded mark-all-archive, bounded
selected-ID state commands, bounded exact-recipient reconciliation, durable
target group-key population, bounded exact-group listing, bounded group
summaries with stored counts plus an authorized latest-item projection, and
bounded exact-group state commands. Channel delivery remains a later workflow.

## Responsibilities

- consume committed semantic source envelopes outside producer transactions;
- materialize neutral producer-owned source factories after executable host
  services exist;
- own notification, delivery, fanout, preference, digest, source-inbox, receipt,
  quarantine, and encrypted push-subscription state;
- resolve audiences in bounded cursor pages under recoverable leases;
- enforce effective tenant capability before source and candidate provider calls;
- durably defer disabled or unresolved tenant work so bounded queues continue to
  later tenants;
- serialize final candidate creation with PostgreSQL tenant lifecycle toggles;
- apply preferences, recipient privacy, and current target authorization before
  creating an inbox row;
- assign and backfill stable target group keys at the Notifications persistence
  boundary while preserving explicit keys;
- recheck recipient privacy and source target authorization at inbox open/list
  time;
- list exact-recipient inbox rows through bounded sparse keyset pages;
- list one exact stored notification group through the same authorization-aware
  sparse pagination contract;
- summarize non-archived groups through bounded latest-activity pages with exact
  stored item/unread counts and an authorized latest item;
- apply exact-item seen/read/mark-unread/archive transitions with terminal archive;
- count exact-recipient unread owner state without deriving totals from list pages;
- mark one bounded exact-recipient unread/seen page as read through the exact state
  owner;
- mark one bounded exact-recipient seen/read page as unread through the exact state
  owner;
- archive one bounded exact-recipient non-archived page through the exact state
  owner;
- apply one bounded explicit-ID state command set through the exact state owner;
- apply one bounded exact-group mark-read, mark-unread, or archive command through
  the exact state owner;
- reconcile one bounded exact-recipient page against current privacy/source policy;
- own replay, reconciliation, retention, and delivery lifecycle.

## Non-responsibilities

- producer subscriptions or source lifecycle;
- source-private tables and producer envelope decoding;
- Profiles or Social Graph persistence;
- SMTP, push-vendor, or SMS SDK implementation;
- synchronous notification calls inside producer transactions.

## Public entry points

- `NotificationsModule` / `NotificationsService`;
- `NotificationOutboxEnvelopeDecoder` / `NotificationOutboxIntakeWorker`;
- `NotificationFanoutService` / `NotificationFanoutWorker`;
- `NotificationFanoutPolicyDeferral`;
- `NotificationCandidateService` / `NotificationCandidateWorker`;
- `NotificationCandidateWorkItem` / `NotificationCandidatePolicyDeferral`;
- `NotificationTenantCapabilityCommitGuard` and its request/decision contracts;
- `NotificationRecipientPolicy` / `NotificationRecipientPolicyRuntime`;
- `NotificationInboxOpenService` and exact open request/decision contracts;
- `NotificationInboxListService` and bounded list request/page/read-model contracts;
- `NotificationInboxGroupListService` and bounded exact-group request contracts;
- `NotificationInboxGroupSummaryService` and bounded group-summary request/page contracts;
- `NotificationInboxGroupStateService` and bounded exact-group state request/page contracts;
- `NotificationInboxStateService` and exact state request/decision/snapshot contracts;
- `NotificationInboxUnreadCountService` and exact count request/result contracts;
- `NotificationInboxMarkAllReadService` and bounded request/page contracts;
- `NotificationInboxMarkAllUnreadService` and bounded request/page contracts;
- `NotificationInboxMarkAllArchiveService` and bounded request/page contracts;
- `NotificationInboxSelectedStateService` and bounded explicit-ID state contracts;
- `NotificationInboxReconcileService` and bounded reconciliation request/page contracts;
- `NotificationInboxStorefrontPort` and the in-process authenticated-user owner facade;
- feature-gated Notifications GraphQL query root and host schema-data composition;
- module-owned native/GraphQL storefront transport and grouped Leptos UI packages;
- `rustok_notifications::api`, `entities`, `model`, and `migrations`.

## Persistence

The owner exposes seven ordered PostgreSQL/SQLite migrations:

1. `m20260721_000010_create_notification_persistence`;
2. `m20260722_000011_create_notification_source_inbox`;
3. `m20260722_000012_add_candidate_processing`;
4. `m20260723_000013_add_outbox_intake_receipts`;
5. `m20260723_000014_add_outbox_intake_rejections`;
6. `m20260726_000015_populate_notification_group_keys`;
7. `m20260726_000016_add_notification_group_summary_index`.

Accepted outbox envelopes receive a durable receipt linked to the semantic source
inbox row. Permanently invalid envelopes receive an owner-local quarantine row;
retryable failures receive no terminal record. Accepted and rejected outcomes are
mutually exclusive. The schema stores no source-private payload, rendered HTML,
email address, phone number, or plaintext push endpoint.

The sixth migration backfills missing notification group keys and installs
backend-specific insert triggers. Missing keys become
`g1:{target_owner}:{target_id}`; explicit non-null keys are preserved. The format
is bounded by the existing 191-byte group-key contract and groups notification
variants for one source-owned target UUID without changing target authorization
metadata.

The seventh migration adds `idx_notifications_group_summary`, a partial index over
non-archived grouped rows ordered by exact recipient latest activity. Existing
`idx_notifications_group` continues to serve exact group scans, bounded group-state
selection, and stored counts.

Global `rustok-migrations` composition remains a maintainer verification gate.

## Runtime pipeline

### 1. Outbox intake

`NotificationOutboxIntakeWorker` selects committed supported `sys_events` rows in
stable `created_at/id` order, 32 by default and 64 maximum. It does not inspect or
mutate relay status. Platform envelope decoding is injected by the executable
server; the owner has no direct `rustok-events`, `rustok-outbox`, or Forum
dependency.

Current mappings are:

- root `forum.topic.created` → source identity `topic_id/1`;
- sealed `forum.mention.user_added` → envelope ID and `source_revision_id`.

The host loop is default-off behind
`RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

### 2. Source fanout

`NotificationFanoutWorker` selects tenant-scoped source and job work in stable
`created_at/id` order. The default/hard batch is 32/64 and the audience page cap
is 256. Selection acquires no lease; every claim, descriptor materialization, and
page persistence delegates to `NotificationFanoutService`.

Before every source or job claim, the server calls
`EffectiveModulePolicyService::is_enabled(..., "notifications")`. Disabled or
unresolved tenant policy fails closed before any producer provider call. Disabled
work receives a 300-second durable retry backoff; temporary policy lookup failure
receives 30 seconds. Both paths increment attempt count, clear lease fields,
persist stable error metadata, and remove the row from the bounded queue head.
The host loop is default-off behind
`RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`.

A producer may return a sparse page with zero recipients after scanning one
bounded source page. Such a page must carry a cursor different from the claimed
cursor. The owner persists that cursor under the same lease/CAS transition,
creates no candidates, and keeps the job pending. Oversized pages and stalled
cursors remain terminal provider failures.

Fanout creates only idempotent pending candidates—never final notifications or
delivery attempts.

### 3. Candidate policy and commit guard

`NotificationCandidateWorker` selects tenant-scoped candidate work. Before
canonical claim, the server resolves one `EffectiveModulePolicySnapshot` containing
the deterministic `policy_revision` and the exact manifest default-enabled module
set used to compute it. Disabled candidates receive a 300-second retry backoff;
temporary policy lookup failure receives 30 seconds. No recipient privacy policy
or source provider is called for deferred work.

When capability is enabled, the service claims the candidate, resolves exact
preference scopes before wildcards, evaluates Profiles/Social Graph recipient
policy, and reauthorizes the source target. The final notification transaction then:

1. validates the candidate lease;
2. invokes `NotificationTenantCapabilityCommitGuard` with the observed revision and
   manifest defaults;
3. locks the Modules-owned `module.lifecycle` policy cursor;
4. resolves tenant overrides through the Modules owner on the same transaction;
5. requires current `notifications` enablement and the observed revision;
6. rechecks preferences;
7. inserts or validates one notification and completes the candidate.

The persistence trigger assigns a missing durable group key before PostgreSQL
insert completion or within the same SQLite transaction. Candidate code remains
neutral and does not need producer-specific grouping fields.

The manifest is not reloaded through another pool connection while the final
transaction is active. This avoids a small-pool/SQLite connection deadlock and
keeps the commit guard limited to transaction-bound owner reads. Manifest mutation
is deliberately outside the cursor guarantee and remains a separate gate.

On PostgreSQL, the cursor `FOR UPDATE` lock serializes this final transaction with
tenant lifecycle enable/disable commits. Whichever transaction commits first is
authoritative: a prior disable rejects notification creation, while a candidate
that already owns the lock commits before the later disable. Revision changes or
disabled capability roll back the notification transaction and move the candidate
to durable retry state.

SQLite scenarios prove rollback, revision rejection, and transaction-bound policy
resolution, but do not claim PostgreSQL row-lock concurrency evidence. Active
manifest, artifact-security, maintenance, and node-readiness mutations are not yet
serialized by this lifecycle cursor and remain a separate policy-expansion gate.

The candidate loop is default-off behind
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`. It requires a materialized source
registry, ready recipient-policy ports, and the shared `ModuleRegistry`. Candidate
finalization creates no channel delivery attempt.

### 4. Inbox reads

`NotificationInboxOpenService` loads one exact tenant/recipient-owned notification,
re-evaluates recipient privacy, and asks the source owner for a current target route.
Missing, foreign, suppressed, or stale targets all return `Unavailable` without
exposing their distinction. Retryable owner failures remain retryable.

`NotificationInboxListService` scans exact-recipient rows in
`created_at DESC, id DESC` order with a default page of 20 and a hard cap of 64.
The versioned cursor preserves timestamp nanoseconds and the UUID tie-breaker. Each
raw row reuses the open-time privacy and source authorization pipeline; suppressed
rows may produce an empty page with a next cursor because progress is based on the
last scanned raw row. Retryable failures abort the page without a partial result.

The list read model exposes typed semantic/template fields and inbox timestamps but
adds no dedicated target route or structural target fields. Open and list reads do
not mutate seen/read/archive timestamps or create delivery attempts.

### 5. Exact inbox state mutations

`NotificationInboxStateService` owns exact notification/tenant/recipient commands.
`mark_seen` advances only unread rows to seen. `mark_read` advances unread or seen
rows to read; a direct unread-to-read transition assigns `seen_at` and `read_at`
from the same instant. `mark_unread` returns seen or read rows to unread and clears
both `seen_at` and `read_at`. `archive` advances every non-archived state to archived
and preserves prior seen/read timestamps. This extends the exact seen/read/archive
state APIs with one explicit reopen command.

Archived remains terminal: `mark_seen`, `mark_read`, and `mark_unread` cannot reopen
it. Repeated commands for an already matching or protected state return the current
snapshot with `changed=false` and preserve state timestamps plus `updated_at`.
Missing, cross-tenant, and cross-recipient rows return the same `Unavailable`
decision.

The service exposes only state and inbox timestamps, calls no privacy/source or
delivery owner, and creates no delivery attempt. SQLite owner evidence is
`tests/inbox_state_sqlite.rs`. Exact-item, bounded mark-all, selected-ID, and bounded
exact-group state commands are delivered. Authenticated native transport and grouped UI are
also delivered; GraphQL group-state write parity remains open.

### 6. Exact unread count

`NotificationInboxUnreadCountService` counts rows in stored owner state `unread` for
one exact non-nil tenant and recipient. The query applies tenant, recipient, and
state filters before aggregation and reuses `idx_notifications_inbox`; callers must
not derive totals from bounded or privacy-filtered list pages. Empty, missing,
cross-tenant, and cross-recipient scopes all return `unread_count=0`, without a
notification-level existence oracle.

The count reflects stored owner state. Current recipient privacy or source changes
affect it after exact or scheduled reconciliation archives unavailable rows. The
service invokes no recipient-policy, source, target, or delivery owner, mutates no
notification timestamps or delivery attempts, and exposes no notification or target
identity. SQLite evidence is `tests/inbox_count_sqlite.rs`. The exact count is exposed through
the authenticated native and GraphQL storefront read planes and the manifest-mounted unread badge.

### 7. Bounded mark-all-read

`NotificationInboxMarkAllReadService` selects only stored `unread` or `seen` rows
for one exact non-nil tenant and recipient in `created_at DESC, id DESC` order. A
request defaults to 20 rows, is capped at 64, and reuses the versioned `i1` inbox
cursor with timestamp nanoseconds and the UUID tie-breaker. Read and archived rows
remain outside selection.

One bounded raw page is loaded before mutation. Each selected row delegates to
`NotificationInboxStateService::mark_read`, preserving direct unread-to-read
`seen_at/read_at` equality and seen-to-read `seen_at` history. The response exposes
only `scanned`, `marked_read`, `next_cursor`, and `has_more`. Empty, missing,
cross-tenant, and cross-recipient scopes return an empty page without notification
identity.

The command calls no privacy/source/target/delivery owner and creates or mutates no
delivery attempt. Earlier exact transitions are durable and idempotent if a later
database operation fails, so callers may resume from the same request cursor.
SQLite evidence is `tests/inbox_mark_all_read_sqlite.rs`; the source guard is
`scripts/verify/verify-forum-notification-inbox-mark-all-read.mjs`.

### 8. Bounded mark-all-unread

`NotificationInboxMarkAllUnreadService` selects only stored `seen` or `read` rows
for one exact non-nil tenant and recipient in `created_at DESC, id DESC` order. A
request defaults to 20 rows, is capped at 64, and reuses the versioned `i1` inbox
cursor with timestamp nanoseconds and the UUID tie-breaker. Already-unread and
archived rows remain outside selection.

One bounded raw page is loaded before mutation. Each selected row delegates to
`NotificationInboxStateService::mark_unread`, clearing `seen_at` and `read_at`
while archived remains terminal. The response exposes only `scanned`,
`marked_unread`, `next_cursor`, and `has_more`. Empty, missing, cross-tenant, and
cross-recipient scopes return an empty page without notification identity.

The command calls no privacy/source/target/delivery owner and creates or mutates no
delivery attempt. Earlier exact transitions are durable and idempotent if a later
database operation fails, so callers may resume from the same request cursor.
SQLite evidence is `tests/inbox_mark_all_unread_sqlite.rs`; the source guard is
`scripts/verify/verify-forum-notification-inbox-mark-all-unread.mjs`.

### 9. Bounded mark-all-archive

`NotificationInboxMarkAllArchiveService` selects only stored `unread`, `seen`, or
`read` rows for one exact non-nil tenant and recipient in
`created_at DESC, id DESC` order. A request defaults to 20 rows, is capped at 64,
and reuses the versioned `i1` inbox cursor with timestamp nanoseconds and the UUID
tie-breaker. Already-archived rows remain outside selection.

One bounded raw page is loaded before mutation. Each selected row delegates to
`NotificationInboxStateService::archive`, preserving any existing `seen_at` and
`read_at` values while assigning `archived_at` and keeping archive terminal. The
response exposes only `scanned`, `marked_archived`, `next_cursor`, and `has_more`.
Empty, missing, cross-tenant, and cross-recipient scopes return an empty page
without notification identity.

The command calls no privacy/source/target/delivery owner and creates or mutates no
delivery attempt. Earlier exact transitions are durable and idempotent if a later
database operation fails, so callers may resume from the same request cursor.
SQLite evidence is `tests/inbox_mark_all_archive_sqlite.rs`; the source guard is
`scripts/verify/verify-forum-notification-inbox-mark-all-archive.mjs`.

### 10. Bounded inbox reconciliation

`NotificationInboxReconcileService` scans only non-archived rows for one exact
tenant/recipient in `created_at DESC, id DESC` order. It uses the same default/hard
page bounds of 20/64 and reuses the crate-private `i1` inbox cursor with timestamp
nanoseconds and the UUID tie-breaker.

Every raw row reuses `NotificationInboxOpenService`, preserving current recipient
privacy before source target authorization. An allowed row remains unchanged. A row
whose current privacy or source policy returns `Unavailable` is archived through
`NotificationInboxStateService`, preserving existing seen/read timestamps. Foreign
owner calls run after raw selection and outside a notification database transaction.

A retryable owner failure stops the page. Any earlier per-row archives are durable
and idempotent, so restarting from the same cursor safely skips already archived
rows. The response exposes only scanned/archived counts and continuation metadata;
it contains no route, source target, or notification identity. SQLite evidence is
`tests/inbox_reconcile_sqlite.rs`. Tenant-wide scheduled reconciliation, payload
redaction, transport wiring, and UI remain closed.

### 11. Group-key population and exact-group listing

`m20260726_000015_populate_notification_group_keys` assigns every missing group key
as `g1:{target_owner}:{target_id}` and backfills historical null values. Explicit
keys remain unchanged. PostgreSQL uses a `BEFORE INSERT` trigger; SQLite uses an
`AFTER INSERT` trigger whose update completes inside the inserting transaction.

`NotificationInboxGroupListService` selects one exact tenant, recipient, and stored
group key with an optional exact state filter. It reuses the shared 20/64 bounds,
the versioned `i1` cursor, and the open-time privacy/source authorization pipeline.
Sparse pages advance by raw rows. The read mutates no inbox timestamp and creates
no delivery attempt.

SQLite evidence is `tests/group_key_population_sqlite.rs` and
`tests/inbox_group_listing_sqlite.rs`.

### 12. Bounded group summaries

`NotificationInboxGroupSummaryService` selects only groups with at least one
non-archived row for one exact tenant and recipient. Raw groups are ordered by their
latest non-archived `created_at DESC, id DESC` row and use the shared 20/64 page
bounds plus the versioned `i1` cursor.

Each summary exposes the opaque group key, exact stored non-archived `item_count`,
exact stored `unread_count`, and the typed latest inbox item without a target route.
The latest row reuses `NotificationInboxOpenService`, so recipient privacy is
checked before source authorization. Suppressed groups are omitted while the raw
group cursor advances. Retryable owner failures abort without a partial result.

Counts reflect stored owner state rather than privacy-filtered rows and converge
after reconciliation archives unavailable rows. The read changes no state or inbox
timestamp and creates no delivery attempt. SQLite evidence is
`tests/inbox_group_summary_sqlite.rs`.

### 13. Bounded exact-group state commands

`NotificationInboxGroupStateService` accepts one exact tenant, recipient, opaque
group key, typed action, optional cursor, and limit. Requests default to 20 eligible
rows and are capped at 64. Selection is ordered by `created_at DESC, id DESC` and
is action-specific: unread/seen for `mark_read`, seen/read for `mark_unread`, and
all non-archived rows for `archive`.

Each selected identity delegates to `NotificationInboxStateService`, preserving
direct unread-to-read timestamp equality, seen-to-read history, mark-unread timestamp
clearing, and terminal archive. Continuation derives from the last scanned eligible
row. The response exposes only `scanned`, `changed`, `next_cursor`, and `has_more`.
Missing, foreign, and already-satisfied scopes are empty without notification
identity. The command calls no privacy/source/target/delivery owner and changes no
delivery attempt.

SQLite evidence is `tests/inbox_group_state_sqlite.rs`; the source guard is
`scripts/verify/verify-forum-notification-inbox-group-state.mjs`.

### 14. Authenticated storefront transport and grouped UI

`NotificationInboxStorefrontPort` is the transport-neutral authenticated-user boundary for
unread count, bounded group summaries/items, fresh open authorization, and bounded group
state commands. Requests carry no tenant or recipient fields: tenant comes from
`PortContext.tenant_id`, recipient from the human-user actor, reads require a deadline, and
writes additionally require idempotency semantics.

The native Leptos server-function adapter serves SSR/hydrate. The feature-gated GraphQL query
root serves CSR/headless unread count, group summaries/items, and fresh open authorization.
Both reuse the same owner port and host-composed source registry and recipient-policy runtime;
there is no transport fallback and no direct storefront database authority. GraphQL now
exposes the same bounded exact-group state commands with typed actions and explicit
idempotency admission.

The module-owned grouped storefront view performs an owner-backed SSR bootstrap, bounded raw
paging, one-group expansion, stale-response rejection, authoritative refresh after writes,
and browser navigation only after `NotificationStorefrontOpenDecision::Allowed`. It
automatically reloads its bootstrap when the reactive auth token or tenant changes and uses the
same resolved transport context for exact unread count plus the first bounded summary page. The
generic storefront header resolves the Notifications action from manifest metadata, builds the
route through `UiRouteContext`, and shows the exact unread badge only when positive while
retaining the link at zero. Optional capability failures hide the action without failing the
header.

Source contracts are guarded by the `FORUM-20AG` through `FORUM-20AO` machine contracts and
matching `verify-forum-notification-*` scripts. These source slices remain unvalidated by the
implementation agent.

The server bootstrap order is intake → fanout → candidate. All loops use the
shared shutdown signal and check it between work items.

## Forum integration

Forum publishes `forum.topic.created` and `forum.mention.user_added` through the
neutral API. Its provider accepts both legacy journal identity/revision references
and semantic identities derived from committed envelopes. Mention processing
still verifies the exact immutable relation and current topic/reply visibility.
Moderator audience expansion remains deferred until a bounded owner directory
port exists.

Notifications remains outside `settings.default_enabled`; producer commands
continue to succeed when the module is absent or disabled.

## Remaining gates

- serialize active-manifest, artifact-security, maintenance, and node-readiness
  policy changes with final candidate commits;
- bounded moderator-directory expansion;
- tenant-wide scheduled reconciliation and payload redaction;
- channel delivery enqueue with delivery-time authorization;
- PostgreSQL cursor/lease contention evidence and worker health/lag metrics;
- retention, quarantine replay/purge, and administrative repair.

## Documentation

- [Live contract](docs/README.md)
- [Implementation gates](docs/implementation-plan.md)
- [Outbox intake contract](contracts/notifications-outbox-intake.json)
- [Fanout worker contract](contracts/notifications-fanout-worker.json)
- [Candidate worker contract](contracts/notifications-candidate-worker.json)
- Canonical cross-module roadmap:
  `crates/rustok-forum/docs/implementation-plan.md`
