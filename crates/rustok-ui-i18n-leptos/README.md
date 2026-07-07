# rustok-ui-i18n-leptos

`rustok-ui-i18n-leptos` is the Leptos adapter for the framework-agnostic
`rustok-ui-i18n` message catalog.

It may read the host-provided `UiRouteContext.locale`, but it does not select
locales from cookies, headers or query parameters. Locale selection remains a
host/runtime responsibility.

Use this crate from Leptos module-owned UI packages instead of duplicating
catalog initialization boilerplate in every package.
