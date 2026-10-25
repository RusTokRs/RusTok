# rustok-notifications / CRATE_API

## Public types

### Module and source registry

- `NotificationsModule`
- `NotificationsService`
- `rustok_notifications::api`

### Durable outbox intake

- `NotificationOutboxEnvelopeDecoder`
- `NotificationOutboxEnvelopeRecord`
- `NotificationOutboxIntakeWorker`
- `NotificationOutboxIntakeOutcome`
- `NotificationOutboxIntakeResult`
- `NotificationOutboxIntakeRejection`
- `NotificationOutboxIntakeBatchResult`
- `NotificationOutboxIntakeFailure`
- `DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE`
- `MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE`

### Source materialization and fanout

- `NotificationFanoutService`
- `NotificationSourceInboxReceipt`
- `NotificationFanoutPageResult`
- `NotificationFanoutWorker`
- `NotificationFanoutSourceWorkItem`
- `NotificationFanoutJobWorkItem`
- `NotificationFanoutPolicyDeferral`
- `NotificationFanoutWorkerBatchResult`
- `NotificationFanoutWorkerFailure`
- `NotificationFanoutWorkerStage`
- `DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE`
- `MAX_NOTIFICATION_FANOUT_BATCH_SIZE`
- `DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE`
- `MAX_NOTIFICATION_FANOUT_PAGE_SIZE`

### Candidate policy

- `NotificationCandidateService`
- `NotificationCandidateProcessResult`
- `NotificationCandidateWorker`
- `NotificationCandidateWorkItem`
- `NotificationCandidatePolicyDeferral`
- `NotificationCandidateBatchResult`
- `NotificationCandidateWorkerFailure`
- `DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE`
- `MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE`
- `NotificationRecipientPolicy`
- `NotificationRecipientPolicyRequest`
- `NotificationRecipientPolicyDecision`
- `NotificationRecipientPolicyError`
- `NotificationRecipientSuppression`
- `NotificationTenantCapabilityCommitGuard`
- `NotificationTenantCapabilityCommitRequest`
- `NotificationTenantCapabilityCommitDecision`
- `NotificationTenantCapabilityCommitError`
- `NotificationRecipientPolicyRuntime`
- `NotificationBlockReadPort` / `NotificationBlockReadRuntime`
- `NotificationMuteReadPort` / `NotificationMuteReadRuntime`

### Persistence

- `rustok_notifications::error::{NotificationError, NotificationResult}`
- `rustok_notifications::entities`
- `rustok_notifications::model`
- `rustok_notifications::migrations`

## Module contract

`NotificationsModule` is optional and declares an `outbox` capability dependency.
Runtime registration guarantees a `NotificationSourceRegistry` exists even when
no producer source is installed.

The owner exposes five ordered PostgreSQL/SQLite migrations:

1. `m20260721_000010_create_notification_persistence`;
2. `m20260722_000011_create_notification_source_inbox`;
3. `m20260722_000012_add_candidate_processing`;
4. `m20260723_000013_add_outbox_intake_receipts`;
5. `m20260723_000014_add_outbox_intake_rejections`.

The module-local `MigrationSource` is authoritative for this schema. Global server
migrator composition remains a separate maintainer verification gate.

## Intake contract

`NotificationOutboxIntakeWorker::new(db, decoder, batch_size)` requires an
executable-host decoder and a batch size from 1 through 64; the canonical host
uses 32. The worker reads a minimal public projection of `sys_events`, selects in
stable `created_at/id` order, and ignores general relay status.

The owner does not depend directly on platform event or outbox crates. The server
decoder validates envelope metadata/schema and maps supported envelopes into
`NotificationSourceEventRef` values.

Accepted source inbox state and its receipt commit in one transaction. Permanent
invalid envelopes enter `notification_outbox_intake_rejections`; retryable errors
receive no terminal record. Receipts and rejections are both excluded from later
selection and are database-enforced mutually exclusive outcomes. Accepted replay
re-decodes the current envelope and must match the persisted semantic source
identity.

## Fanout contract

`NotificationFanoutService` remains the only owner authority for source and job
leases, descriptor materialization, audience resolution, cursor advancement,
candidate persistence, and durable failure transitions.

`NotificationFanoutWorker::new(db, registry, worker_id, batch_size, page_size)`:

- accepts batch sizes 1–64 and page sizes 1–256;
- exposes tenant-scoped `NotificationFanoutSourceWorkItem` and
  `NotificationFanoutJobWorkItem` values;
- selects pending, due retryable, or expired leased records in stable
  `created_at/id` order;
- acquires no lease during selection;
- delegates enabled work to the canonical service.

Executable hosts establish tenant capability before invoking
`materialize_source_inbox` or `process_fanout_job`. The server uses
`EffectiveModulePolicyService::is_enabled` for module slug `notifications` before
every source and job call.

`NotificationFanoutPolicyDeferral` provides two owner-side CAS transitions before
provider execution:

- `TenantDisabled`: retry after 300 seconds with stable code
  `NOTIFICATION_TENANT_CAPABILITY_DISABLED`;
- `PolicyUnavailable`: retry after 30 seconds with stable code
  `NOTIFICATION_TENANT_POLICY_UNAVAILABLE`.

Both transitions set `retryable_error`, increment attempt count, clear lease
fields, persist bounded error metadata, and set future `next_attempt_at`. The CAS
requires the same tenant, record ID, prior attempt count, and current claimable
state; a concurrent canonical claim is never overwritten. This removes disabled
or unresolved tenant rows from the bounded queue head without calling a producer
provider.

Fanout creates pending candidates only. It creates neither final notification rows
nor delivery attempts.

## Candidate contract

`NotificationCandidateWorker::claimable_candidate_work` returns bounded
`NotificationCandidateWorkItem { item_id, tenant_id }` values in stable
`created_at/id` order without acquiring a lease. The legacy
`claimable_candidate_ids` remains a compatibility projection for trusted callers.

Before claim, the server calls `EffectiveModulePolicyService::resolve_snapshot`,
requires the `notifications` capability, and observes one coherent token containing
`ModuleEffectivePolicy::policy_revision` plus the exact manifest default-enabled
module set used for that computation. Disabled or unresolved work does not invoke
recipient privacy policy or a source provider; it receives the existing
300/30-second owner CAS backoff.

Production hosts construct the worker with
`NotificationCandidateWorker::new_with_commit_guard` and process enabled work with
`process_candidate_with_policy_revision(item_id, revision, observed_defaults)`.
The legacy `new` and `process_candidate` methods remain trusted compatibility paths
for callers that establish an equivalent transaction boundary themselves.

`NotificationCandidateService` performs preference, recipient-policy, and source
authorization checks before opening the final transaction. Inside that transaction
it:

1. validates the active candidate lease;
2. invokes `NotificationTenantCapabilityCommitGuard` with the observed revision and
   manifest defaults;
3. requires an `Allow` decision;
4. rechecks preferences;
5. inserts or validates one idempotent notification;
6. completes the candidate under the same lease CAS.

`Disabled`, `RevisionChanged`, and retryable guard failures roll back the final
transaction. The existing durable candidate failure path then clears the lease,
sets `retryable_error`, increments attempt count, records a stable code, and
schedules a retry. No notification or channel delivery attempt survives rejection.

The server guard delegates tenant override reads to
`SeaOrmModulePolicyRevisionConsumer::lock_and_resolve_static_policy_in_transaction`.
It supplies the observed manifest defaults rather than reloading the manifest after
the final transaction has started. Therefore the guard uses no second pool
connection while holding the candidate transaction.

On PostgreSQL the Modules owner locks the `module.lifecycle` cursor row with
`FOR UPDATE`, resolves `tenant_modules` on the same transaction, and compares the
current policy revision. The lifecycle transition advances the same cursor in its
tenant-state transaction. This serializes final candidate commits with production
lifecycle tenant enable/disable commits in commit order.

SQLite evidence covers transaction-bound resolution and rollback decisions only;
it is not PostgreSQL row-lock concurrency evidence. Active-manifest,
artifact-security, maintenance, and node-readiness changes are not serialized by
the lifecycle cursor.

## Runtime flags

All production loops are independent and default-off:

- `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`;
- `RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`;
- `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`.

Invalid or unreadable values remain disabled. Bootstrap order is intake → fanout →
candidate, and all loops share shutdown state.

## Cross-module contract

Producer modules import `rustok-notifications-api`, register neutral source
factories, and never call this owner synchronously. Envelope decoding and
cross-owner policy composition belong to the executable server. Forum supports
`forum.topic.created` and `forum.mention.user_added`, preserving legacy journal
references while accepting semantic source identities from committed envelopes. The
intake pre-filter is supplied by the decoder through
`NotificationOutboxEnvelopeDecoder::accepted_event_types`, so this crate never names
a module event type and a producer extends its notification vocabulary without
touching the Notifications owner.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `NotificationsModule` implementing `RusToKModule` (slug `"notifications"`, kind `ModuleKind::Core`, dependencies `["outbox"]`) and `MigrationSource`.
- **Notification Services**:
  - `NotificationsService`: Runtime-assembled facade for source discovery and registration.
  - `NotificationInboxListService`, `NotificationInboxOpenService`, `NotificationInboxUnreadCountService`.
  - Grouping and reconciliation services: `NotificationInboxGroupListService`, `NotificationInboxGroupStateService`, `NotificationInboxReconcileService`.
- **Worker & Intake Fleet**:
  - `NotificationOutboxIntakeWorker`: Intake decoder consuming transactional outbox events.
  - `NotificationFanoutWorker`: Fan-out engine materializing recipients.
  - `NotificationCandidateWorker`: Preference and channel delivery evaluation engine.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, permissions, and actor identities.
- **`rustok-outbox`**: Consumes published domain events and writes delivery receipts.
- **`rustok-notifications-api`**: Discovers neutral source providers and defines cross-boundary contracts.

## Minimum Contract Set

### Input DTOs/Commands
- `NotificationInboxListRequest` (`user_id`, `state`, `limit`, `cursor`)
- `NotificationInboxOpenRequest` (`notification_id`, `user_id`)
- `NotificationInboxMarkAllReadRequest` (`user_id`, `scope`)
- `NotificationInboxGroupStateRequest` (`user_id`, `group_key`, `action`)
- `NotificationInboxReconcileRequest` (`user_id`, `group_key`)

### Domain Invariants
- Strict tenant and user isolation: All queries, state mutations, and worker operations require non-nil `tenant_id` and `user_id`.
- Finite inbox cursor size: `MAX_NOTIFICATION_INBOX_CURSOR_BYTES` (512 bytes) and bounded page limits (`MAX_NOTIFICATION_INBOX_PAGE_SIZE` = 100).
- Idempotent state transitions: Repeated mark-read/mark-archived transitions are idempotent and safe against outbox replay.
- Atomicity of candidate lease: Commit guards require `Allow` decision with matching policy revision before writing final notification rows.

### Events / Outbox Side Effects
- Outbox intake consumes committed producer domain events via `NotificationOutboxEnvelopeDecoder`.
- Notification state changes trigger delivery receipts and outbox notifications for push/email dispatch.

### Errors / Failure Codes
- `NotificationError::Validation` — invalid cursor, bounds exceeded, or malformed request.
- `NotificationError::NotFound` — notification item or group does not exist.
- `NotificationError::Unauthorized` — caller lacks permission for targeted user inbox.
- `NotificationError::Database` — SeaORM connection or query error.
