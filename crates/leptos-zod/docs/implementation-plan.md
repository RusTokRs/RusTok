# Implementation plan for `leptos-zod`

## Current state

`leptos-zod` owns transport-friendly validation DTOs for Leptos-facing flows:
`ZodIssue` and `ZodError`. The crate carries structured path/message issues and
an empty-check helper; it does not own validation schemas, transport clients,
route state, or domain validation policy.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `shared_ui_support`
- This UI support crate is not a module-owned FBA provider.

## Open results

1. **Validate the validation-envelope contract with consumers.** Confirm API,
   form, and adapter usage of nested field paths and error envelopes before the
   public DTO shape expands.
   **Depends on:** concrete Leptos form and API consumers.
   **Done when:** consumers exchange one serializable issue format without
   importing domain schemas into this crate.

2. **Add focused DTO contract tests.** Cover empty/non-empty errors, API
   construction, nested paths, message preservation, and serde roundtrips.
   **Depends on:** agreed public validation-envelope semantics.
   **Done when:** a compact unit suite locks the generic error format.

3. **Keep validation ownership external.** Add helpers only for reusable issue
   representation; keep schema execution, localization, field labels, and
   domain error policy with adapters or owning modules.
   **Depends on:** demonstrated cross-surface duplication.
   **Done when:** the crate remains a dependency-light DTO boundary usable by
   `leptos-hook-form` and other UI packages.

## Verification

- Targeted unit tests for DTO construction, emptiness, paths, and serde.
- Consumer tests in adopting Leptos form/API packages.

## Change rules

1. Do not add domain schemas, transport clients, or route behavior here.
2. Update the local README with a changed validation DTO contract.
3. Update consumers if the serialized issue envelope changes.
