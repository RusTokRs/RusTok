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

The current slice provides a neutral source registry in
`rustok-notifications-api` and an optional `NotificationsModule` that initializes
that registry. Providers describe semantic events, resolve bounded candidate
audiences, and authorize one recipient opening one target.

No source payload, rendered HTML, contact address, storage credential, or source
database model crosses the contract. Provider absence is a healthy empty state.
Producer transactions remain independent from notification availability.

Pending runtime capabilities are persistence/preferences, durable consumption,
leased fan-out, target-open integration, inbox APIs, delivery providers, and
complete module-owned UI products.

## Verification

```bash
cargo test -p rustok-notifications-api
cargo test -p rustok-notifications
cargo check -p rustok-notifications-admin --all-targets
cargo check -p rustok-notifications-storefront --all-targets
node scripts/verify/verify-notifications-foundation.mjs
```

## Related Documents

- [Module README](../README.md)
- [Module-local implementation gates](implementation-plan.md)
- Canonical cross-module status:
  `crates/rustok-forum/docs/implementation-plan.md`
