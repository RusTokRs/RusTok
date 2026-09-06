# Fulfillment shipping-option read diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the payload-diagnostic gap in the owner projection-read boundary implemented by `crates/rustok-fulfillment/src/shipping_option_read.rs`:

- storefront active-list;
- storefront/admin single lookup;
- administrative list-all;
- tenant UUID parsing;
- all five current `FulfillmentError` variants.

The public traits, request/response DTOs, canonical factories, locale arguments, owner service calls, Commerce runtime composition, filters, optional-not-found behavior, and public `PortError` envelopes remain unchanged.

## Retained diagnostic shape

Events retain correlation id, static owner operation, stable code, retryability, severity, and boundary. Other context is limited to actor kind, character lengths, counts, presence flags, and deadline milliseconds.

The optional shipping-option identity is represented only by presence and non-nil facts. Requested and tenant-default locales are represented only by presence and character length.

Owner failures retain only a closed static variant and aggregate text, UUID, and opaque-payload shape. Parser errors, raw context, resource UUIDs, validation/transition text, database errors, and complete `FulfillmentError` debug/display payloads are not recorded.

## Preserved behavior

- read policy remains before tenant parsing and owner delegation;
- active list, list-all, and lookup delegate to the same owner methods;
- requested/default locale propagation is unchanged;
- database failures remain error severity;
- context, validation, not-found, and transition failures remain warning severity;
- public code, message, kind, and retryability are unchanged.

## Evidence

- `crates/rustok-fulfillment/contracts/evidence/shipping-option-read-diagnostic-safety-source.json`
- `crates/rustok-fulfillment/contracts/evidence/shipping-option-read-diagnostic-safety-source-review.json`
- `scripts/verify/verify-fulfillment-shipping-option-read-port.mjs`

## Remaining gaps

Fulfillment lifecycle projection-read diagnostics in `fulfillment_read.rs` remain a separate open slice. Compile, verifier execution, mounted runtime parity, restart, remote-adapter evidence, and the broader ecommerce cleanup remain open.

No test, verifier, formatter, Cargo, workflow, or CI command was executed for this source slice.
