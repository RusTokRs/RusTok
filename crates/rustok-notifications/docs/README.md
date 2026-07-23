# `rustok-notifications` live contract

## Purpose

Define the live owner and integration boundary for notifications without
copying the canonical cross-module backlog.

## Responsibility Zone

Notifications owns inbox rows, unread/read state, preferences, bounded fan-out,
grouping, digests, retention, delivery attempts, and replay/reconciliation.
Source modules own semantic event state, audience facts, subscriptions,
visibility, target authorization, and target routes. Identity/profile modules own
recipient and block/privacy facts. Delivery modules own channel transport.

## Integration

`rustok-notifications-api` provides the neutral source contract. Source modules
register `NotificationSourceProviderFactory` values through
`ModuleRuntimeExtensions`; the server materializes them with a neutral
`HostRuntimeContext` after database-backed host services exist. Duplicate
factory/provider slugs, identity mismatches, and factory build failures are
startup errors rather than silent provider loss.

`rustok-notifications` is present in module/distribution/server composition but
remains absent from `settings.default_enabled`, so tenants stay notifications-off
unless the capability is explicitly enabled.

## Persistence

The module-local migration source creates PostgreSQL/SQLite storage for:

- notifications and read/seen/archive lifecycle;
- delivery attempts and retry/lease/provider receipt state;
- fan-out jobs/items and candidate processing leases;
- a durable source-event inbox;
- source/type-scoped preferences;
- digest jobs/items;
- encrypted push subscriptions.

Recipient and user references are tenant-composite. Notification identity is
deduplicated by tenant, recipient, source event, source slug, and notification
type. Source inbox identity is deduplicated by tenant, source slug, and event ID.
Candidate processing has typed processing/retryable/terminal states, recoverable
leases, retry timing, and lease-expiry completion guards. JSON payloads, cursors,
worker IDs, errors, scopes, and encrypted endpoint material are bounded.

The persistence schema stores no source-private payload, rendered HTML, email
address, phone number, or plaintext push endpoint. The migrations are exposed
through `NotificationsModule::migrations`; global server migrator registration
remains open until maintainer verification.

## Durable source fan-out

`NotificationFanoutService` provides durable source acceptance, leased provider
description, and one bounded audience page capped at 256 recipients. Its output
is idempotent pending candidates. Provider absence after acceptance is retryable;
changed source/descriptor replay and stalled cursors fail closed.

## Candidate policy and inbox creation

`NotificationCandidateService` requires an injected
`NotificationRecipientPolicy`; no permissive implementation is supplied by the
owner. One candidate is processed in this order:

1. claim or recover its lease;
2. resolve exact source/type preference scopes before wildcard scopes;
3. evaluate recipient/profile/block/mute/tenant privacy through the injected port;
4. reauthorize the current source target for that recipient;
5. recheck preferences inside the final database transaction;
6. insert or validate one deduplicated in-app notification and complete the
   candidate under the same lease CAS.

Disabled preferences and policy/source suppression produce stable `skipped`
rows. Retryable owner/provider failures retain retry state. Semantic mismatch
fails permanently. The workflow creates no channel delivery attempts and invokes
no source or privacy provider inside the final notification transaction.

Profiles privacy and Social Graph block/mute providers are composed through
owner ports. Privacy and source authorization must be checked again on inbox open
and before delayed delivery.

## Candidate worker

`NotificationCandidateWorker` selects one stable, bounded page of claimable
candidate IDs and delegates every item to `NotificationCandidateService`. The
canonical batch is 32 and the hard maximum is 64. Selection is ordered by
`created_at`, then `id`, and does not acquire a lease; the existing per-item claim
CAS remains authoritative under worker contention.

The executable server loop is fail-closed and disabled by default. It starts only
on a background-worker host when
`RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED` is explicitly enabled, the source
registry has been materialized, and `candidate_worker_ready()` confirms both
relation ports and enablement. Invalid or unreadable flag values remain disabled.
The shared shutdown signal prevents later claims while allowing an already
processing candidate to complete its canonical transaction.

## Forum sources

Forum supports `forum.topic.created` and `forum.mention.user_added`. The mention
provider verifies the exact immutable relation row and rechecks current
source visibility while describing, resolving, and authorizing the target. A
pending reply is retryable; closed, hidden, deleted, self-mentioned, or
channel-restricted sources fail closed. `forum.mention.audience_added` remains
deferred until a bounded moderator-directory owner port exists.

Producer transactions remain independent from notification availability.

Pending capabilities include the production outbox consumer runner, channel
delivery commands, moderator audience expansion, grouping, inbox APIs,
PostgreSQL lease evidence, worker health/lag metrics, retention/reconciliation,
and complete module-owned UI products.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-persistence.test.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
cargo xtask module validate notifications
```

The commands above were not run while publishing `NOTIFY-03C`.

## Related Documents

- [Module README](../README.md)
- [Module-local implementation gates](implementation-plan.md)
- [NOTIFY-03B/07A implementation record](notify-03b-candidate-policy.md)
- [Candidate worker machine contract](../contracts/notifications-candidate-worker.json)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
