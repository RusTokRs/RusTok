# FORUM-20AC bounded notification group listing

`NotificationInboxGroupListService` reads one bounded page for one exact stored `group_key`, tenant, and recipient.

## Contract

- group keys are opaque exact strings containing 1 through 191 trimmed, non-control bytes;
- requests may apply one exact `NotificationState` filter;
- raw pages default to 20 rows and are capped at 64;
- selection uses `created_at DESC, id DESC` and the existing versioned `i1` cursor;
- tenant, recipient, group key, optional state, and cursor predicates are applied before any foreign owner call;
- every selected raw row reuses `NotificationInboxOpenService`, preserving current recipient privacy before source target authorization;
- continuation derives from the last scanned raw group row, so a fully suppressed page may be empty while still advancing;
- missing groups and foreign recipient scopes are indistinguishably empty;
- retryable privacy or source failures abort the group page without returning a partial result;
- the response reuses `NotificationInboxPage` and `NotificationInboxItem`, adds no target route or structural target fields, mutates no inbox timestamps, and creates no delivery attempt.

The existing `idx_notifications_group` index supports the exact owner scope. No migration or dependency change is required.

## Deliberate residual

This slice does not assign `group_key`. Current production candidate finalization still stores `group_key = NULL`, so source/Notifications grouping policy and production population remain a separate prerequisite before grouped UI or aggregate summaries can be declared complete. Group unread totals, latest-item summaries, transport adapters, admin/storefront UI, scheduled reconciliation, payload redaction, channel delivery, and PostgreSQL/cross-consumer runtime evidence remain open.

## Evidence

Source-ready SQLite coverage is in `tests/inbox_group_listing_sqlite.rs`. The static source contract is `scripts/verify/verify-forum-notification-inbox-group-listing.mjs`, and the machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-inbox-group-listing.json`.

Tests, formatting, Cargo commands, verifiers, workflows, and CI were not run by the implementation agent.
