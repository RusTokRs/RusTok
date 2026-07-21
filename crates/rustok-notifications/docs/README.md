# `rustok-notifications` live contract

## Ownership

Notifications owns inbox rows, unread/read state, preferences, bounded fan-out,
grouping, digests, retention, delivery attempts, and replay/reconciliation.
Source modules own semantic event state, audience facts, subscriptions,
visibility, target authorization, and target routes. Delivery modules own
channel-specific transport.

## Current runtime boundary

The current slice provides a neutral source registry in
`rustok-notifications-api` and an optional `NotificationsModule` that initializes
that registry. Providers describe semantic events, resolve bounded candidate
audiences, and authorize one recipient opening one target.

No source payload, rendered HTML, contact address, storage credential, or source
database model crosses the contract. Provider absence is a healthy empty state.
Producer transactions remain independent from notification availability.

## Pending runtime capabilities

- persistence and preference schema;
- durable outbox consumption and consumer inbox;
- bounded leased fan-out jobs;
- target-open authorization integration;
- inbox GraphQL/REST APIs;
- delivery-provider SPI and attempts;
- module-owned admin/storefront product surfaces.

The canonical program status and task numbering remain in
`crates/rustok-forum/docs/implementation-plan.md`.
