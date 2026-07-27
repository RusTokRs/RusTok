# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

The package exposes a native Leptos server-function adapter over the
transport-neutral `NotificationInboxStorefrontPort`. The owner boundary provides:

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

The unread count, grouped summaries, and exact-group item pages use one selected read
transport facade:

- SSR and hydrate builds select the native server functions;
- CSR and headless builds select the module-owned GraphQL queries through
  `rustok-graphql`;
- the existing no-context UI read functions remain compatibility wrappers that resolve
  current token and tenant transport credentials before calling the selected facade;
- explicit-context selected functions remain available to headless consumers;
- neither transport request accepts tenant, recipient, or user identity;
- transport selection is compile-profile based and does not attempt cross-path fallback.

The owner GraphQL schema exposes `notificationInboxUnreadCount`,
`notificationInboxGroupSummaries`, and `notificationInboxGroupItems`. Manifest-generated
schema composition invokes `graphql::attach_schema_data`, which reuses the host database,
materialized `NotificationSourceRegistry`, existing `NotificationRecipientPolicyRuntime`,
and `NotificationInboxStorefrontPort`. It does not create a parallel inbox service,
registry, policy, or direct storefront database query.

Grouped GraphQL reads preserve the same owner semantics as native reads:

- authenticated human-user and matching-tenant admission occurs before module access;
- tenant and recipient scope are derived from authenticated request context;
- `PortContext` carries the canonical user actor, storefront channel, effective locale,
  claims, correlation identity, and a five-second read deadline;
- cursors and limits remain owner-bounded;
- current recipient policy and source target authorization are rechecked by the owner;
- missing or suppressed groups remain non-oracular;
- UUIDs and timestamps cross the GraphQL wire as strings, state and priority as enums,
  and bounded template data as ordered key/value fields rather than arbitrary JSON;
- unavailable and invariant failures use stable sanitized public envelopes.

`NotificationNavigation` is a module-owned no-prop header action registered through the
storefront manifest. It builds the localized inbox route through
`UiRouteContext::module_route_base("notifications")`, reads the exact owner unread count,
and displays the exported `NotificationUnreadBadge` only when the count is non-zero. A
zero count still leaves the localized Notifications link available. Missing human-user
authentication, tenant mismatch, disabled module state, and transport failures hide this
best-effort action without breaking the application header.

The navigation unread-count read is dual-path, and grouped inbox reads now have the same
native/GraphQL profile parity. Fresh notification open authorization and group-state
commands are still native-only. Their GraphQL write/security parity remains a separate
gate and is not implied by grouped read parity.

Public entry points include `NotificationsView`, `NotificationNavigation`,
`NotificationUnreadBadge`, compatibility read functions, explicit-context selected read
functions, native open/command functions, and the serializable storefront request/page
models exported from the crate root.
