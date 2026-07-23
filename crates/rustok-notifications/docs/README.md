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

### Candidate policy and lifecycle-serialized inbox creation

`NotificationCandidateWorker` selects bounded tenant-scoped work without acquiring
a lease. Before canonical claim, the server calls
`EffectiveModulePolicyService::resolve`, requires `notifications`, and forwards the
exact deterministic policy revision. Disabled or unresolved work receives the
300/30-second owner CAS backoff without invoking recipient privacy or source
providers.

Enabled work is processed in this order:

1. claim/recover the candidate lease;
2. resolve exact preferences before wildcards;
3. evaluate Profiles/Social Graph recipient policy;
4. reauthorize the target for the recipient;
5. open the final notification transaction and validate the lease;
6. invoke `NotificationTenantCapabilityCommitGuard`;
7. recheck preferences;
8. insert or validate one notification and complete the candidate under the same
   lease CAS.

The production server guard loads the active static manifest, then delegates to
`SeaOrmModulePolicyRevisionConsumer`. The Modules owner locks the
`module.lifecycle` cursor and resolves `tenant_modules` on the candidate
transaction. Current `notifications` enablement and the observed policy revision
must both match.

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
- channel delivery enqueue and transports;
- inbox APIs with open-time authorization/privacy rechecks;
- retention, reconciliation, quarantine replay/purge, and full module-owned UI.

## Maintainer verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-modules --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-modules --test policy_commit_guard_sqlite -- --nocapture
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

These commands were not run while publishing `NOTIFY-03D/03E/03F/03G/03H`.

## Related documents

- [Module README](../README.md)
- [Implementation gates](implementation-plan.md)
- [Outbox intake contract](../contracts/notifications-outbox-intake.json)
- [Fanout worker contract](../contracts/notifications-fanout-worker.json)
- [Candidate worker contract](../contracts/notifications-candidate-worker.json)
- Canonical roadmap: `crates/rustok-forum/docs/implementation-plan.md`
