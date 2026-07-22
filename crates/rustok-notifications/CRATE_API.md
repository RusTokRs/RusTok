# rustok-notifications / CRATE_API

## Public types

- `NotificationsModule`
- `NotificationsService`
- `rustok_notifications::api`
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Module contract

`NotificationsModule` is an optional module with an `outbox` dependency. Runtime
registration guarantees that a `NotificationSourceRegistry` exists without
requiring any producer source to be installed.

The module now owns one PostgreSQL/SQLite migration,
`m20260721_000010_create_notification_persistence`, ordered after the platform
users migration. The module-local `MigrationSource` is authoritative for this
schema. Global server migrator composition remains a separate verification-gated
follow-up.

## Persistence contract

The schema owns:

- recipient notification rows and unread/seen/read/archive timestamps;
- channel delivery attempts with lease, retry, terminal, and provider receipt
  metadata;
- bounded fan-out jobs/items;
- source/type-scoped recipient preferences;
- digest jobs/items;
- encrypted push subscription material.

Every recipient/user relation uses `(tenant_id, user_id)` or
`(tenant_id, recipient_id)` integrity against `users(tenant_id, id)`. Child rows
use tenant-composite parent keys where deletion semantics permit it. Optional
actor and fan-out notification references use database triggers to reject tenant
mismatch.

Notification rows deduplicate at minimum by tenant, recipient, source slug,
source event ID, and notification type. Separate tenant-scoped idempotency keys
cover notifications, delivery attempts, fan-out items, and digest items.

Statuses, priorities, channels, delivery modes, digest modes, push platforms, and
push lifecycle values are Rust `DeriveActiveEnum` types and database `CHECK`
values. Read timestamps require seen timestamps. Leased work requires an owner
and expiry; completed/sent rows require matching completion timestamps.

Template data must be a JSON object of at most 8 KiB. Fan-out descriptors must be
a JSON object of at most 16 KiB. Error codes/messages and cursors are bounded.
The schema does not store source-private payloads, rendered HTML, email addresses,
phone numbers, or plaintext push endpoints. Push endpoint/key material is stored
only in encrypted columns plus a normalized endpoint hash and encryption key
version.

## Service contract

`NotificationsService::from_runtime_extensions` reads the neutral source
registry and safely falls back to an empty registry. It exposes source metadata,
source lookup, source count, and source availability only. Transactional inbox,
fan-out, preference, digest, and delivery workflow methods remain intentionally
absent until later `NOTIFY-01`/`NOTIFY-03` slices define their command semantics.

## Cross-module contract

All source-provider types are re-exported under `rustok_notifications::api` from
`rustok-notifications-api`. Producer modules register providers through runtime
extensions and do not import this owner crate. Producer commands never call the
notifications service synchronously.
