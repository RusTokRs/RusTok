# forum-20ac-group-listing

Status: source-ready / unvalidated.

This owner note records the Forum/Notifications source boundary only; runtime execution and compiled verification remain maintainer-owned.

Tests, Cargo, formatting, and CI were not run by the implementation agent.

## Source contract

- # FORUM-20AC bounded notification group listing
- NotificationInboxGroupListService
- idx_notifications_group
- Current production candidate finalization still stores `group_key = NULL`
- tests/inbox_group_listing_sqlite.rs
