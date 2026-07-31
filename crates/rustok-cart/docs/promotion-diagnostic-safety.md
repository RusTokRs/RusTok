# Cart promotion diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This source slice hardens the canonical guarded `CartPromotionPort` used by Commerce
admin promotion preview and application:

- `read_cart_promotion_preview`;
- `apply_cart_promotion`;
- read/write policy rejection;
- promotion target validation;
- tenant parsing;
- owner `CartError` mapping.

The owner calls, promotion selection, request DTO, public `PortError` policy, and
legacy `CartService` compatibility implementation remain unchanged.

## Confirmed gap

The guarded port already returned bounded public messages, but its diagnostics copied
full tenant, actor, channel, locale, causation, traceparent, idempotency, cart and line
item identities, promotion source, monetary amount, metadata, lifecycle strings, tax
messages, validation details, and the complete `CartError` into structured events.

Those values were not needed to classify or correlate the failure.

## Safe context and request shape

The guarded boundary now retains the per-call correlation id and records only:

- tenant, actor, channel, locale, causation, traceparent, and idempotency lengths or
  presence facts;
- actor kind, claim count, role count, and deadline;
- cart and optional line-item UUID non-nil facts;
- typed promotion scope and kind;
- source-id presence and character length;
- decimal text length rather than the amount;
- metadata JSON kind and bounded collection/string size.

It does not record raw tenant, actor, channel, locale, causation, traceparent,
idempotency, cart, line-item, source-id, amount, or metadata values.

## Safe owner error shape

The owner mapper classifies `CartError` structurally and records only:

- owner error variant;
- validation-detail presence and length;
- missing-resource UUID non-nil facts;
- transition-state string lengths;
- database-error presence;
- tax code/message presence and lengths;
- stable owner/public codes, typed public kind, and retryability.

The complete `CartError`, database text, validation text, lifecycle values, tax
message, and missing-resource UUID are no longer written by this boundary.

## Preserved behavior

This change does not alter:

- `CartPromotionPort`, `CartPromotionRequest`, or response DTOs;
- preview versus apply policy admission;
- scope/line-item validation rules;
- tenant UUID parsing or its public envelope;
- promotion service method selection and argument order;
- percentage/fixed and cart/line-item/shipping behavior;
- metadata forwarding on apply;
- public validation, not-found, conflict, tax-boundary, unavailable, timeout, kind,
  code, message, or retryability contracts;
- the legacy `CartPromotionPort for CartService` compatibility source;
- Cart or ecommerce FFA/FBA status.

## Static evidence

`scripts/verify/verify-cart-promotion-port-error-safety.mjs` now locks:

- context and request fact capture before unchanged owner delegation;
- safe context shape across validation, tenant parsing, policy rejection, and owner
  mapping;
- safe request and owner-error shape;
- technical versus ordinary diagnostic severity;
- unchanged public error mapping and return order;
- absence of raw context, identifiers, source, amount, metadata, message, and complete
  owner-error fields.

## Validation boundary

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed.
Source evidence does not prove compilation, preview/application behavior, mounted admin
transport, browser/SSR parity, database failure behavior, remote ports, or production
operation.

Suggested maintainer commands:

```bash
node scripts/verify/verify-cart-promotion-port-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-commerce-admin --all-features
```

The broad ecommerce correlation-safe mapper cleanup remains open for remaining owner,
consumer, transport, and non-`PortError` envelopes.
