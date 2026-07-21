# `rustok-notifications` module-local implementation gates

The canonical cross-module task order and status remain in
`crates/rustok-forum/docs/implementation-plan.md`. This file does not duplicate
that backlog; it records the owner-local gates that future notification slices
must preserve.

## Boundary gate

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the notifications owner synchronously;
- semantic descriptors contain bounded template data and target identity, not
  contact data or private source payloads;
- audience resolution is cursor-based and capped;
- target opening is reauthorized for the tenant and recipient;
- provider absence and module absence are explicit degraded states.

## Persistence gate

Before inbox or delivery APIs are published, the owner schema must include
tenant/user composite integrity, typed statuses, stable idempotency keys, bounded
payloads, consumer inbox state, fan-out leases, delivery attempts, retention,
and reconciliation metadata.

## UI gate

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
