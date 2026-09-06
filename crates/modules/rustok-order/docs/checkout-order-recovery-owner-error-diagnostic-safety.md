# Checkout order recovery owner error diagnostic safety

Status: **source-ready / unvalidated**

## Boundary

This source slice hardens only the `order_error_to_port_error` mapper in
`checkout_order_recovery.rs`.

The mapper is used by checkout recovery order loading, pending-order confirmation, and localized
projection reloads. It converts the seven `OrderError` variants to the existing recovery-facing
`PortError` contracts.

Before this slice, mapper diagnostics could record complete database or core errors, validation
and transition text, order/return/change UUIDs, raw tenant identity, and raw channel context.

## Bounded owner diagnostics

The mapper now derives one closed error variant:

- `database`;
- `order_not_found`;
- `validation`;
- `invalid_transition`;
- `order_return_not_found`;
- `order_change_not_found`;
- `core`.

It records only:

- the closed variant;
- text-field count and aggregate character length;
- UUID-field count and non-nil count;
- opaque-payload presence for database and core variants;
- truthful owner and exact recovery operation;
- correlation ID;
- bounded tenant, actor, channel, locale, causation, trace, idempotency, and deadline facts;
- stable mapper code;
- boundary `checkout_order_recovery_adapter`.

It does not record the complete `OrderError`, database/core text, validation text, transition
values, resource UUID values, raw tenant identity, raw actor identity, raw channel, or other raw
`PortContext` values.

Database and core failures remain `tracing::error!`; validation, not-found, transition, return, and
change outcomes remain `tracing::warn!`.

## Preserved public mapping

The seven conversion arms retain their existing contracts:

- database: `order.database_unavailable`, unavailable, `order storage is temporarily unavailable`;
- order not found: `order.order_not_found`, not found, `order was not found`;
- validation: `order.checkout_recovery_validation`, validation,
  `checkout order recovery request is invalid`;
- transition: `order.checkout_recovery_state_conflict`, conflict,
  `order lifecycle transition conflicts with checkout recovery`;
- order return/change not found: `order.related_resource_not_found`, not found,
  `related order resource was not found`;
- core: `order.invariant_violation`, invariant violation,
  `order operation failed an internal invariant`.

Retryability remains defined by the same `PortError` constructors.

## Deliberate boundary

This source slice does not change:

- admission or write semantics;
- tenant, actor, or causation parsing;
- request/hash encoding;
- hash normalization;
- durable identity read or legacy adoption;
- identity comparison diagnostics;
- cancelled/unknown lifecycle diagnostics;
- order loading, confirmation, or locale fallback;
- request/response types;
- Commerce orchestration;
- Order FFA/FBA status.

Those non-mapper recovery diagnostics still contain separate raw payload boundaries and remain open
for later bounded slices. The master ecommerce correlation-safe mapper-cleanup item therefore
remains open.

## Evidence

- `crates/rustok-order/contracts/evidence/checkout-order-recovery-owner-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-order-checkout-recovery-owner-error-diagnostic-safety.mjs`

Evidence is source-only. `execution` is empty and every validation flag remains false.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-order-checkout-recovery-owner-error-diagnostic-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-order --lib
```
