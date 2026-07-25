# `rustok-notifications` live contract

## Responsibility zone

Notifications owns inbox/read state, preferences, bounded fanout, grouping,
digests, retention, delivery attempts, intake receipts/quarantine, and
replay/reconciliation. Source modules own semantic state, subscriptions, audience
facts, visibility, target authorization, and routes. Profiles and Social Graph own
recipient privacy. Delivery modules own channel transports.

## Integration boundary

`rustok-notifications-api` is the neutral source contract. Producers register
`NotificationSourceProviderFactory` values through `ModuleRuntimeExtensions`; the
server materializes them with `HostRuntimeContext`. Duplicate slugs, source
identity mismatches, and build failures are startup errors.

The owner does not decode platform envelopes and does not read producer-private
or Modules-private tables. The executable server injects envelope decoding and
cross-owner policy ports. Producer transactions remain independent from
notification availability.

Notifications remains absent from `settings.default_enabled`; tenants must have an
effective `notifications` capability before provider materialization, audience
resolution, or candidate processing.

## Persistence

Five module-local PostgreSQL/SQLite migrations create notification/read lifecycle,
delivery attempts, fanout jobs/items and leases, durable source inbox state,
outbox intake receipts/quarantine, preferences, digests, and encrypted push
subscriptions.

Accepted and rejected intake outcomes are keyed by outbox event ID and mutually
exclusive. Source inbox and accepted receipt commit in one transaction. Permanent
invalid envelopes are quarantined; retryable failures retain no terminal intake
record. Accepted replay re-decodes the current envelope and must match the
persisted source identity. The intake consumer neither depends on nor mutates
relay status.

The schema stores no source-private payload, rendered HTML, contact address, phone
number, or plaintext push endpoint. Global server migration composition remains a
maintainer verification gate.

## Runtime pipeline

### Durable outbox intake

`NotificationOutboxIntakeWorker` selects supported committed `sys_events` rows in
stable `created_at/id` order, 32 by default and 64 maximum. Accepted receipts and
permanent rejections are anti-joined, preventing invalid head-of-line starvation.
The loop is default-off behind `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

### Durable source fanout

`NotificationFanoutService` is the canonical lease and persistence authority.
`NotificationFanoutWorker` selects tenant-scoped source/job work without acquiring
leases; the default/hard batch is 32/64 and one audience page is capped at 256.
Before each source or job call, the server resolves effective `notifications`
capability.

Disabled tenant work is moved to `retryable_error` for 300 seconds; temporary
policy lookup failure is deferred for 30 seconds. Owner CAS transitions increment
attempt count, set `next_attempt_at`, clear lease fields, and persist stable error
metadata before any producer call. The loop is default-off behind
`RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED` and creates only pending candidates.

A bounded producer scan may return zero recipients with a next cursor. The cursor
must differ from the claimed cursor; the owner persists it under the same lease
CAS, creates no candidates, and keeps the job pending. Oversized pages and stalled
cursors fail closed.

### Candidate policy and lifecycle-serialized inbox creation

`NotificationCandidateWorker` selects bounded tenant-scoped work without acquiring
a lease. Before canonical claim, the server calls
`EffectiveModulePolicyService::resolve_snapshot`, requires `notifications`, and
captures both the deterministic policy revision and the manifest default-enabled
module set used to compute it. Disabled or unresolved work receives the
300/30-second owner CAS backoff without invoking recipient privacy or source
providers.

Enabled work is processed in this order:

1. claim/recover the candidate lease;
2. resolve exact preferences before wildcards;
3. evaluate Profiles/Social Graph recipient policy;
4. reauthorize the target for the recipient;
5. open the final notification transaction and validate the lease;
6. invoke `NotificationTenantCapabilityCommitGuard` with the observed policy
   revision and manifest defaults;
7. recheck preferences;
8. insert or validate one notification and complete the candidate under the same
   lease CAS.

The commit guard delegates to `SeaOrmModulePolicyRevisionConsumer`. The Modules
owner locks the `module.lifecycle` cursor and resolves `tenant_modules` on the
candidate transaction using the already-observed manifest defaults. The manifest
is not reloaded through another pool connection while the final transaction is
active. Current `notifications` enablement and the observed policy revision must
both match.

On PostgreSQL, the cursor uses `FOR UPDATE`. Production lifecycle tenant toggles
advance the same cursor inside their tenant-state transaction, so final candidate
commit and tenant enable/disable are serialized by commit order. A disable that
commits first rejects notification creation; a candidate that owns the cursor first
commits before the later disable. Disabled, changed-revision, or retryable guard
outcomes roll back the notification transaction and enter durable candidate retry.

SQLite evidence covers transaction-bound resolution and rollback behavior only;
it does not claim PostgreSQL lock-contention evidence. Active-manifest,
artifact-security, maintenance, and node-readiness changes are not yet serialized
by this lifecycle cursor.

The loop remains default-off behind
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`, requires ready recipient-policy
ports and `ModuleRegistry`, and never creates channel delivery attempts.

### Inbox open-time authorization

`NotificationInboxOpenService` loads one stored notification by exact notification,
tenant, and recipient identity. Missing, cross-tenant, and cross-recipient rows all
return `Unavailable` before recipient policy or source authorization, preventing a
notification existence oracle.

For an owned row, the service reconstructs bounded source and target identities,
then evaluates the same injected Profiles/Social Graph recipient policy used during
candidate processing. Suppression returns `Unavailable` without invoking the source
provider, while temporary policy failures preserve retryability.

Only an allowed recipient reaches the registered source provider's
`authorize_target_open` method. The service returns only the fresh owner-provided
route or `Unavailable`. It does not expose the stored row, mutate
`seen/read/archive` state, or enqueue delivery attempts.

### Bounded authorized inbox listing

`NotificationInboxListService` scans one exact tenant/recipient page in
`created_at DESC, id DESC` order. A request defaults to 20 rows, is capped at 64,
and may apply one exact notification-state filter. Its versioned cursor preserves
full timestamp nanoseconds plus the UUID tie-breaker.

Each scanned row is passed through `NotificationInboxOpenService`. Current recipient
privacy and source target authorization therefore decide whether the row is returned.
The list read model exposes typed source, notification type, template key,
source-owned template data, actor, priority, state, and inbox timestamps. It adds no
dedicated route or structural target owner, kind, or ID fields.

The next cursor is derived from the last scanned raw row rather than the last returned
item. Privacy or source suppression may produce an empty page with a next cursor,
while still preserving bounded work and forward progress. Retryable policy or source
failures abort the page without returning a partial result. Listing does not mutate
`seen/read/archive` state or enqueue delivery attempts.

### Exact inbox state mutations

Exact seen/read/archive state APIs are owner-public through
`NotificationInboxStateService`. Every request requires non-nil notification,
tenant, and recipient identities and updates only the exact owned row. Missing,
cross-tenant, and cross-recipient rows return the same `Unavailable` decision.

The monotonic order is `unread → seen → read → archived`. `mark_seen` changes only
unread rows. `mark_read` changes unread or seen rows; direct unread-to-read assigns
`seen_at` and `read_at` from the same instant, while seen-to-read preserves the
existing `seen_at`. `archive` changes every non-archived row and preserves existing
seen/read timestamps.

No command downgrades an archived row. Same-state and later-state requests are
idempotent: `changed=false`, state timestamps remain unchanged, and `updated_at`
is not rewritten. The response contains only notification state and inbox
timestamps. The service calls no recipient-policy, source-provider, target, or
delivery owner and does not create delivery attempts.

SQLite evidence is `tests/inbox_state_sqlite.rs`. Mark-unread, bulk/mark-all
mutations, canonical unread counts, grouped inbox views, external transport
adapters, and module-owned UI remain closed.

The server starts workers in intake → fanout → candidate order. Invalid or
unreadable flags remain disabled.

## Forum sources

Forum supports `forum.topic.created` and `forum.mention.user_added`. Its provider
accepts legacy journal UUID/sequence references and semantic source identities from
committed envelopes. Mention handling verifies immutable relation and current
target visibility. Pending replies are retryable; closed, hidden, deleted,
self-mentioned, or restricted sources fail closed. Moderator audience expansion
remains deferred.

## Pending capabilities

- serialize active-manifest, artifact-security, maintenance, and node-readiness
  policy changes with final candidate commits;
- PostgreSQL cursor/lease contention evidence and operational health/lag metrics;
- grouping and bounded moderator-directory expansion;
- mark-unread, bulk/mark-all mutations, canonical unread counts, and grouped views;
- external inbox transport adapters and full module-owned UI;
- channel delivery enqueue and transports with delivery-time authorization;
- retention, reconciliation, quarantine replay/purge, and administrative repair.

## Maintainer verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-modules --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-modules --test policy_commit_guard_sqlite -- --nocapture
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sparse_page_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_open_authorization_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_listing_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_state_sqlite -- --nocapture
cargo test -p rustok-notifications --test outbox_intake_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_policy_deferral_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-notifications-fanout-worker.mjs
node scripts/verify/verify-forum-notification-inbox-open-authorization.mjs
node scripts/verify/verify-forum-notification-inbox-open-privacy.mjs
node scripts/verify/verify-forum-notification-inbox-listing.mjs
node scripts/verify/verify-forum-notification-inbox-state-mutations.mjs
cargo xtask module validate notifications
```

These commands were not run while publishing `NOTIFY-03D/03E/03F/03G/03H/03I` or
`FORUM-20R/20S/20T/20U`.

## Related documents

- [Module README](../README.md)
- [Implementation gates](implementation-plan.md)
- [Outbox intake contract](../contracts/notifications-outbox-intake.json)
- [Fanout worker contract](../contracts/notifications-fanout-worker.json)
- [Candidate worker contract](../contracts/notifications-candidate-worker.json)
- Canonical roadmap: `crates/rustok-forum/docs/implementation-plan.md`
