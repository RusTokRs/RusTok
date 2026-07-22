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

The module-local migration source now creates PostgreSQL/SQLite tables for:

- notifications and read/seen/archive lifecycle;
- delivery attempts and retry/lease/provider receipt state;
- fan-out jobs/items;
- source/type-scoped preferences;
- digest jobs/items;
- encrypted push subscriptions.

Recipient and user references are tenant-composite. Notification identity is
deduplicated by tenant, recipient, source event, source slug, and notification
type, with additional tenant-scoped idempotency keys. JSON payloads, cursors,
error fields, scope keys, and encrypted endpoint material are bounded. Database
guards enforce typed statuses/channels/modes, read-implies-seen, valid lease and
completion fields, and cross-tenant actor/fan-out references.

The persistence schema intentionally stores no source-private payload, rendered
HTML, email address, phone number, or plaintext push endpoint. Push endpoint and
key material require encrypted columns, an endpoint hash, and a key version.

The migration is exposed through `NotificationsModule::migrations`. Global
server migrator registration and the first transactional owner services remain
open until maintainer verification of this schema slice.

## First source

Forum supports `forum.topic.created`, binds the source revision to the Forum
event-journal sequence, emits only bounded semantic data, resolves category
watchers through a capped UUID cursor, excludes the actor, and fails closed for
channel-restricted, cross-tenant, missing, or non-open targets. Target-open
authorization returns only a validated internal route.

Provider absence is a healthy empty state. Producer transactions remain
independent from notification availability.

Pending capabilities are global migration composition, transactional
notification/preference commands, durable source consumption, leased fan-out,
full privacy/block policy, inbox APIs, delivery providers, retention/reconciliation,
and complete module-owned UI products.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-persistence.test.mjs
cargo xtask module validate notifications
```

## Related Documents

- [Module README](../README.md)
- [Module-local implementation gates](implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
