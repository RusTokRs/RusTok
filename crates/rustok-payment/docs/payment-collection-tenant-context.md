# Payment collection tenant diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source-only contract hardens invalid tenant UUID diagnostics in
`PaymentCollectionPort::create_or_reuse_collection` and
`PaymentCollectionPort::read_collection_status`.

Both operations still select their canonical owner operation, complete admission, and then call the
same payment-owned tenant parser before any owner storage or service work.

## Preserved parser flow

The parser still calls `Uuid::parse_str(&context.tenant_id)`, constructs the same validation
`PortError` on failure, emits one warning, and returns that same constructed error.

The validation envelope remains:

- code `payment.tenant_id_invalid`;
- message `PortContext.tenant_id must be a UUID for payment ports`;
- validation kind and existing retryability.

The same constructed validation `PortError` is returned after diagnostics.

## Bounded diagnostic policy

Tenant diagnostics retain only the parse-error type and bounded context/message-shape facts:
correlation, exact owner operation, validation phase, actor kind, identity/text lengths, optional
field presence, claim/role counts, deadline, stable code, message presence/length, closed kind, and
boundary.

The parser does not record the complete parse error, the constructed `PortError`, raw context
values, internal message text, or Debug kind output.

## Related source-only contracts

Admission diagnostics remain closed by
`scripts/verify/verify-payment-collection-admission-context.mjs`.

Canonical `payment_error_to_port_error` is now closed by a separate source-only contract:

- verifier: `scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs`;
- evidence: `crates/rustok-payment/contracts/evidence/payment-collection-owner-error-diagnostic-safety-source.json`;
- documentation: `crates/rustok-payment/docs/payment-collection-owner-error-diagnostic-safety.md`.

The owner contract removes raw validation, transition, provider, database, context, and identifier
payloads while preserving stable codes/kinds/retryability and using static not-found messages.

## Preserved behavior

No trait, DTO, admission policy, collection lookup/create/race adoption/status behavior, provider
execution, persistence, checkout/compensation consumer, Commerce orchestration, or FFA/FBA status is
changed.

Execution evidence remains empty; compile, verifier, runtime, replay, restart, remote-port,
workflow, CI, and production evidence remain open.

## Suggested maintainer checks

```bash
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-payment-collection-owner-error-diagnostic-safety.mjs
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
