# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

The package now exposes a native Leptos server-function adapter over the
transport-neutral `NotificationInboxStorefrontPort`. The adapter provides:

- exact authenticated-user unread count;
- bounded group summaries;
- bounded exact-group item pages;
- fresh notification open authorization;
- bounded group mark-read, mark-unread, and archive commands.

Every endpoint extracts `AuthContext`, `TenantContext`, and `RequestContext`, rejects
auth/tenant mismatch, derives tenant and recipient identity only through
`PortContext`, and composes the owner service from the materialized notification source
registry plus recipient-policy runtime already present in `HostRuntimeContext`. Reads
carry a five-second deadline. Group-state writes additionally carry the caller-supplied
idempotency key. Transport request DTOs contain no tenant, recipient, or user identity
field.

The grouped Leptos inbox view has not been delivered yet, so `NotificationsView`
continues to render the explicit unavailable state with `unread_count = None`. It does
not invent unread state, read local storage, persist a shadow inbox, or bypass the
native owner adapter.

Public entry points include `NotificationsView`, the native transport functions, and
the serializable storefront request/page models exported from the crate root.
