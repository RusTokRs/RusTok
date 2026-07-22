# `rustok-notifications` live contract

## Purpose

Define the live owner and integration boundary for notifications without
copying the canonical cross-module backlog.

## Responsibility Zone

Notifications owns inbox rows, unread/read state, preferences, bounded fan-out,
grouping, digests, retention, delivery attempts, and replay/reconciliation.
Source modules own semantic event state, audience facts, subscriptions,
visibility, target authorization, and target routes. Delivery modules own
channel-specific transport.

## Integration

`rustok-notifications-api` provides the neutral source contract. Source modules
register `NotificationSourceProviderFactory` values through
`ModuleRuntimeExtensions`; the server materializes them with a neutral
`HostRuntimeContext` after database-backed host services exist. Duplicate
factory/provider slugs, factory/provider identity mismatches, and factory build
failures are startup errors rather than silent provider loss.

`rustok-notifications` is present in `modules.toml`, the selected distribution,
server features, and module-owned admin/storefront host packages. It remains
absent from `settings.default_enabled`, so tenants stay notifications-off unless
the capability is explicitly enabled.

## Persistence

The module-local migration source creates PostgreSQL/SQLite tables for:

- notifications and read/seen/archive lifecycle;
- delivery attempts and retry/lease/provider receipt state;
- fan-out jobs/items;
- a durable source-event inbox;
- source/type-scoped preferences;
- digest jobs/items;
- encrypted push subscriptions.

Recipient and user references are tenant-composite. Notification identity is
deduplicated by tenant, recipient, source event, source slug, and notification
type, with additional tenant-scoped idempotency keys. Source inbox identity is
deduplicated by tenant, source slug, and event ID; changed event type or source
revision is a conflict rather than a second row. JSON payloads, cursors, worker
IDs, error fields, scope keys, and encrypted endpoint material are bounded.
Database guards enforce typed statuses/channels/modes, read-implies-seen, valid
lease and completion fields, and cross-tenant actor/fan-out references.

The persistence schema intentionally stores no source-private payload, rendered
HTML, email address, phone number, or plaintext push endpoint. Push endpoint and
key material require encrypted columns, an endpoint hash, and a key version.

The migrations are exposed through `NotificationsModule::migrations`. Global
server migrator registration remains open until maintainer verification.

## Durable source fan-out

`NotificationFanoutService` provides three owner phases:

- durable idempotent source-event acceptance;
- leased provider description and descriptor-bound fan-out job creation;
- leased resolution of one audience page capped at 256 recipients with
  idempotent pending candidate items and cursor advancement.

Provider absence after acceptance is retryable. Changed source identity or
changed descriptor replay fails closed. Expired leases can be reclaimed and a
cursor that does not advance becomes a terminal job error.

Pending candidate items are not final notifications. This boundary creates no
notification rows or delivery attempts. Preference, profile/block privacy,
recipient-specific source authorization, grouping, and channel policy remain
mandatory before candidate processing.

## Forum sources

Forum supports `forum.topic.created` and `forum.mention.user_added`.

The user-mention provider verifies the exact immutable `forum_user_mentions`
row and rechecks current topic/reply visibility while describing, resolving the
single candidate, and opening the target. A pending reply is retryable; closed,
hidden, deleted, self-mentioned, or channel-restricted sources fail closed.
Final profile/block privacy is deferred to `NOTIFY-07`, before candidate
processing. `forum.mention.audience_added` remains deferred until a bounded
moderator-directory owner port exists.

Provider absence is a healthy degraded state for producer commands. Producer
transactions remain independent from notification availability.

Pending capabilities are global migration composition, candidate
preference/privacy processing, final notification and delivery commands, inbox
APIs, delivery providers, retention/reconciliation, production outbox consumer
wiring, and complete module-owned UI products.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
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
cargo xtask module validate notifications
```

The commands above were not run while publishing `NOTIFY-01B/03A`.

## Related Documents

- [Module README](../README.md)
- [Module-local implementation gates](implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
