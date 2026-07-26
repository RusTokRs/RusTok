# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

The owner transport-neutral inbox port exists in `rustok-notifications` and derives
its tenant and recipient scope from authenticated `PortContext` identity. A native
server adapter and host runtime composition have not been delivered yet, so this
package continues to render the explicit unavailable state with
`unread_count = None`.

The package does not invent unread state, read local storage, persist a shadow inbox,
or bypass the owner port while the native server adapter remains absent.

Public entry point: `NotificationsView`.
