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
the capability is explicitly enabled. No owner schema exists in this slice;
the first migration belongs to `NOTIFY-01`.

Forum is the first source provider. It supports `forum.topic.created`, binds the
source revision to the Forum event-journal sequence, emits only bounded semantic
data, resolves category watchers through a capped UUID cursor, excludes the
actor, and fails closed for channel-restricted, cross-tenant, missing, or
non-open targets. Target-open authorization returns only a validated internal
route.

No source payload, rendered HTML, contact address, storage credential, or source
database model crosses the neutral contract. Provider absence is a healthy empty
state. Producer transactions remain independent from notification availability.

Pending capabilities are owner persistence/preferences, durable consumption,
leased fan-out, full privacy/block policy, inbox APIs, delivery providers, and
complete module-owned UI products.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-forum --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --all-targets
cargo test -p rustok-notifications-api
cargo test -p rustok-notifications
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
cargo xtask module validate notifications
cargo xtask module validate forum
```

## Related Documents

- [Module README](../README.md)
- [Module-local implementation gates](implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
