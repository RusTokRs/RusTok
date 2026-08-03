# Payment collection tenant diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This bounded source slice hardens only invalid tenant UUID diagnostics in
`PaymentCollectionPort`:

- `create_or_reuse_collection` tenant parsing;
- `read_collection_status` tenant parsing.

Both public operations continue to select their canonical owner operation and complete admission
before invoking `parse_port_tenant_id`.

## Preserved parser flow

The payment-owned parser still:

1. calls `Uuid::parse_str(&context.tenant_id)`;
2. enters the same `map_err` path on parse failure;
3. constructs the exact same validation `PortError`;
4. emits one warning diagnostic;
5. returns that same constructed validation `PortError`.

The validation envelope remains:

- kind: validation;
- code: `payment.tenant_id_invalid`;
- message: `PortContext.tenant_id must be a UUID for payment ports`;
- retryability: unchanged.

The same constructed validation `PortError` is returned after diagnostics. No replacement error is
constructed after the warning event.

## Bounded diagnostic policy

Tenant diagnostics retain only the parse-error type and bounded context/message-shape facts.

The parser records:

- concrete parse-error type through `type_name_of_val`;
- explicit `tenant_id_parse_failed = true`;
- truthful owner `rustok_payment`;
- exact owner operation;
- validation phase `tenant_id`;
- boundary `payment_collection_port`;
- correlation id;
- tenant-id and actor-id character lengths;
- closed actor-kind label;
- claim and role counts;
- channel presence and optional character length;
- locale character length;
- causation-id, traceparent, and idempotency-key presence plus optional character lengths;
- optional deadline milliseconds;
- stable validation code and retryability;
- internal-message presence and character length;
- closed error-kind label `validation`.

The parser does not record:

- the complete UUID parse error;
- the complete constructed `PortError`;
- raw tenant, actor, channel, locale, causation, traceparent, or idempotency values;
- human-readable internal message text;
- Debug `PortErrorKind` output.

## Preserved behavior

This slice does not change:

- public `PaymentCollectionPort` trait signatures;
- create/reuse or status DTOs;
- canonical owner-operation selection;
- read/write admission or write-semantics behavior;
- admission-before-tenant ordering;
- the two operation-aware parser call sites;
- reusable collection lookup, create, or race-adoption behavior;
- collection status reads or snapshot conversion;
- the admission diagnostic mapper closed by the preceding source slice;
- canonical `PaymentError` variant mapping;
- provider, lifecycle, reconciliation, configuration, or database public envelopes;
- checkout execution or compensation consumers;
- Commerce orchestration;
- ecommerce audit, Payment FFA, or Payment FBA status.

## Static evidence

The focused guard is:

- `scripts/verify/verify-payment-collection-tenant-context.mjs`.

It requires:

- exactly two operation-aware parser call sites;
- operation selection and admission before tenant parsing;
- tenant parsing before owner storage or service work;
- exact validation code, message, kind, and retryability;
- type-only parse cause and explicit parse-failure fact;
- bounded context and message-shape facts;
- one warning path followed by return of the same constructed error;
- unchanged admission helpers and collection flow;
- explicit preservation of the canonical Payment mapper as a separate open boundary;
- absence of complete parse errors, complete `PortError`, raw context, message text, and Debug kind
  output inside the covered parser.

Source evidence is recorded in:

- `crates/rustok-payment/contracts/evidence/payment-collection-tenant-diagnostic-safety-source.json`.

The evidence remains source-only: `execution` is empty and every validation flag is false.

## Remaining diagnostic boundary

Canonical `payment_error_to_port_error` remains the next separate cleanup slice. It still contains
raw validation and transition text, provider identifiers and operations, database errors, raw
tenant values, and public not-found messages that interpolate owner UUIDs.

Compile, runtime, replay, restart, remote-port parity, workflows, CI, and production evidence
remain open. The broad ecommerce correlation-safe mapper cleanup remains open, and no FFA/FBA
status is promoted.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
