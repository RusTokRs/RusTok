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

## Confirmed gaps

The guarded port already returned bounded public messages and shape-only context,
request, and owner-error fields. Two diagnostic details still violated the stricter
ecommerce mapper contract:

- tenant UUID rejection logged the parser cause as `parse_error = ?error`;
- policy and mapped owner events emitted `PortErrorKind` through debug formatting.

The parser cause is not required to distinguish an invalid tenant context, and debug
formatting is not a closed diagnostic contract.

## Safe context and request shape

The guarded boundary retains the per-call correlation id and records only:

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

Tenant UUID rejection now records `tenant_id_parse_failed = true` together with the
existing tenant length, operation, correlation, code, and boundary. The UUID parser
cause is not copied into the event.

## Safe error shape

Policy and owner-result diagnostics retain:

- stable internal/public code;
- message presence and length where applicable;
- retryability;
- one closed error-kind label:
  - `validation`;
  - `not_found`;
  - `conflict`;
  - `forbidden`;
  - `unavailable`;
  - `timeout`;
  - `invariant_violation`.

They do not use debug formatting for `PortErrorKind`.

The owner mapper also classifies `CartError` structurally and records only:

- owner error variant;
- validation-detail presence and length;
- missing-resource UUID non-nil facts;
- transition-state string lengths;
- database-error presence;
- tax code/message presence and lengths.

The complete `CartError`, database text, validation text, lifecycle values, tax
message, and missing-resource UUID are not written by this boundary.

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
- technical-versus-ordinary diagnostic severity;
- the legacy `CartPromotionPort for CartService` compatibility source;
- Cart or ecommerce FFA/FBA status.

## Static evidence

`scripts/verify/verify-cart-promotion-port-error-safety.mjs` locks:

- context and request fact capture before unchanged owner delegation;
- safe context shape across validation, tenant parsing, policy rejection, and owner
  mapping;
- a boolean tenant-parse failure fact with no parser cause;
- the closed seven-value `PortErrorKind` mapper across policy and owner events;
- safe request and owner-error shape;
- technical versus ordinary diagnostic severity;
- unchanged public error mapping and return order;
- absence of raw context, identifiers, source, amount, metadata, message, parser cause,
  debug kind, and complete owner-error fields.

Retained source and review evidence remain at:

- `crates/rustok-cart/contracts/evidence/cart-promotion-diagnostic-safety-source.json`;
- `crates/rustok-cart/contracts/evidence/cart-promotion-diagnostic-safety-source-review.json`.

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
