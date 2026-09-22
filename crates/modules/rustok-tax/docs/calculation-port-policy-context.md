# Tax calculation port policy context

Status: **source-ready / unvalidated**

## Scope

The canonical `TaxCalculationPort::calculate_tax` implementation assigns the truthful
`calculate_tax` operation before applying `PortCallPolicy::read()`. Admission failures
are inspected, diagnosed, and returned unchanged.

This slice keeps that ordering while closing the remaining policy-admission diagnostic
payload gap.

## Safe policy diagnostics

Policy rejection events retain:

- owner `rustok_tax`;
- operation `calculate_tax`;
- boundary `tax_calculation_port`;
- correlation id;
- stable error code, a closed typed-kind label, and retryability;
- error-message presence and character length;
- tenant and actor-id lengths;
- actor kind, claim count, and role count;
- channel, causation, traceparent, and idempotency presence plus lengths;
- locale length and deadline.

They do not record:

- the complete `PortError` through Debug or Display formatting;
- public/internal error-message text;
- debug-formatted `PortErrorKind` output;
- raw tenant, actor id, channel, locale, causation id, traceparent, or idempotency
  key values.

The original typed `PortError` is still returned to the caller unchanged. It is not
copied into the diagnostic event.

Unavailable, timeout, and invariant failures retain error severity. Ordinary policy or
validation rejections retain warning severity.

## Preserved behavior

The change does not alter:

- read/deadline admission semantics;
- tax request or result DTOs;
- validation order;
- `TaxService::calculate` delegation;
- provider selection or result validation;
- public error codes, messages, kinds, retryability, or return paths;
- runtime composition, FBA, or FFA status.

## Static evidence

`scripts/verify/verify-tax-calculation-policy-context.mjs` requires operation assignment
before policy admission, the unchanged read policy, safe context-shape diagnostics,
closed kind classification, message shape, and unchanged public envelopes. It forbids
complete `PortError` formatting, raw message text, debug-kind formatting, and raw
context values.

The owner and wrapper result paths remain guarded by:

- `scripts/verify/verify-tax-calculation-error-context.mjs`;
- `scripts/verify/verify-tax-calculation-local-context.mjs`.

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

Consumer-side transport context, external-provider runtime evidence, remote profiles,
compile evidence, and the remaining ecommerce mapper cleanup stay open. Architecture
status is not promoted from source inspection alone.
