# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

The package exposes a native Leptos server-function adapter over the
transport-neutral `NotificationInboxStorefrontPort`. The adapter provides:

- exact authenticated-user unread count;
- bounded group summaries;
- bounded exact-group item pages;
- fresh notification open authorization;
- bounded group mark-read, mark-unread, and archive commands.

Every endpoint extracts `AuthContext`, `TenantContext`, and `RequestContext`, rejects
auth/tenant mismatch and OAuth service principals, derives the user actor through the
canonical `AuthContext::port_actor` mapping, and composes the owner service from the
materialized notification source registry plus recipient-policy runtime already present
in `HostRuntimeContext`. Reads carry a five-second deadline. Group-state writes
additionally carry the caller-supplied idempotency key. Transport request DTOs contain
no tenant, recipient, or user identity field.

`NotificationsView` now renders the owner-backed grouped inbox with:

- an exact unread-count badge;
- SSR bootstrap loading plus explicit empty and unavailable states;
- bounded group-summary and exact-group item pagination;
- one expanded group at a time;
- fresh open authorization before browser navigation;
- bounded mark-read, mark-unread, and archive actions;
- authoritative refresh after every mutation instead of optimistic count changes;
- in-memory page deduplication without local storage or a shadow inbox.

One group action intentionally applies at most 64 eligible owner rows. When more rows
remain, the UI reports that the caller should repeat the action after the authoritative
refresh. The exported `NotificationUnreadBadge` is reusable by a future application
navigation slot, but global navigation/header composition is not part of this package
slice.

Public entry points include `NotificationsView`, `NotificationUnreadBadge`, the native
transport functions, and the serializable storefront request/page models exported from
the crate root.
