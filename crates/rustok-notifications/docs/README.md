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
tables. The executable server injects envelope decoding and composes cross-owner
policy. Producer transactions remain independent from notification availability.

Notifications remains absent from `settings.default_enabled`; tenants must have an
effective `notifications` capability before provider materialization or audience
resolution.

## Persistence

Five module-local PostgreSQL/SQLite migrations create:

- notification/read lifecycle and delivery-attempt state;
- fanout jobs/items and candidate leases;
- durable source inbox state;
- accepted outbox intake receipts;
- permanent owner-local intake quarantine;
- source/type preferences, digests, and encrypted push subscriptions.

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
stable `created_at/id` order, 32 by default and 64 maximum. Both accepted receipts
and permanent rejections are anti-joined, preventing invalid head-of-line
starvation.

The server decoder maps:

- root `forum.topic.created` to `forum/topic_id/1`;
- sealed `forum.mention.user_added` to `forum/envelope_id/source_revision_id`.

The executable loop is default-off behind
`RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED` and uses the shared shutdown signal.

### Durable source fanout

`NotificationFanoutService` is the canonical lease and persistence authority for
source descriptor materialization and bounded audience pages.

`NotificationFanoutWorker` selects tenant-scoped source/job work without acquiring
leases. The default/hard batch is 32/64; one audience page is capped at 256. Before
each source or job call, the server resolves
`EffectiveModulePolicyService::is_enabled(..., "notifications")`.

Disabled tenant work is moved to `retryable_error` for 300 seconds; temporary
policy lookup failure is deferred for 30 seconds. Both owner-side CAS transitions
increment attempt count, set `next_attempt_at`, clear lease fields, and persist
stable error metadata before any producer provider call. This prevents disabled
or unresolved tenant rows from occupying the bounded queue head indefinitely.

The executable loop is default-off behind
`RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`, requires a background-worker host,
materialized non-empty source registry, and module registry, and checks shutdown
between records. It creates only pending candidates—never final notifications or
delivery attempts.

### Candidate policy and inbox creation

`NotificationCandidateService` requires an injected `NotificationRecipientPolicy`.
One candidate is processed in this order:

1. claim/recover its lease;
2. resolve exact preference scopes before wildcard scopes;
3. evaluate recipient/profile/block/mute/tenant privacy;
4. reauthorize the target for the recipient;
5. recheck preferences inside the final transaction;
6. insert or validate one in-app notification and complete the candidate under the
   same lease CAS.

`NotificationCandidateWorker` is default-off behind
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`. It requires a materialized source
registry and ready relation-policy ports. Candidate finalization creates no
channel delivery attempt.

The server starts workers in intake → fanout → candidate order. Invalid or
unreadable flags remain disabled.

## Forum sources

Forum supports `forum.topic.created` and `forum.mention.user_added`. Its provider
accepts both legacy journal UUID/sequence references and semantic source identities
from committed envelopes. Mention handling verifies the immutable relation and
current target visibility at describe, audience, and open time. Pending replies
are retryable; closed, hidden, deleted, self-mentioned, or restricted sources fail
closed. Moderator audience expansion remains deferred.

## Pending capabilities

- PostgreSQL contention/recovery evidence and operational health/lag metrics;
- grouping and bounded moderator-directory expansion;
- channel delivery enqueue and transports;
- inbox APIs with open-time authorization/privacy rechecks;
- retention, reconciliation, quarantine replay/purge, and full module-owned UI.

## Maintainer verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
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
cargo xtask module validate notifications
```

These commands were not run while publishing `NOTIFY-03D/03E/03F`.

## Related documents

- [Module README](../README.md)
- [Implementation gates](implementation-plan.md)
- [Outbox intake contract](../contracts/notifications-outbox-intake.json)
- [Fanout worker contract](../contracts/notifications-fanout-worker.json)
- [Candidate worker contract](../contracts/notifications-candidate-worker.json)
- Canonical roadmap: `crates/rustok-forum/docs/implementation-plan.md`
