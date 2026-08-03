# Fulfillment checkout owner admission diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens the read/write admission diagnostics for the two operations
published by `CheckoutFulfillmentExecutionPort` in
`crates/rustok-fulfillment/src/checkout_execution.rs`:

- `ensure_checkout_fulfillments`;
- `read_checkout_fulfillments`.

The public methods, helper signatures, policy selection, write-semantics requirement, and
execution ordering remain unchanged.

## Preserved admission flow

`ensure_checkout_fulfillments` still evaluates:

1. write policy;
2. write semantics;
3. tenant parsing;
4. checkout causation validation;
5. owner execution.

`read_checkout_fulfillments` still evaluates:

1. read policy;
2. tenant parsing;
3. checkout causation validation;
4. owner execution.

Admission failures continue through `inspect_err`, so the original `PortError` is returned by
the policy or write-semantics result without reconstruction. The diagnostic helper does not
select a replacement code, message, kind, or retryability value.

## Bounded diagnostic policy

Admission diagnostics record only a closed error-kind label:

- `validation`;
- `not_found`;
- `conflict`;
- `forbidden`;
- `unavailable`;
- `timeout`;
- `invariant_violation`.

They retain:

- truthful owner `rustok_fulfillment`;
- exact owner operation;
- exact phase `policy` or `write_semantics`;
- boundary `checkout_fulfillment_execution_port`;
- correlation id;
- stable internal code and retryability;
- message presence and character length;
- tenant-id and actor-id character lengths;
- a closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds.

The complete admission `PortError` is not logged. Human-readable message text is not logged.
Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are not
recorded by this mapper.

## Preserved severity

Unavailable, timeout, and invariant-violation failures continue through `tracing::error!`.
Validation, not-found, conflict, forbidden, and other ordinary admission rejections continue
through `tracing::warn!`.

Both paths preserve the exact owner operation, phase, correlation id, code, closed kind label,
retryability, boundary, and bounded context/message facts.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write policy selection;
- write-semantics requirements;
- admission-before-tenant-parsing ordering;
- tenant UUID parsing;
- checkout-operation causation validation;
- local request, identity, set, or immutable-plan validation;
- fulfillment creation, post-error adoption, lookup, read, or sorting behavior;
- canonical `FulfillmentError` public mappings;
- metadata construction;
- Commerce orchestration;
- FBA, FFA, or ecommerce audit status.

## Static evidence

The focused guard is:

- `scripts/verify/verify-fulfillment-checkout-admission-context.mjs`.

It requires:

- one read and one write admission helper;
- read policy, write policy, and write-semantics interception through `inspect_err`;
- exact owner-operation and phase attribution;
- the closed error-kind classification;
- bounded message and delegated-context facts;
- technical-versus-ordinary severity;
- original-error pass-through and admission-before-tenant-parsing ordering;
- absence of complete errors, raw context values, raw message text, and Debug kind output;
- preservation of local, tenant, causation, service, and owner mapper boundaries.

Source evidence is recorded in:

- `crates/rustok-fulfillment/contracts/evidence/checkout-admission-diagnostic-safety-source.json`.

The previously closed local-`PortError` contract is synchronized to treat admission cleanup as
a separate source-ready/unvalidated contract rather than an open unsafe payload site.

## Remaining diagnostic boundaries

Causation validation, tenant parsing, and canonical `FulfillmentError` diagnostics remain separate
bounded slices. This change does not claim that the complete `checkout_execution.rs` diagnostic
surface is source-closed.

Compile, runtime, replay, restart, contention, mounted Commerce behavior, remote-port parity,
workflows, CI, and production evidence remain open. The broad ecommerce correlation-safe
mapper cleanup and FFA/FBA status are not promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-local-porterror-diagnostic-safety.mjs
node scripts/verify/verify-fulfillment-checkout-local-validation-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
cargo check -p rustok-fulfillment --lib
```
