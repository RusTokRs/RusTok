# Payment collection admission diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source-only contract covers read-policy admission for payment collection status and write
policy plus write-semantics admission for collection create/reuse.

Admission remains before tenant parsing and owner work. Read operations do not require write
semantics; create/reuse retains policy-before-write-semantics ordering.

## Bounded diagnostic policy

Admission diagnostics retain only bounded context and message-shape facts: truthful owner, exact
operation and admission phase, correlation, actor kind, identity/text lengths, optional-field
presence, claim/role counts, deadline, stable code, message presence/length, closed error kind,
retryability, and boundary.

They do not record complete `PortError` payloads, raw context values, internal message text, or Debug
kind output. Technical admission failures remain error severity; ordinary rejections remain warning
severity.

The exact original admission `PortError` continues through `inspect_err` unchanged.

## Related source-only contracts

Tenant UUID parsing and canonical `payment_error_to_port_error` are now closed by separate
source-only contracts:

- tenant verifier: `scripts/verify/verify-payment-collection-tenant-context.mjs`;
- owner mapper verifier: `scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs`.

The owner mapper contract also replaces identifier-bearing collection/payment/refund not-found
messages with static public envelopes while preserving codes, kinds, and retryability.

## Preserved behavior

No trait, DTO, collection lookup/create/race adoption/status flow, owner service call, tenant
validation envelope, provider execution, persistence, checkout/compensation consumer, Commerce
orchestration, or FFA/FBA status is changed by this admission contract.

Execution evidence remains empty; compile, verifier, runtime, replay, restart, remote-port,
workflow, CI, and production evidence remain open.

## Suggested maintainer checks

```bash
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
