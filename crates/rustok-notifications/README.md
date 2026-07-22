# `rustok-notifications`

## Purpose

`rustok-notifications` owns notification inbox state, recipient preferences,
bounded fan-out, grouping, digests, retention, and channel delivery attempts.
The current implementation provides the neutral source boundary, optional runtime
composition, source-provider discovery, and the first owner persistence
foundation. Durable consumption and product APIs follow in later tasks.

## Responsibilities

- consume committed semantic source events outside producer transactions;
- materialize producer-owned `NotificationSourceProviderFactory` registrations
  after the executable host has a neutral `HostRuntimeContext`;
- own tenant/user-scoped notification, delivery, fan-out, preference, digest, and
  encrypted push-subscription storage;
- resolve candidate recipients in bounded cursor pages;
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
- `rustok_notifications::api` re-export of the neutral source contract
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Persistence foundation

`m20260721_000010_create_notification_persistence` creates PostgreSQL and SQLite
storage for notifications, delivery attempts, fan-out jobs/items, preferences,
digest jobs/items, and push subscriptions.

The database enforces tenant-composite recipient integrity, source-event and
idempotency dedupe, typed state/channel/mode values, read-implies-seen semantics,
lease/completion timestamps, bounded JSON/error/cursor fields, and encrypted push
endpoint storage. No email address, phone number, rendered HTML, raw source
payload, or plaintext push endpoint is persisted.

The migration is exposed through `NotificationsModule::migrations`. Global
`rustok-migrations` server composition remains a verification-gated follow-up to
this module-local schema slice.

## Interactions

Producer modules depend on `rustok-notifications-api`, publish semantic outbox
events, and register deferred source factories through runtime extensions. The
server materializes those factories only after database-backed host services are
available. Delivery and identity/contact providers remain separate owner
capabilities.

The first live producer is Forum for `forum.topic.created`. Forum reads its own
event journal, resolves category watchers in bounded pages, and reauthorizes the
current public target. Forum commands continue to succeed when the notifications
owner is tenant-disabled or absent.

The module is compiled into the selected distribution but is not in
`settings.default_enabled`; tenant composition therefore remains notifications-
off by default. The bootstrap admin/storefront packages still expose only
foundation or unavailable states until inbox APIs exist.

## Documentation

- [Live module contract](docs/README.md)
- [Module-local implementation gates](docs/implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
