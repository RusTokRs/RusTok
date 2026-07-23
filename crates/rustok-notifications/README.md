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
- enforce effective tenant capability before source provider calls;
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
- `NotificationCandidateService` / `NotificationCandidateWorker`;
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
unresolved tenant policy fails closed before any producer provider call. The host
loop is default-off behind `RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`.

Fanout creates only idempotent pending candidates—never final notifications or
delivery attempts.

### 3. Candidate policy

`NotificationCandidateService` claims a candidate, resolves exact preference
scopes before wildcards, evaluates Profiles/Social Graph recipient policy,
reauthorizes the source target, rechecks preferences inside the final transaction,
and inserts or validates one in-app notification under the same lease CAS.

`NotificationCandidateWorker` is default-off behind
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`. It requires a materialized source
registry and ready recipient-policy ports. Candidate finalization creates no
channel delivery attempt.

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

- durable backoff for work belonging to disabled tenants;
- PostgreSQL contention/recovery evidence and worker health/lag metrics;
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
