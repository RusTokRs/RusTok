# Tax calculation local outcome context

Status: **source-ready / unvalidated**

## Scope

This source slice retains stable local outcome context for the canonical root tax calculation construction:

- `TaxCalculationPort::calculate_tax`;
- root `InProcessTaxCalculationPort`;
- root `in_process_tax_calculation_port`.

The existing owner implementation in `ports.rs` remains unchanged. A root wrapper retains the delegated `PortContext` and safe request-shape facts, calls the existing `TaxService` port implementation, classifies only exact stable returned `PortError` envelopes, and returns the same error unchanged.

## Canonical root cutover

The crate keeps `pub mod ports`, so the existing module-path trait and factory remain available for compatibility. Root exports now separate the contract from canonical construction:

- root `TaxCalculationPort` continues to come from `ports`;
- root `InProcessTaxCalculationPort` and `in_process_tax_calculation_port` come from `calculation_context`.

Current cart consumers import the root factory and therefore receive the wrapper without source changes. Hosts with a custom `TaxService` may use `InProcessTaxCalculationPort::from_service`. Direct callers that deliberately construct through `rustok_tax::ports` remain an explicit compatibility bypass.

## Delegation order

The wrapper performs no new admission, request validation, provider selection, or result validation. Its source order is:

1. clone the incoming `PortContext` for diagnostics;
2. retain safe request-shape facts;
3. delegate the original context and request to the unchanged owner port;
4. inspect only a returned `PortError`;
5. emit a local event only when the exact stable code and message are covered;
6. return the same `PortError` unchanged.

Policy admission remains inside the existing owner implementation. Policy failures already retain complete owner context and are intentionally passed through without a second local event.

## Retained request facts

Covered diagnostics retain only typed identifiers, booleans, lengths, and counts:

- currency-code character length;
- optional channel id;
- tax-exempt flag;
- taxable amount count;
- line-item target count;
- shipping target count;
- dual-target count;
- country-rule count;
- configured provider-id character length;
- configured channel-provider-id character length;
- configured country-code character length.

The wrapper does not record raw currency, provider ids, country codes, tax rates, monetary amounts, tax classes, descriptions, policy rows, or provider result metadata.

## Covered stable outcomes

The mapper requires exact `code + message` pairs. Code-only matching is forbidden.

Request and owner validation outcomes use warning severity:

- `tax.currency_code_invalid` / `tax request is invalid`;
- `tax.negative_policy_rate` / `tax request is invalid`;
- `tax.country_code_invalid` / `tax request is invalid`;
- `tax.negative_country_rate` / `tax request is invalid`;
- `tax.duplicate_country_rule` / `tax request is invalid`;
- `tax.negative_taxable_amount` / `tax request is invalid`;
- `tax.validation` / `tax request is invalid`.

Provider-result invariant outcomes use error severity:

- `tax.negative_total` / `tax calculation result is invalid`;
- `tax.exempt_customer_charged` / `tax calculation result is invalid`;
- `tax.total_overflow` / `tax calculation result is invalid`;
- `tax.total_mismatch` / `tax calculation result is invalid`;
- `tax.provider_id_invalid` / `tax calculation result is invalid`;
- `tax.negative_line` / `tax calculation result is invalid`;
- `tax.currency_code_invalid` / `tax calculation result is invalid`;
- `tax.currency_mismatch` / `tax calculation result is invalid`;
- `tax.unknown_taxable_target` / `tax calculation result is invalid`.

Unavailable and timeout kinds also use error severity if a future owner implementation returns a covered stable envelope with those kinds.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_tax`;
- public operation `calculate_tax`;
- operation-specific local label;
- boundary `tax_calculation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- safe request-shape facts described above;
- exact stable internal code and public-safe message;
- typed error kind and retryability;
- the complete delegated `PortError`.

## Preserved behavior

This work does not change:

- `PortCallPolicy::read()` admission;
- request or result DTOs;
- currency, policy, taxable-target, exempt-customer, line, or total validation;
- provider resolution and `region_default` behavior;
- custom provider composition through `TaxService`;
- stable public codes, messages, kinds, or retryability;
- the original owner error return;
- FBA, FFA, or ecommerce audit status.

## Static evidence

`scripts/verify/verify-tax-calculation-local-context.mjs` guards:

- legacy module compatibility plus canonical root factory cutover;
- context and safe-fact retention before unchanged owner delegation;
- post-delegation-only mapping and same delegated error return;
- exact stable code-and-message classification;
- technical versus ordinary severity;
- complete available `PortContext`, safe request facts, and original `PortError` fields;
- absence of raw currency, provider, country, monetary, tax-class, description, and metadata fields in wrapper diagnostics;
- unchanged policy-admission and public request/result envelopes in `ports.rs`.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- direct callers that deliberately bypass the root wrapper through `rustok_tax::ports`;
- consumer-side tax transport context retention;
- external-provider runtime, timeout, retry, and remote-profile evidence;
- remaining customer, promotion, ecommerce, and non-`PortError` adapters;
- compile and cross-transport verification.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-tax-calculation-local-context.mjs
node scripts/verify/verify-tax-calculation-policy-context.mjs
node scripts/verify/verify-tax-calculation-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-tax --lib
cargo check -p rustok-cart --lib
```
