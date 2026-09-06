# Tax calculation local outcome context

Status: **source-ready / unvalidated**

## Scope

This source slice hardens diagnostics across both tax calculation layers:

- the canonical root `InProcessTaxCalculationPort` wrapper;
- the owner `TaxCalculationPort for TaxService` implementation in `ports.rs`.

The public trait, DTOs, constructors, provider selection, validation policy, result policy,
public `PortError` envelopes, and return paths are unchanged.

## Stable attribution

The canonical wrapper now derives its local operation from `PortError.code` only.
Human-readable public messages are no longer used as routing control. The shared
`tax.currency_code_invalid` code uses the truthful generic local label
`validate_currency_code`, covering both request and provider-result currency checks.
Unknown codes pass through without an additional wrapper event.

## Safe context shape

Both layers retain the per-call correlation id. All other `PortContext` information is
represented only through safe shape:

- tenant and actor-id character lengths;
- actor kind;
- claim and role counts;
- channel, causation, traceparent, and idempotency presence plus lengths;
- locale length;
- deadline.

Raw tenant, actor id, channel, locale, causation id, traceparent, and idempotency key
values are not written by these tax diagnostics.

## Safe request and result shape

The wrapper records only:

- currency-code length;
- channel-id presence and UUID non-nil fact;
- tax-exempt flag;
- taxable, line-target, shipping-target, dual-target, and country-rule counts;
- provider-id, channel-provider-id, and country-code lengths.

The owner validation and invariant mappers retain only the presence and character
length of their internal detail text. Provider validation text is likewise reduced to
presence and length.

The diagnostic boundary does not write raw currency, provider ids, country codes,
rates, monetary amounts, line-item or shipping-option UUIDs, tax classes,
descriptions, policy rows, or provider metadata.

## Preserved behavior

The following remain unchanged:

- `PortCallPolicy::read()` admission and deadline semantics;
- request and result DTOs;
- currency, policy-rate, country-rule, taxable-amount, exempt-customer, provider-id,
  line, target, currency, and total validation;
- `TaxService::calculate` delegation and provider selection;
- canonical root and legacy module-path construction;
- every public error code, message, kind, retryability value, and returned error;
- Tax FFA/FBA status and the broad ecommerce cleanup state.

## Static evidence

The focused guards lock the new source contract:

- `scripts/verify/verify-tax-calculation-local-context.mjs`;
- `scripts/verify/verify-tax-calculation-policy-context.mjs`;
- `scripts/verify/verify-tax-calculation-error-context.mjs`.

They require stable-code-only local attribution, safe context/request/detail shape, the
same delegated error return, and unchanged public envelopes. They forbid raw context,
raw channel UUIDs, raw detail text, and message-based local routing.

Machine-readable source evidence is retained in:

- `crates/rustok-tax/contracts/evidence/tax-calculation-diagnostic-safety-source.json`;
- `crates/rustok-tax/contracts/evidence/tax-calculation-diagnostic-safety-source-review.json`.

## Remaining gaps

Direct construction through `rustok_tax::ports` remains a compatibility path, although
its owner diagnostics are now safe. Consumer-side transport context, external-provider
runtime behavior, timeout/retry evidence, remote profiles, compile evidence, and the
remaining ecommerce mapper cleanup stay open.

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
