# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

The package exposes a native Leptos server-function adapter over the
transport-neutral `NotificationInboxStorefrontPort`. The adapter provides:

- exact authenticated-user unread count;
- bounded group summaries;
- bounded exact-group item pages;
- fresh notification open authorization;
- bounded group mark-read, mark-unread, and archive commands.

Every native endpoint extracts `AuthContext`, `TenantContext`, and `RequestContext`, rejects
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

`NotificationsView` renders the owner-backed grouped inbox without a second client-side
inbox authority. One group action intentionally applies at most 64 eligible owner rows.
When more rows remain, the UI reports that the caller should repeat the action after the
authoritative refresh.

`NotificationNavigation` is a module-owned no-prop header action registered through the
storefront manifest. It builds the localized inbox route through
`UiRouteContext::module_route_base("notifications")`, reads the exact owner unread count,
and displays the exported `NotificationUnreadBadge` only when the count is non-zero. A
zero count still leaves the localized Notifications link available. Missing human-user
authentication, tenant mismatch, disabled module state, and transport failures hide this
best-effort action without breaking the application header.

The navigation unread-count read is dual-path:

- SSR and hydrate builds use the native server function;
- CSR and headless builds use the module-owned `notificationInboxUnreadCount` GraphQL
  query through `rustok-graphql`;
- neither path accepts tenant, recipient, or user identity in the request payload;
- the owner GraphQL resolver derives tenant and recipient from authenticated request
  context and maps database details to a generic stable error envelope.

The full grouped inbox, open authorization, and group-state commands are still native-only.
GraphQL parity for those operations remains a separate gate rather than being implied by
the navigation count query.

Public entry points include `NotificationsView`, `NotificationNavigation`,
`NotificationUnreadBadge`, the transport facade, and the serializable storefront
request/page models exported from the crate root.
