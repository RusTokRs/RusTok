# Checkout order recovery admission diagnostic safety

Status: `source_reviewed_unvalidated`

This continuation closes only the request-context admission diagnostics in
`checkout_order_recovery.rs`:

- `require_operation_context`;
- `parse_tenant_id`;
- `parse_actor_id`.

The broad ecommerce correlation-safe mapper cleanup remains open.

## Previous exposure

The three helpers retained stable public `PortError` envelopes, but their
warning logs included request-owned values or internal parser details:

- the complete tenant identifier;
- the complete actor tenant context through the tenant field;
- the complete channel value;
- the complete causation identifier;
- the expected checkout operation UUID;
- the debug representation of UUID parse failures.

Those fields could carry request or identity payload across a public adapter
boundary even though only admission shape is required for diagnosis.

## Source change

All three helpers now delegate rejection logging to
`log_checkout_order_recovery_admission_rejection`.

The logger retains:

- owner and static operation;
- correlation identifier;
- bounded `PortContext` shape facts already shared by the owner-error mapper;
- rejected field name;
- field presence and character length;
- UUID parseability;
- optional non-nil shape for the parsed and expected UUID;
- optional equality outcome;
- stable code and adapter boundary.

It does not retain the rejected field value, UUID parser error, parsed UUID, or
expected operation UUID.

## Preserved contracts

No request ordering or owner operation changed. Recovery still validates:

1. read/write policy and write semantics;
2. tenant UUID;
3. actor UUID for recovery writes;
4. causation UUID equality with `checkout_operation_id`;
5. hash evidence, owner identity, owner lifecycle, and projection loading.

The exact public envelopes remain:

- `order.checkout_operation_id_invalid` / validation /
  `checkout operation context is invalid`;
- `order.tenant_id_invalid` / validation /
  `order request context is invalid`;
- `order.actor_id_invalid` / validation /
  `order request context is invalid`.

Warning severity, correlation, owner operation, and boundary remain available.

## Deliberately open

This slice does not change diagnostics for:

- identity conflict comparison;
- request/hash serialization and canonical encoding;
- hash normalization;
- read projection not-found;
- cancelled or unknown lifecycle states;
- the already bounded owner `OrderError` mapper.

Those boundaries must be reviewed independently before the master ecommerce
cleanup item can close.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were
executed. The accompanying verifier is retained source contract only and was
not run. No compile or runtime status is promoted from this review.
