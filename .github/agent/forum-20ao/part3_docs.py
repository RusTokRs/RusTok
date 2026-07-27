from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    source = target.read_text()
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    target.write_text(source.replace(old, new, 1))


owner = "crates/rustok-notifications/README.md"
live = "crates/rustok-notifications/docs/README.md"
storefront = "crates/rustok-notifications/storefront/README.md"

replace_once(
    owner,
    '''The module-owned grouped storefront view performs an owner-backed SSR bootstrap, bounded raw
paging, one-group expansion, stale-response rejection, authoritative refresh after writes,
and browser navigation only after `NotificationStorefrontOpenDecision::Allowed`. The generic
storefront header resolves the Notifications action from manifest metadata, builds the route
through `UiRouteContext`, and shows the exact unread badge only when positive while retaining
the link at zero. Optional capability failures hide the action without failing the header.''',
    '''The module-owned grouped storefront view performs an owner-backed SSR bootstrap, bounded raw
paging, one-group expansion, stale-response rejection, authoritative refresh after writes,
and browser navigation only after `NotificationStorefrontOpenDecision::Allowed`. It
automatically reloads its bootstrap when the reactive auth token or tenant changes and uses the
same resolved transport context for exact unread count plus the first bounded summary page. The
generic storefront header resolves the Notifications action from manifest metadata, builds the
route through `UiRouteContext`, and shows the exact unread badge only when positive while
retaining the link at zero. Optional capability failures hide the action without failing the
header.''',
)

replace_once(
    owner,
    '''Source contracts are guarded by the `FORUM-20AG` through `FORUM-20AL` machine contracts and''',
    '''Source contracts are guarded by the `FORUM-20AG` through `FORUM-20AO` machine contracts and''',
)

replace_once(
    owner,
    '''- tenant-wide scheduled reconciliation and payload redaction;
- auth-reactive automatic grouped bootstrap refresh;
- channel delivery enqueue with delivery-time authorization;''',
    '''- tenant-wide scheduled reconciliation and payload redaction;
- channel delivery enqueue with delivery-time authorization;''',
)

replace_once(
    live,
    '''The grouped Notifications view uses owner-backed SSR bootstrap, bounded pages, exact-group
expansion, stale-response guards, authoritative post-command refresh, and allowed-only route
navigation. A manifest-driven generic header action exposes the localized route and exact
unread badge without a host import of the Notifications UI. Optional failures degrade by
omitting the action rather than breaking the storefront shell.''',
    '''The grouped Notifications view uses owner-backed SSR bootstrap, bounded pages, exact-group
expansion, stale-response guards, authoritative post-command refresh, and allowed-only route
navigation. It automatically reloads its bootstrap when the reactive auth token or tenant
changes and uses the same resolved transport context for exact unread count plus the first
bounded summary page. A manifest-driven generic header action exposes the localized route and
exact unread badge without a host import of the Notifications UI. Optional failures degrade by
omitting the action rather than breaking the storefront shell.''',
)

replace_once(
    live,
    '''- tenant-wide scheduled reconciliation and payload redaction;
- auth-reactive automatic grouped bootstrap refresh;
- channel delivery enqueue and transports with delivery-time authorization;''',
    '''- tenant-wide scheduled reconciliation and payload redaction;
- channel delivery enqueue and transports with delivery-time authorization;''',
)

replace_once(
    live,
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
    '''node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs''',
)

replace_once(
    live,
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN`.''',
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO`.''',
)

replace_once(
    storefront,
    '''- authoritative refresh after every mutation instead of optimistic count changes;
- in-memory page deduplication without local storage or a shadow inbox.''',
    '''- authoritative refresh after every mutation instead of optimistic count changes;
- automatic auth/session and tenant bootstrap refresh without polling;
- in-memory page deduplication without local storage or a shadow inbox.''',
)

replace_once(
    storefront,
    '''When more rows remain, the UI reports that the caller should repeat the action after the
authoritative refresh.

The unread count, grouped summaries, exact-group item pages, fresh open authorization,''',
    '''When more rows remain, the UI reports that the caller should repeat the action after the
authoritative refresh. `NotificationsView` automatically reloads the grouped bootstrap when
the reactive token or tenant changes. Its resource source combines the manual refresh nonce
with `current_notification_storefront_transport_context`, and one resolved context is reused
for exact unread count plus the first bounded summary page.

The unread count, grouped summaries, exact-group item pages, fresh open authorization,''',
)

replace_once(
    storefront,
    '''- explicit-context selected functions remain available to headless consumers;
- raw native read, open, and group-state functions remain available with explicit''',
    '''- explicit-context selected functions remain available to headless consumers;
- `current_notification_storefront_transport_context` is the reactive no-prop UI resolver for
  current token and tenant transport credentials;
- raw native read, open, and group-state functions remain available with explicit''',
)

replace_once(
    storefront,
    '''Public entry points include `NotificationsView`, `NotificationNavigation`,
`NotificationUnreadBadge`, compatibility read/open functions, explicit-context selected
read/open/group-state functions, raw native functions, and the serializable storefront
request/page models exported from the crate root.''',
    '''Public entry points include `NotificationsView`, `NotificationNavigation`,
`NotificationUnreadBadge`, the reactive current transport-context resolver, compatibility
read/open functions, explicit-context selected read/open/group-state functions, raw native
functions, and the serializable storefront request/page models exported from the crate root.''',
)
