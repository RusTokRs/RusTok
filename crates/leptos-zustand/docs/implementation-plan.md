# leptos-zustand implementation plan

## Current state

`leptos-zustand` currently provides only the serializable generic DTOs
`StoreSnapshot<T>` and `StoreUpdate<T>`. The workspace declares the crate for
the Leptos hosts, but there are no production call sites for either type. It
does not yet implement a state container, persistence, subscriptions, or a
host integration.

## Readiness

- FFA/FBA status: `not_started` — this shared helper has no UI or transport
  boundary of its own.
- Owner: UI platform.
- Dependencies: host teams must identify a genuine cross-component state
  workflow before this crate becomes a runtime dependency.
- Verification: `cargo test -p leptos-zustand`.

## Next results

1. **Decide the supported state contract.** Confirm one concrete host workflow
   that needs serializable cross-component state, or remove the unused host
   dependencies and keep the DTOs as an inactive package. Done when the
   decision records the owner, lifecycle, and why an app-local Leptos signal
   is insufficient.
2. **Implement only the approved minimal runtime integration.** If the
   decision retains the crate, add the smallest typed API needed by the chosen
   workflow, including update semantics and an owning-host example. Done when
   the application uses the public API without module business logic leaking
   into this crate.
3. **Lock Rust/Next contract parity.** Verify that the Rust DTO wire shape and
   `packages/leptos-zustand` TypeScript types remain compatible, with a
   serialization fixture or contract test. Done when a breaking field or
   naming change fails a repeatable check.

## Verification

- `cargo test -p leptos-zustand`
- Targeted host-consumer and Rust/Next wire-contract tests after adoption.

## Change rules

1. Keep product state policy in the consuming host.
2. Do not add a runtime store without an approved owner and lifecycle contract.
3. Update the README and host documentation with a public DTO shape change.
