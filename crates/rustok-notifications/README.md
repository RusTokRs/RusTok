# `rustok-notifications`

## Purpose

`rustok-notifications` is the owner of notification inbox state, recipient
preferences, bounded fan-out, grouping, digests, retention, and channel delivery
attempts. This initial foundation publishes the owner boundary and source
registry; persistence and delivery execution follow in later slices.

## Responsibilities

- consume committed semantic source events outside producer transactions;
- discover producer-owned `NotificationSourceProvider` implementations;
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
events, and register optional source providers through runtime extensions.
`rustok-notifications` consumes those contracts after producer commits. Delivery
providers and identity/contact providers remain separate owner capabilities.

With no source providers registered, the module initializes an empty registry
and remains healthy. With the module absent, producers still commit owner state
and semantic outbox events. The bootstrap admin/storefront packages expose only
foundation/unavailable states until inbox persistence exists.

## Documentation

- [Live module contract](docs/README.md)
- [Module-local implementation gates](docs/implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
