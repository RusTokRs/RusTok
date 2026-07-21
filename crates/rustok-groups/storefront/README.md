# rustok-groups-storefront

Module-owned Leptos storefront FFA package for public group discovery, the group
shell, and authenticated invitation acceptance.

The package uses a framework-neutral core, an explicit native/GraphQL transport
facade, host-provided locale, host-provided auth session, and thin Leptos adapters.
Secret groups are never listed by the public transport. Provider-owned feature
screens are composed by the host and are not embedded in this package.

Invitation acceptance is prepared in `core`, executed through the selected
transport without implicit fallback, and bound in `ui/invitation_acceptance.rs`.

Two explicit flows are supported:

- `invite=<opaque>` or password-style manual input uses token acceptance for
  shareable and directly delivered tokens;
- `invitation=<uuid>` uses authenticated exact-recipient acceptance for a targeted
  invitation opened through an authorized Notifications source route.

The UI removes the active invitation query value when submission begins, preserves
in-memory input for a failed retry, clears it after success, and never renders a
plaintext token as result text. Native and GraphQL adapters call the same owner
ports for validation, target authorization, redemption, membership, group version,
audit, and receipt rules.

The package does not resolve recipients, publish notification events, or own inbox,
preference, email, push, retry, or delivery state. Those responsibilities remain in
the Groups backend source provider and the Notifications owner.
