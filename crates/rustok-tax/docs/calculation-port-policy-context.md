# Tax calculation port policy context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the owner-side policy-admission context gap in the canonical
`TaxCalculationPort::calculate_tax` implementation.

Before this change, `PortContext::require_policy(PortCallPolicy::read())` ran before
the tax owner operation was assigned. A deadline or policy rejection therefore left
the tax owner without a dedicated structured event containing the full available
request context.

The port now:

- assigns `calculate_tax` before policy admission;
- delegates admission to one tax-owned helper using the unchanged read policy;
- records the truthful `rustok_tax` owner and stable `tax_calculation_port` boundary;
- retains correlation id, tenant, actor, channel, locale, causation id, traceparent,
  idempotency key, and deadline;
- retains the exact owner operation and original `PortError` code, kind, and
  retryability;
- emits error severity for unavailable, timeout, and invariant failures and warning
  severity for ordinary policy or validation rejection;
- returns the original `PortError` unchanged after diagnostics.

The canonical root factory now additionally wraps post-delegation stable validation
and result-invariant outcomes. That separate contract is documented in
[`calculation-local-context.md`](./calculation-local-context.md). Policy admission
continues to run only in the owner implementation and is not duplicated by the root
wrapper.

## Preserved behavior

The change does not alter:

- `PortCallPolicy::read()` admission semantics;
- tax request or result DTOs;
- currency, rate, taxable-target, exempt-customer, or total validation;
- `TaxService::calculate` delegation;
- stable public validation and invariant messages;
- provider result validation or existing unit-test source;
- runtime composition, FBA, or FFA status.

## Static evidence

`scripts/verify/verify-tax-calculation-policy-context.mjs` fixes the following source
contract:

- owner operation assignment precedes policy admission;
- the shared helper performs the unchanged read-policy check;
- rejected admission logs the complete available `PortContext` and original typed
  error fields;
- the original error is rethrown;
- existing tax service mapping and public validation/result envelopes remain intact;
- direct admission before owner-operation assignment is forbidden.

`scripts/verify/verify-tax-calculation-local-context.mjs` separately guards the
canonical root wrapper, safe request-shape facts, exact stable post-delegation
classification, same-error return, and the legacy module-path compatibility boundary.

## Validation status

Tests, Cargo commands, formatting commands, verifier execution, workflow checks, and
CI were not run by the implementation agent, per maintainer instruction.

Suggested focused checks:

```bash
node scripts/verify/verify-tax-calculation-local-context.mjs
node scripts/verify/verify-tax-calculation-policy-context.mjs
node scripts/verify/verify-tax-calculation-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-tax --lib
cargo check -p rustok-cart --lib
```

## Remaining work

This does not close direct legacy-module callers, consumer-side tax transport context
retention, promotion cleanup, mounted checkout compensation, external-provider runtime
evidence, or the remaining customer, promotion, and ecommerce adapter mapper work.
Architecture status must not be promoted from source inspection alone.
