# Pricing owner port error safety

Status: **source-ready / unvalidated**

## Scope

This contract tracks the currently identified Pricing public-message and payload-diagnostic
gaps across the complete canonical boundary:

- owner `crates/rustok-pricing/src/ports.rs`;
- canonical read `crates/rustok-pricing/src/read_context.rs`;
- canonical write `crates/rustok-pricing/src/write_context.rs`.

The six `PricingReadPort` operations and four `PricingWritePort` operations remain
unchanged. Request and response DTOs, policy and write-semantics checks, service
delegation, price resolution, discount application, price-list scope/rule behavior,
event publication, and persistence behavior are unchanged.

## Owner outcomes

Codes, `PortErrorKind` values, and retryability are preserved. Dynamic payload-bearing
owner messages are replaced with static messages:

- product and variant identities are not included in not-found messages;
- product/variant mismatch messages do not include either UUID;
- handles, locales, SKUs, and shipping-profile slugs are not included;
- requested and available stock values are not included;
- tenant and actor parser messages no longer expose internal `PortContext` wording.

All fifteen current `CommerceError` variants receive a closed static variant label.
Owner diagnostics retain only aggregate text, UUID, numeric, and opaque-payload shape.

## Canonical wrappers

Both canonical wrappers preserve their original owner delegation and local outcome
classification. Their diagnostics retain correlation ID, exact public/local operation,
stable code, retryability, message lengths, a closed error-kind label, and bounded
context/request shape.

They do not record:

- raw tenant, actor, channel, locale, causation, traceparent, or idempotency values;
- product, variant, region, channel, or price-list UUID values;
- exact read quantity or write minimum/maximum quantity values;
- public or original message text;
- debug-formatted `PortErrorKind` values;
- price, percentage, compare-at, currency, slug, handle, SKU, row, or projection payloads.

## Severity

Database, Rich, Core, unavailable, timeout, and invariant outcomes remain error severity.
Not-found, conflict, and validation outcomes remain warning severity. Direct mismatch and
direct not-found outcomes also remain warning severity.

## Boundary status

The owner `ports.rs`, canonical read wrapper, and canonical write wrapper are
**source-closed / unvalidated** for the currently identified Pricing public-message and
payload-diagnostic gaps.

The broader ecommerce cleanup remains open. Compile validation, focused and aggregate
verifier execution, verifier-test execution, CI, and mounted runtime evidence also remain
open.

## Evidence

- `crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source.json`
- `crates/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source-review.json`
- `crates/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source.json`
- `crates/rustok-pricing/contracts/evidence/pricing-write-local-diagnostic-safety-source.json`
- `scripts/verify/verify-pricing-owner-port-error-safety.mjs`
- `scripts/verify/verify-pricing-read-local-context.mjs`
- `scripts/verify/verify-pricing-write-local-context.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`

No test, verifier, formatter, Cargo, workflow, CI, or mounted runtime command was executed
for this source contract.
