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

The neutral API, bounded source registry, owner module skeleton, and explicit
admin/storefront foundation states exist. Runtime composition, real source
providers, persistence, consumption, delivery, and executable fallback evidence
remain open in the canonical plan.

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

Compose the owner through `modules.toml`, distribution, server, migrations, and
host package wiring only after the neutral API compiles and the first source
provider can prove notifications-off/on behavior.

### Persistence gate

Before inbox or delivery APIs are published, the owner schema must include
tenant/user composite integrity, typed statuses, stable idempotency keys, bounded
payloads, consumer inbox state, fan-out leases, delivery attempts, retention,
and reconciliation metadata.

### UI gate

Admin and storefront packages remain module-owned. Until persistence exists,
they expose only bootstrap/unavailable states and must not invent unread counts
or store shadow inbox state in the host.

## Verification

```bash
cargo test -p rustok-notifications-api
cargo test -p rustok-notifications
cargo check -p rustok-notifications-admin --all-targets
cargo check -p rustok-notifications-storefront --all-targets
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
```

## Update Rules

- keep task status in the canonical forum/notifications program plan;
- update this file only for owner-local gates or verified runtime shape;
- never move producer subscriptions, contact data, or channel SDKs into this
  owner;
- never add synchronous notification calls to producer transactions;
- add persistence and UI behavior only with matching owner contracts, migrations,
  degraded-mode notes, and verification commands.
