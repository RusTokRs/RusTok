# forum-20ab-selected-state

Status: source-ready / unvalidated.

This owner note records the Forum/Notifications source boundary only; runtime execution and compiled verification remain maintainer-owned.

Tests, Cargo, formatting, and CI were not run by the implementation agent.

## Source contract

- # FORUM-20AB selected inbox state owner
- 1 through 64 unique, non-nil notification IDs
- NotificationInboxStateService
- `requested`, `changed`, and `not_changed`
- tests/inbox_selected_state_sqlite.rs
