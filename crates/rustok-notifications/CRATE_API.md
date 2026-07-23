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
selection and are database-enforced mutually exclusive outcomes.

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
- delegates each selected record to the canonical service.

Executable hosts must establish tenant capability before invoking
`materialize_source_inbox` or `process_fanout_job`. The server uses
`EffectiveModulePolicyService::is_enabled` for module slug `notifications` before
every source and job call. Disabled or unresolved policy fails closed before
producer provider execution.

Fanout creates pending candidates only. It creates neither final notification rows
nor delivery attempts.

## Candidate contract

`NotificationCandidateService::new(db, registry, policy)` requires an explicit
recipient policy; no permissive default exists. `process_candidate`:

1. claims a pending, due retryable, or expired-processing candidate;
2. resolves exact source/type preference scopes before wildcard scopes;
3. evaluates recipient/profile/block/mute/tenant privacy;
4. invokes recipient-specific source authorization;
5. rechecks preferences inside the final transaction;
6. inserts or validates one idempotent in-app notification and completes the
   candidate under the same lease CAS.

No channel delivery attempt is created by candidate finalization.

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
references while accepting semantic source identities from committed envelopes.
