# Checkout payment execution UUID and serde diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

Checkout payment execution has two retained parsing boundaries in
`validation_errors.rs`:

- tenant context text is parsed as a UUID before owner access;
- a durable provider-operation JSON result is decoded into
  `PaymentProviderOperationResult` during recovery.

Both paths already returned sanitized public `PortError` values, but their private
tracing events recorded complete parser errors. Serde error text can include payload
fragments and structural detail, while UUID parser debug output is unnecessary for
correlation once the rejected input shape is known.

## Private diagnostics

The tenant UUID path now records only:

- a static parse-failure fact;
- tenant ID character length;
- owner operation, correlation ID, stable code, and boundary.

The durable provider-result path now records only:

- a static decode-failure fact;
- top-level JSON kind (`null`, `bool`, `number`, `string`, `array`, or `object`);
- array length or object field count when applicable;
- operation identity shape and existing bounded context facts;
- stable code, owner operation, correlation ID, and boundary.

Neither event records parser error text, tenant ID text, or provider-result payload.

## Preserved behavior

This source slice does not change:

- UUID parsing or accepted tenant IDs;
- the `payment.tenant_id_invalid` validation code or public message;
- provider-result presence and operation-status gates;
- `serde_json::from_value` decoding behavior;
- successful provider-result recovery;
- malformed-result routing to `manual_reconciliation`;
- the public manual-reconciliation `PortError` code, kind, message, or retryability;
- payment lifecycle, provider execution, journal, or recovery ordering.

## Remaining payment execution diagnostics

Separate cleanup remains open for:

- manual-reconciliation reason text;
- local persistence diagnostics;
- provider checkpoint diagnostics.

The canonical ecommerce correlation-safe mapper-cleanup item remains open.

## Evidence

- `contracts/evidence/checkout-execution-uuid-serde-diagnostic-safety-source.json`
- `scripts/verify/verify-payment-checkout-execution-uuid-serde-diagnostic-safety.mjs`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
payment owner tests, compilation, malformed durable JSON injection, and invalid tenant
UUID injection.

No payment FFA/FBA, runtime, workflow, CI, or production status is promoted from this
source review.
