# FORUM-20AB selected inbox state owner

`NotificationInboxSelectedStateService` applies one explicit state action to a bounded set of notification identities owned by one exact tenant and recipient.

## Contract

- accepted actions are `mark_seen`, `mark_read`, `mark_unread`, and `archive`;
- each request contains 1 through 64 unique, non-nil notification IDs;
- input order is preserved;
- every ID delegates to `NotificationInboxStateService`, so exact transition, timestamp, idempotency, and terminal-archive rules remain authoritative;
- missing, cross-tenant, cross-recipient, already-satisfied, and protected-state rows are all represented only by the aggregate `not_changed` count;
- the response contains `requested`, `changed`, and `not_changed` counts and no notification or semantic target identity;
- earlier exact transitions remain durable and idempotent if a later database operation fails;
- no recipient privacy, source provider, target, or delivery owner is called, and no delivery attempt is created or changed.

This owner surface does not publish HTTP, GraphQL, native server-function, admin, or storefront adapters. Grouped inbox views, tenant-wide scheduled reconciliation, payload redaction, channel delivery, and PostgreSQL/cross-consumer runtime evidence remain separate work.

## Evidence

Source-ready SQLite coverage is in `tests/inbox_selected_state_sqlite.rs`. The static source contract is `scripts/verify/verify-forum-notification-inbox-selected-state.mjs` and the machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-inbox-selected-state.json`.

Tests, formatting, Cargo commands, verifiers, workflows, and CI were not run by the implementation agent.
