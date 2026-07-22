# `rustok-notifications`

## Purpose

`rustok-notifications` owns notification inbox state, recipient preferences,
bounded fan-out, grouping, digests, retention, and channel delivery attempts.
The current implementation provides the neutral source boundary, optional runtime
composition, owner persistence, a durable source-event inbox, and bounded
candidate fan-out. Final notification creation remains closed until preference
and privacy policy are implemented.

## Responsibilities

- consume committed semantic source events outside producer transactions;
- materialize producer-owned `NotificationSourceProviderFactory` registrations
  after the executable host has a neutral `HostRuntimeContext`;
- own tenant/user-scoped notification, delivery, fan-out, preference, digest, and
  encrypted push-subscription storage;
- accept source events idempotently and retain them while a source provider is
  temporarily unavailable;
- resolve candidate recipients in bounded cursor pages under recoverable leases;
- apply notification preferences, privacy, visibility, blocks, and delivery
  policy before creating inbox or channel work;
- own retention, replay, reconciliation, and delivery-attempt lifecycle.

## Non-responsibilities

- producer subscriptions and source lifecycle;
- SMTP, push-vendor, or SMS SDK implementation;
- authentication identity and contact data;
- source authorization policy or source-private tables;
- synchronous notification calls inside producer transactions.

## Entry points

- `NotificationsModule`
- `NotificationsService`
- `NotificationFanoutService`
- `NotificationSourceInboxReceipt`
- `NotificationFanoutPageResult`
- `rustok_notifications::api` re-export of the neutral source contract
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Persistence foundation

`m20260721_000010_create_notification_persistence` creates PostgreSQL and SQLite
storage for notifications, delivery attempts, fan-out jobs/items, preferences,
digest jobs/items, and push subscriptions.

`m20260722_000011_create_notification_source_inbox` adds a durable source-event
inbox deduplicated by tenant, source slug, and source event ID. A replay with a
changed event type or source revision conflicts. Processing, retry, suppression,
rejection, completion, and expired-lease recovery are explicit persisted states.
A completed source inbox row retains its linked fan-out job.

The database enforces tenant-composite recipient integrity, source-event and
idempotency dedupe, typed state/channel/mode values, read-implies-seen semantics,
lease/completion timestamps, bounded JSON/error/cursor fields, and encrypted push
endpoint storage. No email address, phone number, rendered HTML, raw source
payload, or plaintext push endpoint is persisted.

The migrations are exposed through `NotificationsModule::migrations`. Global
`rustok-migrations` server composition remains a verification-gated follow-up.

## Durable candidate fan-out

`NotificationFanoutService` separates source processing into explicit phases:

1. `enqueue_source_event` durably accepts the typed source identity without
   requiring the provider to be currently available;
2. `materialize_source_event` leases the inbox row, invokes the registered source
   provider, stores the bounded semantic descriptor, and creates or replays one
   fan-out job;
3. `process_fanout_page` leases that job, resolves one audience page capped at
   256 recipients, persists idempotent `pending` candidate items, and advances the
   cursor or completes the job.

Candidate items are not notifications. This slice deliberately creates no
`notifications` rows and no delivery attempts. Preference, profile/block privacy,
recipient-specific source authorization, and channel policy must process each
candidate first.

## Interactions

Producer modules depend on `rustok-notifications-api`, publish semantic outbox
events, and register deferred source factories through runtime extensions. The
server materializes those factories only after database-backed host services are
available. Delivery and identity/contact providers remain separate owner
capabilities.

Forum supports `forum.topic.created` and `forum.mention.user_added`. The user
mention provider binds the event to the exact immutable `forum_user_mentions`
row, rechecks current topic/reply visibility before describing and resolving the
single recipient, defers pending replies, suppresses self-mentions, and fails
closed for deleted, hidden, closed, or channel-restricted sources. Moderator
audience expansion remains deferred until a bounded owner directory port exists.

Forum commands continue to succeed when the notifications owner is tenant-
disabled or absent. The module is compiled into the selected distribution but is
not in `settings.default_enabled`; tenant composition therefore remains
notifications-off by default. Admin/storefront packages still expose only
foundation or unavailable states until inbox APIs exist.

## Documentation

- [Live module contract](docs/README.md)
- [Module-local implementation gates](docs/implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
