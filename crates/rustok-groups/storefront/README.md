# rustok-groups-storefront

Module-owned Leptos storefront FFA package for public group discovery, the group
shell, and authenticated invitation acceptance.

The package uses a framework-neutral core, an explicit native/GraphQL transport
facade, host-provided locale, and thin Leptos adapters. Secret groups are never
listed by the public transport. Provider-owned feature screens are composed by
the host and are not embedded in this package.

Invitation acceptance is prepared in `core`, executed through the selected
transport without implicit fallback, and bound in `ui/invitation_acceptance.rs`.
The UI accepts an opaque token from the `invite` query value or password-style
input, removes the query value when submission begins, never renders the token
as result text, and uses the owner service for all validation, redemption,
membership, audit, and receipt rules.
