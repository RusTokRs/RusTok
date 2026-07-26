# FORUM-20AF bounded notification group state commands

`NotificationInboxGroupStateService` applies one bounded state action to one exact tenant, recipient, and stored notification group.

## Contract

- the request carries a validated opaque `group_key`, one typed action, an optional `i1` cursor, and a bounded limit;
- requests default to 20 eligible rows and are capped at 64;
- rows are ordered by `created_at DESC, id DESC`;
- `mark_read` selects only unread or seen rows;
- `mark_unread` selects only seen or read rows;
- `archive` selects only non-archived rows;
- every selected notification delegates to `NotificationInboxStateService`;
- unread-to-read assigns matching seen/read timestamps;
- seen-to-read preserves the existing seen timestamp;
- mark-unread clears seen/read timestamps;
- archive preserves prior seen/read history and remains terminal;
- continuation derives from the last scanned eligible raw row;
- the response exposes only `scanned`, `changed`, `next_cursor`, and `has_more`;
- missing, foreign, and already-satisfied groups return an empty page without notification identity;
- earlier exact transitions remain durable and idempotent if a later database operation fails;
- the owner calls no recipient privacy, source target, or delivery owner and creates no delivery attempt.

The group-key validation boundary is shared with `NotificationInboxGroupListService`, preventing divergent whitespace, control-character, or 191-byte rules.

## Deliberate residual

This slice does not deliver external GraphQL/native transport, grouped admin/storefront UI, tenant-wide scheduled reconciliation, payload redaction, channel delivery, or PostgreSQL runtime execution evidence.

## Evidence

SQLite source evidence is in `tests/inbox_group_state_sqlite.rs`. It covers bounded cursor progress, exact group and recipient isolation, eligible-state filtering, direct unread-to-read timestamps, seen history preservation, mark-unread clearing, archive history preservation, fail-closed validation, shared limits, unchanged foreign groups, and zero delivery attempts.

The static source contract is `scripts/verify/verify-forum-notification-inbox-group-state.mjs`, and the machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-inbox-group-state.json`.

Tests, formatting, Cargo commands, verifiers, workflows, and CI were not run by the implementation agent.
