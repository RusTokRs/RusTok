# rustok-groups-storefront

Module-owned Leptos storefront FFA package for public group discovery and the
group shell.

The package uses a framework-neutral core, an explicit native/GraphQL transport
facade, host-provided locale, and a thin Leptos adapter. Secret groups are never
listed by the public transport. Provider-owned feature screens are composed by
the host and are not embedded in this package.
