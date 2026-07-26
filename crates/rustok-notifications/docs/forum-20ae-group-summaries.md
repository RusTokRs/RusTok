# FORUM-20AE bounded notification group summaries

`NotificationInboxGroupSummaryService` reads one bounded page of exact-recipient notification group summaries.

## Contract

- only non-null groups with at least one non-archived row are eligible;
- groups are ordered by their latest non-archived row using `created_at DESC, id DESC`;
- requests default to 20 raw groups and are capped at 64;
- pagination reuses the versioned `i1` timestamp-nanosecond and UUID cursor;
- `item_count` is the exact stored count of non-archived rows in the group;
- `unread_count` is the exact stored count of unread rows in the group;
- `latest_item` reuses the typed inbox read model and exposes no target route or structural target fields;
- the latest row must pass current recipient privacy before source target authorization;
- suppressed groups are omitted while continuation advances from the last scanned raw group;
- missing, archived-only, cross-tenant, and cross-recipient scopes are indistinguishably empty;
- retryable policy or source failures abort the page without a partial result;
- the read mutates no inbox timestamp and creates or changes no delivery attempt.

Counts intentionally reflect stored Notifications owner state. Current privacy or source changes converge after exact or scheduled reconciliation archives unavailable rows.

`m20260726_000016_add_notification_group_summary_index` adds the partial PostgreSQL/SQLite index `idx_notifications_group_summary` for non-archived grouped rows ordered by recipient latest activity. Existing `idx_notifications_group` continues to support exact group counts and latest-row exclusion.

## Deliberate residual

This slice does not deliver group-level mark-read, mark-unread, or archive commands. External GraphQL/native transport, admin/storefront grouped UI, tenant-wide scheduled reconciliation, payload redaction, channel delivery, and PostgreSQL runtime execution evidence remain open.

## Evidence

SQLite source evidence is in `tests/inbox_group_summary_sqlite.rs`. It covers exact stored counts, archived exclusion, latest-row ordering, cursor continuation, sparse authorization, owner isolation, validation, retryable abort, unchanged inbox state, and zero delivery attempts.

The static source contract is `scripts/verify/verify-forum-notification-inbox-group-summaries.mjs`, and the machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-inbox-group-summaries.json`.

Tests, formatting, Cargo commands, verifiers, workflows, and CI were not run by the implementation agent.
