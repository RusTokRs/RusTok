# rustok-notifications / CRATE_API

## Public types

- `NotificationsModule`
- `NotificationsService`
- `NotificationFanoutService`
- `NotificationSourceInboxReceipt`
- `NotificationFanoutPageResult`
- `NotificationOutboxIntakeWorker`
- `NotificationOutboxIntakeResult`
- `NotificationOutboxIntakeBatchResult`
- `NotificationOutboxIntakeFailure`
- `DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE`
- `MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE`
- `NotificationCandidateService`
- `NotificationCandidateProcessResult`
- `NotificationCandidateWorker`
- `NotificationCandidateBatchResult`
- `NotificationCandidateWorkerFailure`
- `DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE`
- `MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE`
- `NotificationRecipientPolicy`
- `NotificationRecipientPolicyRequest`
- `NotificationRecipientPolicyDecision`
- `NotificationRecipientPolicyError`
- `NotificationRecipientSuppression`
- `NotificationRecipientPolicyRuntime`
- `NotificationBlockReadPort` / `NotificationBlockReadRuntime`
- `NotificationMuteReadPort` / `NotificationMuteReadRuntime`
- `rustok_notifications::error::{NotificationError, NotificationResult}`
- `rustok_notifications::api`
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Module contract

`NotificationsModule` is an optional module with an `outbox` dependency. Runtime
registration guarantees that a `NotificationSourceRegistry` exists without
requiring any producer source to be installed.

The module owns four ordered PostgreSQL/SQLite migrations:

- `m20260721_000010_create_notification_persistence`, after the platform users
  migration;
- `m20260722_000011_create_notification_source_inbox`, after the notification
  persistence migration;
- `m20260722_000012_add_candidate_processing`, after the durable source inbox;
- `m20260723_000013_add_outbox_intake_receipts`, after candidate processing.

The module-local `MigrationSource` is authoritative for this schema. Global
server migrator composition remains a separate verification-gated follow-up.

## Persistence contract

The schema owns:

- recipient notification rows and unread/seen/read/archive timestamps;
- channel delivery attempts with lease, retry, terminal, and provider receipt
  metadata;
- bounded fan-out jobs/items;
- a durable source-event inbox with typed retry/lease/terminal state;
- durable outbox-intake receipts keyed by the committed outbox envelope ID;
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
is a retryable materialization state after durable acceptance. An outbox intake
receipt separately binds one committed `sys_events.id` to the accepted source
identity and source-inbox row. Source-inbox acceptance and receipt creation share
one transaction.

Template data must be a JSON object of at most 8 KiB. Fan-out descriptors must be
a JSON object of at most 16 KiB. Audience cursors are bounded to 512 bytes and a
page is capped at 256 unique recipients. Error codes/messages and worker IDs are
bounded. The schema does not store source-private payloads, rendered HTML, email
addresses, phone numbers, or plaintext push endpoints.

## Service contract

`NotificationsService::from_runtime_extensions` reads the neutral source
registry and safely falls back to an empty registry. It exposes source metadata,
source lookup, source count, and source availability.

`NotificationOutboxIntakeWorker` is the owner-side durable consumer for supported
committed outbox envelopes. Its constructor accepts a batch size from 1 through
64; the canonical host uses 32. Selection is stable by `created_at`, then `id`,
and excludes rows that already have an intake receipt. General relay status is
not read or mutated: pending, dispatched, and failed relay rows are all eligible
until Notifications records its own receipt.

The current intake mapping is explicit and bounded:

- root `forum.topic.created` uses source `forum`, source event ID `topic_id`, and
  semantic revision `1`;
- sealed `forum.mention.user_added` uses the envelope ID and persisted
  `source_revision_id`.

Envelope validation, source-inbox insert/replay validation, and intake-receipt
insert/replay validation fail closed. The owner reads only the public outbox
envelope and does not call Forum services or query Forum-owned tables.

`NotificationFanoutService` performs durable source acceptance, descriptor
materialization, and bounded cursor fan-out into idempotent pending candidates.
It creates neither final notification rows nor delivery attempts. Automatic
source-inbox materialization and page processing remain a separate worker slice.

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

`NotificationCandidateWorker` is the owner-side bounded driver used by executable
hosts. Its constructor accepts a batch size from 1 through 64; the canonical
host uses 32. `claimable_candidate_ids` selects one stable page ordered by
`created_at`, then `id`, but does not acquire leases. Each ID must be passed to
`process_candidate`, which preserves the existing per-item lease CAS. This split
allows deployment-owned shutdown handling to stop between candidates without
moving private-table queries into the host.

`NotificationRecipientPolicyRuntime` separates relation-port readiness from
candidate-worker enablement. The server composes Profiles and Social Graph owner
ports and enables the worker only when the explicit
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED` flag is true. Missing or invalid
enablement remains false. Outbox intake has an independent default-off flag,
`RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`. Target visibility and privacy must
still be rechecked when opening an inbox item and before delayed delivery.

## Cross-module contract

All source-provider types are re-exported under `rustok_notifications::api` from
`rustok-notifications-api`. Producer modules register providers through runtime
extensions and do not import this owner crate. Producer commands never call the
notifications service synchronously.

Forum publishes `forum.topic.created` and `forum.mention.user_added` provider
contracts. Its provider accepts both legacy journal UUID/sequence references and
the semantic source identities derived from the committed outbox envelopes. User
mention resolution still requires the exact immutable relation row and rechecks
current source visibility at describe/audience/open time. Pending replies are
retryable; deleted, hidden, closed, self-mentioned, or channel-restricted sources
fail closed. Profile/block privacy is supplied through the recipient policy port
rather than by reading Profiles-owned private tables.
