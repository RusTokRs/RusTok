# Pricing owner port error safety

Status: **source-ready / unvalidated**

## Scope

This contract closes the currently identified public-message and payload-diagnostic gaps in `crates/rustok-pricing/src/ports.rs` across:

- tenant and write-actor UUID parsing;
- direct variant/product consistency validation;
- direct price and price-list not-found outcomes;
- `pricing_error_to_port_error` owner mapping.

The six `PricingReadPort` operations and four `PricingWritePort` operations remain unchanged. Request and response DTOs, policy and write-semantics checks, service delegation, price resolution, discount application, price-list scope/rule behavior, event publication and persistence behavior are unchanged.

## Static public outcomes

Codes, `PortErrorKind` values and retryability are preserved. Dynamic payload-bearing messages are replaced with static messages:

- product and variant identities are not included in not-found messages;
- product/variant mismatch messages do not include either UUID;
- handles, locales, SKUs and shipping-profile slugs are not included;
- requested and available stock values are not included;
- tenant and actor parser messages no longer expose internal `PortContext` wording.

The stable database, validation, option, no-variant, published-product and invariant messages remain bounded.

## Bounded context

Pricing owner events retain correlation ID, exact owner operation, stable code and boundary. Context is represented only through bounded shape:

- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, trace and idempotency presence/length;
- locale length and deadline.

Raw tenant, actor, channel, locale, causation ID, traceparent and idempotency key values are not recorded by `ports.rs` diagnostics.

## Bounded owner errors

All fifteen current `CommerceError` variants receive a closed static variant label. Diagnostics retain only:

- text-field count and aggregate character length;
- UUID-field count and non-nil count;
- numeric-field count plus non-zero and negative counts;
- opaque-payload presence for database, rich and core errors.

Database/Rich/Core payloads, validation and price text, UUID values, handles, locales, SKUs, slugs and exact inventory values are not recorded.

## Severity

Database, Rich and Core outcomes remain error severity. Not-found, conflict and validation outcomes remain warning severity. Direct mismatch and direct not-found outcomes are also warning severity.

## Deliberate boundary

`crates/rustok-pricing/src/read_context.rs` and `crates/rustok-pricing/src/write_context.rs` are separate canonical local-diagnostic surfaces. They are not changed or claimed closed by this contract. The broader ecommerce mapper cleanup also remains open.

Compile validation, focused and aggregate verifier execution, verifier-test execution and mounted runtime evidence remain open.

## Evidence

- `crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source.json`
- `crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source-review.json`
- `scripts/verify/verify-pricing-owner-port-error-safety.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs`

No test, verifier, formatter, Cargo, workflow or CI command was executed for this source contract.
