# FORUM-20AG notification inbox storefront port

`NotificationInboxStorefrontPort` is the transport-neutral Notifications owner boundary for one authenticated user's grouped inbox.

## Scope and identity

- `PortContext.tenant_id` is the only tenant identity source;
- `PortContext.actor.id` is the only recipient identity source;
- the actor must be `PortActorKind::User`;
- storefront request DTOs contain no tenant or recipient fields;
- tenant and user identities must parse as non-nil UUIDs before any owner query.

This prevents an HTTP, GraphQL, native, or other transport adapter from selecting another user's inbox by forwarding caller-controlled owner identifiers.

## Admission policy

The port applies shared `rustok-api` policy before owner access:

- unread count, group summaries, exact-group items, and open authorization require `PortCallPolicy::read()`, including deadline semantics;
- group-state commands require `PortCallPolicy::write()`, including deadline and non-empty idempotency-key semantics.

The idempotency key remains transport context. The delegated group-state owner is already state-idempotent and cursor-resumable; this slice adds no parallel command ledger.

## Owner delegation

The port composes existing Notifications services rather than duplicating domain logic:

- `NotificationInboxUnreadCountService`;
- `NotificationInboxGroupSummaryService`;
- `NotificationInboxGroupListService`;
- `NotificationInboxOpenService`;
- `NotificationInboxGroupStateService`.

Current recipient privacy and source target authorization remain in the existing grouped read/open owners. Group-state commands remain inside the exact state-owner transition boundary. Reads and writes create or change no delivery attempt.

## Error boundary

Owner validation maps to a stable transport validation envelope. Retryable database, source, recipient-policy, and capability failures map to sanitized unavailable errors while preserving retryability. Invalid persisted descriptors and serialization failures map to sanitized invariant violations. Database/provider detail is not exposed through `PortError`.

## Deliberate residual

This slice does not add a Leptos server function, HTTP route, GraphQL resolver, host runtime composition, or grouped UI. `rustok-notifications-storefront` therefore continues rendering its explicit unavailable state until a native adapter can extract authenticated tenant/user context, obtain the runtime source registry and recipient policy, and call this port.

## Evidence

SQLite source evidence is `tests/inbox_storefront_port_sqlite.rs`. The machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-inbox-storefront-port.json`, and the static source contract is `scripts/verify/verify-forum-notification-inbox-storefront-port.mjs`.

Tests, Cargo commands, formatting commands, verifier execution, workflows, and CI were not run by the implementation agent.
