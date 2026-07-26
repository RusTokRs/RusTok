# `rustok-notifications`

## Purpose

`rustok-notifications` owns inbox state, recipient preferences, bounded fanout,
grouping, digests, retention, and delivery-attempt lifecycle. The implemented
pipeline now covers durable outbox intake, source materialization, bounded
audience expansion, recipient policy, one idempotent in-app notification, exact
open-time authorization, bounded authorized inbox listing, exact
seen/read/mark-unread/archive state APIs, exact unread counting, and bounded
exact-recipient reconciliation. Channel delivery remains a later workflow.

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
- recheck recipient privacy and source target authorization at inbox open/list
  time;
- list exact-recipient inbox rows through bounded sparse keyset pages;
- apply exact-item seen/read/mark-unread/archive transitions with terminal archive;
- count exact-recipient unread owner state without deriving totals from list pages;
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
- `NotificationInboxStateService` and exact state request/decision/snapshot contracts;
- `NotificationInboxUnreadCountService` and exact count request/result contracts;
- `NotificationInboxReconcileService` and bounded reconciliation request/page contracts;
- `rustok_notifications::api`, `entities`, `model`, and `migrations`.

## Persistence

The owner exposes five ordered PostgreSQL/SQLite migrations:

1. `m20260721_000010_create_notification_persistence`;
2. `m20260722_000011_create_notification_source_inbox`;
3. `m20260722_000012_add_candidate_processing`;
4. `m20260723_000013_add_outbox_intake_receipts`;
5. `m20260723_000014_add_outbox_intake_rejections`.

Accepted outbox envelopes receive a durable receipt linked to the semantic source
inbox row. Permanently invalid envelopes receive an owner-local quarantine row;
retryable failures receive no terminal record. Accepted and rejected outcomes are
mutually exclusive. The schema stores no source-private payload, rendered HTML,
email address, phone number, or plaintext push endpoint.

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
`tests/inbox_state_sqlite.rs`. Exact-item mark-unread is delivered; bulk/mark-all
mutations and grouped inbox views, transport adapters, and UI remain closed.

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
identity. SQLite evidence is `tests/inbox_count_sqlite.rs`. External transport and
UI exposure remain closed until an authorized adapter composes this owner read.

### 7. Bounded inbox reconciliation

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
- grouping and moderator-directory expansion;
- bulk/mark-all mutations and grouped inbox views;
- tenant-wide scheduled reconciliation and payload redaction;
- external inbox transport adapters and module-owned UI;
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
