# rustok-notifications / CRATE_API

## Public types

- `NotificationsModule`
- `NotificationsService`
- `NotificationFanoutService`
- `NotificationSourceInboxReceipt`
- `NotificationFanoutPageResult`
- `NotificationCandidateService`
- `NotificationCandidateProcessResult`
- `NotificationRecipientPolicy`
- `NotificationRecipientPolicyRequest`
- `NotificationRecipientPolicyDecision`
- `NotificationRecipientPolicyError`
- `NotificationRecipientSuppression`
- `rustok_notifications::error::{NotificationError, NotificationResult}`
- `rustok_notifications::api`
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Module contract

`NotificationsModule` is an optional module with an `outbox` dependency. Runtime
registration guarantees that a `NotificationSourceRegistry` exists without
requiring any producer source to be installed.

The module owns three ordered PostgreSQL/SQLite migrations:

- `m20260721_000010_create_notification_persistence`, after the platform users
  migration;
- `m20260722_000011_create_notification_source_inbox`, after the notification
  persistence migration;
- `m20260722_000012_add_candidate_processing`, after the durable source inbox.

The module-local `MigrationSource` is authoritative for this schema. Global
server migrator composition remains a separate verification-gated follow-up.

## Persistence contract

The schema owns:

- recipient notification rows and unread/seen/read/archive timestamps;
- channel delivery attempts with lease, retry, terminal, and provider receipt
  metadata;
- bounded fan-out jobs/items;
- a durable source-event inbox with typed retry/lease/terminal state;
- leased candidate-processing state with retry timing and terminal suppression;
- source/type-scoped recipient preferences;
- digest jobs/items;
- encrypted push subscription material.

Every recipient/user relation uses `(tenant_id, user_id)` or
`(tenant_id, recipient_id)` integrity against `users(tenant_id, id)`. Child rows
use tenant-composite parent keys where deletion semantics permit it. Optional
actor and fan-out notification references use database triggers to reject tenant
mismatch.

Notification rows deduplicate by tenant, recipient, source slug, source event ID,
and notification type. Candidate rows are leased before policy evaluation;
expired workers cannot complete the row. Processing, retryable, processed,
skipped, and failed states are database-constrained, and final notification
creation plus candidate completion share one transaction.

The source inbox deduplicates by tenant, source slug, and source event ID. The
persisted event type and source revision must match on replay. Provider absence
is a retryable materialization state after durable acceptance.

Template data must be a JSON object of at most 8 KiB. Fan-out descriptors must be
a JSON object of at most 16 KiB. Audience cursors are bounded to 512 bytes and a
page is capped at 256 unique recipients. Error codes/messages and worker IDs are
bounded. The schema does not store source-private payloads, rendered HTML, email
addresses, phone numbers, or plaintext push endpoints.

## Service contract

`NotificationsService::from_runtime_extensions` reads the neutral source
registry and safely falls back to an empty registry. It exposes source metadata,
source lookup, source count, and source availability.

`NotificationFanoutService` performs durable source acceptance, descriptor
materialization, and bounded cursor fan-out into idempotent pending candidates.
It creates neither final notification rows nor delivery attempts.

`NotificationCandidateService::new(db, registry, policy)` requires an explicit
`NotificationRecipientPolicy`; no permissive default exists.
`process_candidate(item_id, worker_id)`:

1. leases a pending, retryable, or expired-processing candidate;
2. resolves notification preferences using exact source/type precedence over
   wildcard scopes;
3. invokes the injected recipient privacy policy for profile, block, mute,
   recipient, and tenant decisions;
4. invokes the source provider's recipient-specific `authorize_target_open`;
5. rechecks preferences inside the final database transaction;
6. inserts or validates one idempotent in-app notification and completes the
   candidate under lease CAS in that same transaction.

A disabled preference, privacy suppression, or unavailable source target marks
the candidate `skipped`. Retryable policy/provider/database failures clear the
lease and retain retry metadata. Changed semantic identity fails closed. This
service does not create channel delivery attempts or perform provider calls
inside the final notification transaction.

The production profile/block implementation of `NotificationRecipientPolicy`
remains a separate composition slice. Target visibility and privacy must be
rechecked again when opening an inbox item and before delayed delivery.

## Cross-module contract

All source-provider types are re-exported under `rustok_notifications::api` from
`rustok-notifications-api`. Producer modules register providers through runtime
extensions and do not import this owner crate. Producer commands never call the
notifications service synchronously.

Forum publishes `forum.topic.created` and `forum.mention.user_added` provider
contracts. User mention resolution requires the exact immutable relation row and
rechecks current source visibility at describe/audience/open time. Pending replies
are retryable; deleted, hidden, closed, self-mentioned, or channel-restricted
sources fail closed. Profile/block privacy is supplied through the recipient
policy port rather than by reading Profiles-owned private tables.
