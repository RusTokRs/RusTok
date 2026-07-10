# Implementation plan for `leptos-hook-form`

## Current state

`leptos-hook-form` owns lightweight, serializable form-state helpers for Leptos
surfaces: `FormState`, `FieldError`, `ValidationIssue`, and
`issues_to_field_errors`. It represents submission state, form errors, and
field errors without owning a form schema, transport client, route, or domain
validation policy.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `shared_ui_support`
- This UI support crate is not a module-owned FBA provider.

## Open results

1. **Validate the form-state contract with consumers.** Confirm how host and
   module UI packages apply submission transitions, field paths, and form-level
   errors before adding helper behavior.
   **Depends on:** concrete Leptos form consumers and validation adapters.
   **Done when:** consumers use the same serializable state shape without
   embedding domain schema or transport policy in this crate.

2. **Add focused form-state tests.** Cover idle/submitting/error transitions,
   error clearing, error detection, nested field-path mapping, and serde
   roundtrips.
   **Depends on:** agreed public DTO semantics.
   **Done when:** a compact test suite locks the shared behavior independently
   of individual forms.

3. **Keep schema and transport ownership external.** Extend the crate only for
   cross-surface form-state behavior; retain schema validation in adapters such
   as `leptos-zod` and transport/domain errors with their owners.
   **Depends on:** demonstrated duplication across independent forms.
   **Done when:** the public API remains generic and does not import domain,
   route, or client-specific contracts.

## Verification

- Targeted unit tests for state transitions, issue mapping, and serde.
- Consumer tests in adopting Leptos hosts or module UI packages.

## Change rules

1. Do not add domain form schemas, route state, or transport clients here.
2. Update the local README with a changed shared form-state contract.
3. Update consumers if the serialized public DTOs change.
