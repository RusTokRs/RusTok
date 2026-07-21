# rustok-notifications / CRATE_API

## Public types

- `NotificationsModule`
- `NotificationsService`
- `rustok_notifications::api`

## Module contract

`NotificationsModule` is an optional module with an `outbox` dependency. Runtime
registration guarantees that a `NotificationSourceRegistry` exists without
requiring any producer source to be installed. This slice owns no database
migrations.

## Service contract

`NotificationsService::from_runtime_extensions` reads the neutral source
registry and safely falls back to an empty registry. It exposes source metadata,
source lookup, source count, and source availability only. Inbox, fan-out,
preference, digest, and delivery methods are intentionally absent until their
owner persistence exists.

## Cross-module contract

All source-provider types are re-exported under `rustok_notifications::api` from
`rustok-notifications-api`. Producer modules register providers through runtime
extensions and do not import this owner crate. Producer commands never call the
notifications service synchronously.
