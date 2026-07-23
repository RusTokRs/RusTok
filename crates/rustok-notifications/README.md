# `rustok-notifications`

## Purpose

`rustok-notifications` owns notification inbox state, recipient preferences,
bounded fan-out, grouping, digests, retention, and channel delivery attempts.
The current implementation provides the neutral source boundary, optional runtime
composition, durable outbox intake, durable source state, bounded candidate
fan-out, and a policy-gated command that can create one final in-app
notification. Channel delivery remains a separate later workflow.

## Responsibilities

- consume committed semantic source events outside producer transactions;
- materialize producer-owned `NotificationSourceProviderFactory` registrations
  after the executable host has a neutral `HostRuntimeContext`;
- own tenant/user-scoped notification, delivery, fan-out, preference, digest, and
  encrypted push-subscription storage;
- accept source events idempotently and retain them while a source provider is
  temporarily unavailable;
- resolve candidate recipients in bounded cursor pages under recoverable leases;
- apply notification preferences, injected recipient privacy policy, and current
  source authorization before creating an inbox row;
- own retention, replay, reconciliation, and delivery-attempt lifecycle.

## Non-responsibilities

- producer subscriptions and source lifecycle;
- SMTP, push-vendor, or SMS SDK implementation;
- authentication identity and contact data;
- source-private tables or Profiles/Social Graph persistence;
- synchronous notification calls inside producer transactions.

## Entry points

- `NotificationsModule`
- `NotificationsService`
- `NotificationOutboxIntakeWorker`
- `NotificationFanoutService`
- `NotificationCandidateService`
- `NotificationCandidateWorker`
- `NotificationRecipientPolicy`
- `NotificationRecipientPolicyRuntime`
- `NotificationSourceInboxReceipt`
- `NotificationOutboxIntakeResult`
- `NotificationFanoutPageResult`
- `NotificationCandidateProcessResult`
- `rustok_notifications::api` re-export of the neutral source contract
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Persistence foundation

`m20260721_000010_create_notification_persistence` creates PostgreSQL and SQLite
storage for notifications, delivery attempts, fan-out jobs/items, preferences,
digest jobs/items, and push subscriptions.

`m20260722_000011_create_notification_source_inbox` adds a durable source-event
inbox deduplicated by tenant, source slug, and source event ID.

`m20260722_000012_add_candidate_processing` adds processing/retry leases to
fan-out candidates, typed retryable/terminal states, and recovery indexes while
preserving SQLite tenant-integrity triggers.

`m20260723_000013_add_outbox_intake_receipts` binds each accepted committed
outbox envelope to its semantic source identity and durable source-inbox row.

The database enforces tenant-composite recipient integrity, source-event and
idempotency dedupe, typed state/channel/mode values, read-implies-seen semantics,
lease/completion timestamps, bounded JSON/error/cursor fields, and encrypted push
endpoint storage. No email address, phone number, rendered HTML, raw source
payload, or plaintext push endpoint is persisted.

The migrations are exposed through `NotificationsModule::migrations`. Global
`rustok-migrations` server composition remains a verification-gated follow-up.

## Durable outbox, source, and candidate processing

`NotificationOutboxIntakeWorker` selects supported committed `sys_events` rows
without an intake receipt and accepts them into the source inbox. Selection is
bounded to 32 by default and 64 maximum, ordered by `created_at/id`, and does not
read or mutate general relay status. Source inbox and receipt commit in one
transaction. The current mappings are root `forum.topic.created → topic_id/1`
and sealed `forum.mention.user_added → envelope_id/source_revision_id`.

`NotificationFanoutService` separates source processing into durable event
acceptance, descriptor materialization, and bounded cursor fan-out. Its output is
an idempotent set of `pending` candidates, not inbox rows. A production worker
that leases pending source rows and drives materialization/pages remains open.

`NotificationCandidateService` requires an explicitly injected
`NotificationRecipientPolicy`; there is no allow-all default. For one candidate it:

1. claims a recoverable candidate lease;
2. resolves exact source/type preferences before wildcard preferences;
3. evaluates the injected recipient/profile/block/mute policy;
4. reauthorizes the current source target for that recipient;
5. rechecks the preference in the final transaction;
6. inserts or validates one deduplicated in-app notification and completes the
   candidate under the same lease CAS.

Disabled preferences, privacy suppression, and unavailable targets become stable
`skipped` outcomes. Retryable policy/provider failures retain retry state.
Changed semantic replay fails closed. No channel delivery attempt is created by
this workflow.

The server composes Profiles privacy and Social Graph block/mute owner ports into
the production policy runtime. Outbox intake and candidate processing have
separate default-off runtime flags:

- `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`;
- `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`.

Privacy and source visibility must be checked again when an inbox item is opened
and before delayed delivery.

## Interactions

Producer modules depend on `rustok-notifications-api`, publish semantic outbox
events, and register deferred source factories through runtime extensions. The
server materializes those factories only after database-backed host services are
available. Delivery and identity/contact providers remain separate owner
capabilities.

Forum supports `forum.topic.created` and `forum.mention.user_added`. Its provider
accepts legacy journal identity/revision references and semantic identities from
the committed outbox envelopes. The user mention provider still binds the event
to the exact immutable `forum_user_mentions` row, rechecks current topic/reply
visibility, defers pending replies, suppresses self-mentions, and fails closed for
deleted, hidden, closed, or channel-restricted sources. Moderator audience
expansion remains deferred until a bounded owner directory port exists.

Forum commands continue to succeed when the notifications owner is tenant-
disabled or absent. Notifications remains outside `settings.default_enabled`.
Admin/storefront packages still expose only foundation or unavailable states
until inbox APIs exist.

## Documentation

- [Live module contract](docs/README.md)
- [Module-local implementation gates](docs/implementation-plan.md)
- [Outbox intake machine contract](contracts/notifications-outbox-intake.json)
- [Candidate worker machine contract](contracts/notifications-candidate-worker.json)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
