# `rustok-notifications`

## Purpose

`rustok-notifications` owns notification inbox state, recipient preferences,
bounded fan-out, grouping, digests, retention, and channel delivery attempts.
The current foundation publishes the owner boundary, optional runtime
composition, and source-provider discovery. Persistence and durable delivery
execution follow in later tasks.

## Responsibilities

- consume committed semantic source events outside producer transactions;
- materialize producer-owned `NotificationSourceProviderFactory` registrations
  after the executable host has a neutral `HostRuntimeContext`;
- resolve candidate recipients in bounded cursor pages;
- apply notification preferences, privacy, visibility, blocks, and delivery
  policy before creating inbox or channel work;
- own notification rows, delivery attempts, fan-out jobs, digest jobs, retention,
  replay, and reconciliation.

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

## Interactions

Producer modules depend on `rustok-notifications-api`, publish semantic outbox
events, and register deferred source factories through runtime extensions. The
server materializes those factories only after database-backed host services are
available. `rustok-notifications` consumes the resulting providers after
producer commits. Delivery and identity/contact providers remain separate owner
capabilities.

The first live producer is Forum for `forum.topic.created`. Forum reads its own
event journal, resolves category watchers in bounded pages, and reauthorizes the
current public target. Forum commands continue to succeed when the notifications
owner is tenant-disabled or absent.

The module is compiled into the selected distribution but is not in
`settings.default_enabled`; tenant composition therefore remains notifications-
off by default. With no source providers, the owner exposes a healthy empty
registry. The bootstrap admin/storefront packages still expose only foundation
or unavailable state until inbox persistence exists.

## Documentation

- [Live module contract](docs/README.md)
- [Module-local implementation gates](docs/implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
