# rustok-notifications / CRATE_API

## Public types

- `NotificationsModule`
- `NotificationsService`
- `NotificationFanoutService`
- `NotificationSourceInboxReceipt`
- `NotificationFanoutPageResult`
- `rustok_notifications::error::{NotificationError, NotificationResult}`
- `rustok_notifications::api`
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Module contract

`NotificationsModule` is an optional module with an `outbox` dependency. Runtime
registration guarantees that a `NotificationSourceRegistry` exists without
requiring any producer source to be installed.

The module owns two ordered PostgreSQL/SQLite migrations:

- `m20260721_000010_create_notification_persistence`, after the platform users
  migration;
- `m20260722_000011_create_notification_source_inbox`, after the notification
  persistence migration.

The module-local `MigrationSource` is authoritative for this schema. Global
server migrator composition remains a separate verification-gated follow-up.

## Persistence contract

The schema owns:

- recipient notification rows and unread/seen/read/archive timestamps;
- channel delivery attempts with lease, retry, terminal, and provider receipt
  metadata;
- bounded fan-out jobs/items;
- a durable source-event inbox with typed retry/lease/terminal state;
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

The source inbox deduplicates by tenant, source slug, and source event ID. The
persisted event type and source revision must match on replay. Provider absence
is a retryable materialization state after durable acceptance. Processing leases
expire and can be reclaimed. Completed, suppressed, and rejected rows are
terminal and carry a completion timestamp.

Statuses, priorities, channels, delivery modes, digest modes, push platforms, and
push lifecycle values are Rust `DeriveActiveEnum` types and database `CHECK`
values. Read timestamps require seen timestamps. Leased work requires an owner
and expiry; completed/sent rows require matching completion timestamps.

Template data must be a JSON object of at most 8 KiB. Fan-out descriptors must be
a JSON object of at most 16 KiB. Audience cursors are bounded to 512 bytes and a
page is capped at 256 unique recipients. Error codes/messages and worker IDs are
bounded. The schema does not store source-private payloads, rendered HTML, email
addresses, phone numbers, or plaintext push endpoints.

## Service contract

`NotificationsService::from_runtime_extensions` reads the neutral source
registry and safely falls back to an empty registry. It exposes source metadata,
source lookup, source count, and source availability.

`NotificationFanoutService` exposes the first transactional owner workflow:

- `enqueue_source_event(NotificationSourceEventRef)` durably accepts or replays a
  source event identity;
- `materialize_source_event(inbox_id, worker_id)` leases the inbox row, invokes
  the registered provider, suppresses unavailable source targets, and creates or
  replays one bounded descriptor fan-out job;
- `process_fanout_page(job_id, worker_id, limit)` resolves one cursor page and
  writes idempotent `pending` candidate items before advancing the cursor or
  completing the job.

A changed source revision/type or changed descriptor fails closed. A cursor that
does not advance dead-letters the job. Retryable provider/database failures clear
the lease and retain bounded recovery metadata.

Candidate items are not final notifications. These methods create neither
`notifications` rows nor delivery attempts. Preference, profile/block privacy,
recipient-specific authorization, and channel policy remain mandatory before a
candidate can be processed.

## Cross-module contract

All source-provider types are re-exported under `rustok_notifications::api` from
`rustok-notifications-api`. Producer modules register providers through runtime
extensions and do not import this owner crate. Producer commands never call the
notifications service synchronously.

Forum publishes `forum.topic.created` and `forum.mention.user_added` provider
contracts. User mention resolution requires the exact immutable relation row and
rechecks current source visibility at describe/audience/open time. Pending replies
are retryable; deleted, hidden, closed, self-mentioned, or channel-restricted
sources fail closed. Final profile/block privacy remains downstream policy under
`NOTIFY-07`.
