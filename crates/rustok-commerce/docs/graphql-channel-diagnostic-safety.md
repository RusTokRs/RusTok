# GraphQL channel diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Commerce GraphQL storefront-channel admission boundary in `crates/rustok-commerce/src/graphql/mod.rs`.

The boundary performs one owner call to determine whether Commerce is enabled for the request channel. Before this change, its dependency-failure event logged the complete storage error together with the raw tenant UUID, optional channel UUID, and optional channel slug. Its disabled-channel warning logged the same request identities.

## Bounded projection

The root now creates one diagnostic context from the existing `RequestContext`:

- tenant UUID becomes `nil` / `non_nil`;
- optional channel UUID becomes `absent` / `present_nil` / `present_non_nil`;
- optional channel slug becomes `absent` / `empty` / `present`.

The dependency failure discards the storage cause before logging and shadows it with a diagnostic type whose `Debug` output is always `redacted`.

Both the dependency-error event and disabled-channel warning retain only stable owner, operation, error kind, public code, retryability, boundary, event message, and the closed identity shapes above.

## Preserved GraphQL behavior

This work does not change:

- optional `RequestContext` compatibility;
- `DatabaseConnection` resolution;
- `is_module_enabled_for_request_channel` ownership or arguments;
- the module slug;
- the public dependency-failure message `Commerce availability could not be verified`;
- the disabled-channel message `Commerce is not enabled for the current channel`;
- the `MODULE_NOT_ENABLED` GraphQL extension;
- Commerce query-root composition, mutation exports, or routing;
- Product public-error delegation and correlation extension behavior.

The additional `COMMERCE_AVAILABILITY_UNAVAILABLE` value is diagnostic metadata only and is not added to the public GraphQL envelope.

## Verifier correction

The previous root verifier expected raw internal and request identity logging. It also expected inline Product `CommerceError` variant matching that no longer exists in this root because Product public mapping is delegated to `rustok_product::map_product_public_error`.

The updated verifier follows the current source architecture and fails closed on raw channel diagnostics.

## Remaining boundary

The broad ecommerce correlation-safe mapper and non-`PortError` cleanup remains open. Storefront shared context/customer/channel mappers, storefront cart shipping, tax, promotion, remaining owner adapters, native transports, and runtime evidence are not completed by this slice.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-channel-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-root-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
