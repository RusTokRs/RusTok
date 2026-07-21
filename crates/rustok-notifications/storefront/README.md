# `rustok-notifications-storefront`

Module-owned Leptos storefront package for the notification inbox.

Until the owner inbox read API exists, the package renders an explicit
unavailable state with `unread_count = None`. It does not invent unread state,
read local storage, or persist a shadow inbox in the host.

Public entry point: `NotificationsView`.
