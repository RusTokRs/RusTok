# `rustok-notifications`

## Purpose

`rustok-notifications` owns inbox state, recipient preferences, bounded fanout,
grouping, digests, retention, and delivery-attempt lifecycle. The implemented
pipeline now covers durable outbox intake, source materialization, bounded
audience expansion, recipient policy, and one idempotent in-app notification.
Channel delivery remains a later workflow.

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
- PostgreSQL cursor/lease contention evidence and worker health/lag metrics;
- grouping and moderator-directory expansion;
- inbox APIs and open-time privacy/source rechecks;
- channel delivery enqueue after candidate acceptance;
- retention, reconciliation, quarantine replay/purge, and module-owned UI.

## Documentation

- [Live contract](docs/README.md)
- [Implementation gates](docs/implementation-plan.md)
- [Outbox intake contract](contracts/notifications-outbox-intake.json)
- [Fanout worker contract](contracts/notifications-fanout-worker.json)
- [Candidate worker contract](contracts/notifications-candidate-worker.json)
- Canonical cross-module roadmap:
  `crates/rustok-forum/docs/implementation-plan.md`
