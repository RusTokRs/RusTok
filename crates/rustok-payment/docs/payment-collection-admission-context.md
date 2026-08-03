# Payment collection owner admission diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens only admission diagnostics in
`PaymentCollectionPort`:

- `create_or_reuse_collection` write-policy admission;
- `create_or_reuse_collection` write-semantics admission;
- `read_collection_status` read-policy admission.

The public operations, policy selection, write-semantics requirement, tenant parsing, owner
service calls, and public `PortError` behavior remain unchanged.

## Preserved admission flow

`create_or_reuse_collection` still evaluates:

1. write policy;
2. write semantics;
3. tenant parsing;
4. reusable collection lookup and owner execution.

`read_collection_status` still evaluates:

1. read policy;
2. tenant parsing;
3. owner status read.

The exact original admission `PortError` continues through `inspect_err` unchanged. The
diagnostic helper does not construct a replacement code, message, kind, or retryability value.

## Bounded diagnostic policy

Admission diagnostics retain only bounded context and message-shape facts.

The helper records one closed error-kind label:

- `validation`;
- `not_found`;
- `conflict`;
- `forbidden`;
- `unavailable`;
- `timeout`;
- `invariant_violation`.

Both severity paths retain:

- truthful owner `rustok_payment`;
- exact owner operation;
- exact admission phase `policy` or `write_semantics`;
- boundary `payment_collection_port`;
- correlation id;
- stable original error code and retryability;
- internal-message presence and character length;
- tenant-id and actor-id character lengths;
- closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds.

The helper does not record the complete admission `PortError`, human-readable internal message,
raw tenant, actor, channel, locale, causation, traceparent, or idempotency values, or Debug
`PortErrorKind` output.

## Preserved severity

Unavailable, timeout, and invariant-violation failures continue through `tracing::error!`.
Validation, not-found, conflict, forbidden, and other ordinary admission rejections continue
through `tracing::warn!`.

## Preserved behavior

This slice does not change:

- public `PaymentCollectionPort` trait signatures;
- create/reuse or status request and response DTOs;
- canonical owner operation selection;
- read/write policy selection;
- write-semantics requirements;
- admission-before-tenant-parsing ordering;
- tenant UUID parsing or its validation envelope;
- reusable collection lookup, create, or race-adoption behavior;
- collection status reads or snapshot conversion;
- `PaymentError` variant mapping;
- provider, lifecycle, reconciliation, configuration, or database public envelopes;
- checkout execution or compensation consumers;
- Commerce orchestration;
- ecommerce audit, Payment FFA, or Payment FBA status.

## Static evidence

The focused guard is:

- `scripts/verify/verify-payment-collection-admission-context.mjs`.

It requires:

- one read and one write admission helper;
- three preserved diagnostic call sites through `inspect_err`;
- exact owner-operation and admission-phase attribution;
- the closed error-kind classification;
- bounded message and delegated-context facts;
- technical-versus-ordinary severity;
- original-error pass-through and admission-before-tenant ordering;
- unchanged collection lookup, create, adoption, status-read, and snapshot behavior;
- absence of complete errors, raw context values, raw message text, and Debug kind output inside
  the covered helper.

Source evidence is recorded in:

- `crates/rustok-payment/contracts/evidence/payment-collection-admission-diagnostic-safety-source.json`.

The evidence remains source-only: `execution` is empty and every validation flag is false.

## Remaining diagnostic boundaries

Tenant UUID parsing and canonical `PaymentError` mapping remain separate cleanup slices. The
current tenant parser still records its complete parse cause, constructed `PortError`, raw
context, message text, and Debug kind. The canonical mapper still contains raw validation,
lifecycle, provider, identifier, database, and tenant diagnostics; some not-found public
envelopes also still interpolate internal UUIDs.

Compile, runtime, replay, restart, remote-port parity, workflows, CI, and production evidence
remain open. The broad ecommerce correlation-safe mapper cleanup remains open, and no FFA/FBA
status is promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
