# Public ecommerce port error safety

Status: `source_ready_unvalidated`

This source wave closes the public transport leak for technical owner-port errors
without claiming that every ecommerce owner mapper has correlation-aware internal
logging.

## Enforced invariant

`rustok-api::PortError` treats these kinds as technical and fail-closed:

- `Unavailable` always exposes `the requested capability is temporarily unavailable`.
- `InvariantViolation` always exposes `the requested operation could not be completed safely`.

The rule is applied in all three boundary locations:

1. `PortError::new` and the typed constructors.
2. custom `Serialize` implementation before a port error crosses a transport.
3. custom `Deserialize` implementation when a remote port error enters a consumer.

A caller cannot bypass transport sanitization by mutating `PortError.message` or by
returning a remote payload containing raw SQL, SDK, provider, stack, or invariant text.
Validation, not-found, conflict, forbidden, and explicit timeout messages remain
available only for actionable domain errors.

## Owner mapper hardening present

- `rustok-channel`: database and serialization causes are logged internally and mapped
  to stable public messages.
- `rustok-region`: database causes are logged internally and mapped to a stable public
  message.
- `rustok-cart` checkout snapshot: validator and serialization causes are logged
  internally; request/projection and encoding failures use stable public messages.
- `rustok-cart` promotion guard: target validation, tenant parsing, rejected call
  contexts, and owner service failures retain the cart-promotion owner, correlation id,
  tenant, channel, operation, stable code, and original internal cause. Existing public
  validation, not-found, conflict, tax-boundary, and unavailable messages remain static.
- `rustok-pricing`: every read/write owner-port mapper receives the `PortContext` and
  operation name. Database, rich, and core causes are logged with `correlation_id`,
  tenant, operation, and stable code; public messages are stable. Pricing validation
  cause text stays internal and the public boundary returns `pricing request is invalid`.
- `rustok-payment` collection port: create/reuse and status reads pass the request
  context and owner operation into the mapper. Database, validation, transition, and
  provider causes are logged with correlation identity; raw provider ids, lifecycle
  strings, and storage causes do not appear in the public message.
- `rustok-fulfillment` checkout execution: ensure/read storage calls and idempotent
  adoption lookups pass the original `PortContext` and owner operation into the mapper.
  Validation, missing-resource, transition, and database causes are logged with
  correlation identity and stable codes while the existing public envelopes remain
  static.
- `rustok-commerce` admin order detail payment lookup: the complete typed payment cause
  remains internal while owner, tenant, order, operation, error kind, stable public code,
  status, and HTTP boundary are logged. Validation, missing-resource, transition,
  provider, reconciliation, configuration, and storage outcomes preserve the existing
  static public messages and status policy.
- `rustok-commerce` admin order detail fulfillment lookup: the typed fulfillment cause
  remains internal while owner, tenant, order, operation, error kind, stable public code,
  status, and HTTP boundary are logged. Validation, not-found, transition, and storage
  outcomes use static public messages instead of the shared dynamic validation envelope.
- `rustok-order` checkout payment settlement: identity absence/mismatch, lifecycle and
  payment-reference conflicts, owner-context validation, missing resources, storage,
  transition, validation, and core causes retain the settlement owner, correlation id,
  tenant, channel, operation, stable code, and reconciliation evidence. Public
  validation, not-found, conflict, unavailable, and invariant envelopes remain static.
- `rustok-order` checkout recovery: read identity absence, immutable identity mismatch,
  cancelled/unknown lifecycle states, owner-context validation, request/hash encoding,
  missing resources, storage, transition, validation, and core causes retain the
  recovery owner, correlation id, tenant, channel, operation, stable code, and
  reconciliation evidence. Raw hash values remain private and public validation,
  not-found, conflict, unavailable, and invariant envelopes remain static.
- `rustok-order` checkout compensation: invalid request and causation context, durable
  identity conflicts, cancellation races, effectful or unknown lifecycle states,
  owner-context validation, missing resources, storage, transitions, validation, and
  core causes retain the compensation owner, correlation id, tenant, channel, operation,
  stable code, typed lifecycle state, and truthful optional order identity. Public
  validation, not-found, conflict, unavailable, and invariant envelopes remain static.
- `rustok-tax` calculation port: request-validation details, provider-result contract
  violations, and owner validation causes remain internal. Every mapper records owner,
  correlation id, tenant, channel, operation, and stable code while public validation
  and invariant messages remain static.

## Still open

- Audit remaining order adapters, remaining fulfillment adapters, inventory, customer,
  remaining promotion adapters/transports, payment execution/compensation, remaining tax
  adapters/transports, and remaining ecommerce adapters for technical text mislabeled as
  validation/conflict errors.
- Add structured owner-side logging with `correlation_id`, owner operation, stable error
  code, and the original cause before every remaining technical `PortError` mapping.
- Remove raw technical text from non-`PortError` public REST, GraphQL, native, and
  operator error envelopes.
- Add compile and transport round-trip evidence before changing any FBA/FFA status.

## Verification

- `node scripts/verify/verify-port-error-public-safety.mjs`
- `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `node scripts/verify/verify-cart-promotion-port-error-safety.mjs`
- `node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs`
- `node scripts/verify/verify-commerce-admin-order-detail-payment-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-detail-fulfillment-error-safety.mjs`
- `node scripts/verify/verify-order-payment-settlement-error-context.mjs`
- `node scripts/verify/verify-order-checkout-recovery-error-context.mjs`
- `node scripts/verify/verify-order-checkout-compensation-error-context.mjs`
- `node scripts/verify/verify-tax-calculation-error-context.mjs`
- `cargo test -p rustok-api ports::tests`
- `cargo check -p rustok-cart --all-features`
- `cargo check -p rustok-order --all-features`
- `cargo check -p rustok-pricing --all-features`
- `cargo check -p rustok-payment --all-features`
- `cargo check -p rustok-fulfillment --all-features`
- `cargo check -p rustok-tax --all-features`
- Targeted cart promotion, order payment settlement, order checkout recovery, order
  checkout compensation, pricing, payment collection, fulfillment checkout execution,
  admin order-detail payment and fulfillment mapping, and tax calculation validation,
  provider-contract, reconciliation, correlation, HTTP-envelope, and transport
  round-trip tests.

No verification command above was executed as part of this source wave.
