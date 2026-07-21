# `rustok-notifications` module-local implementation gates

The canonical cross-module task order and status remain in
`crates/rustok-forum/docs/implementation-plan.md`. This file does not duplicate
that backlog; it records the owner-local gates that future notification slices
must preserve.

## Scope

Preserve the neutral producer boundary, owner-only persistence, optional-module
degraded behavior, and module-owned UI packages while the notifications product
is implemented incrementally.

## Current State

The neutral API, bounded source/provider registries, deferred host factory
materialization, optional owner composition, and explicit admin/storefront
foundation states exist. Forum publishes the first real source provider for
`forum.topic.created`; an executable SQLite profile covers notifications-off/on,
bounded audience paging, target authorization, cross-tenant/non-open fallback,
and retryable provider failure classification.

No notification owner persistence exists yet. The first schema, migration,
durable consumer inbox, fan-out jobs, preferences, notification rows, and
delivery attempts remain under `NOTIFY-01` and later canonical tasks.

## Milestones

### Boundary gate

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the notifications owner synchronously;
- semantic descriptors contain bounded template data and target identity, not
  contact data or private source payloads;
- audience resolution is cursor-based and capped;
- target opening is reauthorized for the tenant and recipient;
- provider absence and module absence are explicit degraded states.

### Runtime composition gate

- `rustok-notifications` is declared in `modules.toml`, distribution features,
  the server, and owner-owned admin/storefront host packages;
- notifications is compiled into the selected server distribution but remains
  outside `settings.default_enabled`;
- source modules register deferred factories before database services exist;
- the executable host materializes factories after constructing
  `HostRuntimeContext`;
- factory/provider slug conflicts and build failures fail startup explicitly;
- Forum commands succeed without the notifications owner;
- the Forum topic-created provider reads owner event state, pages watchers,
  excludes the actor, fails closed for channel restrictions, and reauthorizes
  the current target.

### Persistence gate

Before inbox or delivery APIs are published, the owner schema must include
tenant/user composite integrity, typed statuses, stable idempotency keys, bounded
payloads, consumer inbox state, fan-out leases, delivery attempts, retention,
and reconciliation metadata. No empty migration or placeholder owner table is
added merely to satisfy composition.

### UI gate

Admin and storefront packages remain module-owned. Until persistence exists,
they expose only bootstrap/unavailable states and must not invent unread counts
or store shadow inbox state in the host.

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

## Update Rules

- keep task status in the canonical forum/notifications program plan;
- update this file only for owner-local gates or verified runtime shape;
- never move producer subscriptions, contact data, or channel SDKs into this
  owner;
- never add synchronous notification calls to producer transactions;
- add persistence and UI behavior only with matching owner contracts, migrations,
  degraded-mode notes, and verification commands.
