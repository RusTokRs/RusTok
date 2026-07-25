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
- `rustok-commerce` storefront staged checkout recovery: the typed recovering-checkout
  cause is emitted only through structured logging with owner, correlation id, tenant,
  channel, actor, cart, operation, error kind, stable public code, retryability, and
  runtime boundary. Raw `eprintln!` debug output is removed while reconciliation,
  compensation-pending, temporary-unavailable, and checkout-failed public outcomes remain
  unchanged across REST, GraphQL, native, and mounted transports.
- `rustok-commerce` storefront checkout HTTP completion: the typed staged-runtime error
  is mapped with tenant, actor, cart, channel id/slug, locale, exact route operation,
  error kind, stable public code, retryability, status, and HTTP boundary. Validation,
  cart-access, authentication, temporary-unavailable, checkout-failed,
  compensation-pending, and reconciliation-required status/code/message contracts remain
  unchanged while idempotency validation, checkout input forwarding, provider registry,
  staged runtime arguments, and the response contract stay intact.
- `rustok-commerce` storefront payment-collection creation: reusable lookup and create
  calls retain the typed payment cause with payment owner, tenant, actor, cart, truthful
  optional customer identity, channel id/slug, locale, exact operation, error kind,
  stable public code, status, and HTTP boundary. Validation, missing-resource,
  transition, provider-unavailable/configuration/rejected, reconciliation, and storage
  outcomes preserve the existing public envelopes while cart access, repricing, context
  metadata, service arguments, reusable response, and created response remain unchanged.
- `rustok-commerce` admin fulfillment reconciliation: list, quarantine, manual resolve,
  and retry paths retain the typed fulfillment or orchestration cause with owner, tenant,
  truthful optional provider-operation identity, operation, stable code, status, and HTTP
  boundary. Provider-result encoding failures are internal invariant failures with a
  static fail-closed `500` envelope instead of dynamic serialization text in a `400`.
- `rustok-commerce` admin fulfillment routes: list, create, detail, ship, deliver, reopen,
  reship, and cancel paths retain typed owner or orchestration causes with owner, tenant,
  route operation, truthful optional fulfillment and order identities, stable public code,
  status, and HTTP boundary. Persisted reconciliation errors adopt the fulfillment id from
  the typed orchestration error while validation and technical details stay internal.
- `rustok-commerce` admin shipping-option routes: list, create, detail, update, deactivate,
  and reactivate paths map the typed `FulfillmentError` locally with owner, tenant, route
  operation, truthful optional shipping-option identity, stable public code, status, and
  HTTP boundary. Validation details remain internal while the existing not-found,
  transition, and storage status/code policy stays unchanged.
- `rustok-commerce` admin order routes: list, detail, mark-paid, ship, deliver, and cancel
  paths map the typed `OrderError` locally with owner, tenant, actor, route operation,
  truthful optional order and customer identities, stable public code, status, and HTTP
  boundary. Typed missing-order identities are adopted from the error while locale
  fallback, filters, lifecycle inputs, and the existing static response policy stay
  unchanged.
- `rustok-commerce` admin checkout-operation routes: detail, compensation, and compensation
  sweep retain the typed operation, compensation, payment, payment-orchestration, order,
  reservation-journal, or storage cause with owner, source owner, tenant, actor, route
  operation, and truthful optional checkout, reservation, payment, refund, and order
  identities. Dynamic validation/conflict details remain internal, all current
  compensation variants have explicit HTTP and sweep-code policies, and existing route,
  provider-registry, worker, limit, and response contracts stay unchanged.
- `rustok-commerce` admin payment routes: payment collection and refund lists/details plus
  authorize, capture, cancel, create-refund, complete-refund, and cancel-refund operations
  retain the typed payment or payment-orchestration cause with owner, source owner, tenant,
  actor, route operation, and truthful optional collection, refund, order, cart, and
  customer identities. Validation, provider, reconciliation, configuration, and storage
  details stay internal while the existing status/code/message policy, idempotency header,
  provider registry, filter, pagination, service, and response contracts remain unchanged.
- `rustok-commerce` admin product routes: list count/page/translation/tag reads plus detail,
  create, update, delete, publish, and unpublish owner calls retain the typed commerce cause
  with tenant, actor, exact owner operation, and truthful optional product and variant
  identities. Product create/update shipping-profile prevalidation now retains the typed
  commerce cause with tenant, actor, exact validation operation, and truthful optional
  product and shipping-profile identities. Database, validation, conflict, inventory,
  state, and fail-closed outcomes preserve the existing static HTTP policy while locale
  fallback, filters, metrics, pagination, slug normalization, validation service arguments,
  catalog service arguments, and responses stay unchanged.
- `rustok-commerce` admin order-change owner routes: create, list, detail, and cancel paths
  map the typed `OrderError` locally with owner, tenant, route operation, truthful optional
  order and order-change identities, stable public code, status, and HTTP boundary. Typed
  missing-resource identities are adopted from the error while validation, conflict,
  storage, and fail-closed responses preserve the existing static policy.
- `rustok-commerce` admin order-change orchestration route: apply retains the top-level
  typed post-order cause with orchestration and source owner, tenant, actor, route
  operation, truthful optional order, order-change, payment-collection, payment, and
  reserved-refund identities, stable public code, status, and HTTP boundary. Nested order,
  payment, provider, validation, and reserved-refund outcomes preserve the existing static
  status, code, and message policy without delegating through a second generic error event.
- `rustok-commerce` admin order-return owner routes: create, list, detail, and cancel paths
  map the typed `OrderError` locally with owner, tenant, route operation, truthful optional
  order and return identities, stable public code, status, and HTTP boundary. Validation,
  not-found, conflict, storage, and fail-closed outcomes preserve the existing static
  status, code, and message policy without a second generic error event.
- `rustok-commerce` admin order-return orchestration routes: decision and complete paths
  retain the top-level typed post-order cause with orchestration and source owner, tenant,
  actor, route operation, truthful optional order, return, and reserved refund identities,
  stable public code, status, and HTTP boundary. Nested order, payment, provider, and
  reserved-refund outcomes preserve the existing static status, code, and message policy
  without delegating through a second generic error event.
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
- `node scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs`
- `node scripts/verify/verify-commerce-storefront-checkout-http-error-context.mjs`
- `node scripts/verify/verify-commerce-storefront-payment-collection-error-context.mjs`
- `node scripts/verify/verify-cart-promotion-port-error-safety.mjs`
- `node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs`
- `node scripts/verify/verify-commerce-admin-fulfillment-reconciliation-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-fulfillment-route-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs`
- `node scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-route-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-payment-list-http-error-safety.mjs`
- `node scripts/verify/verify-commerce-admin-product-route-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-product-shipping-profile-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-change-owner-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-change-orchestration-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-return-owner-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-order-return-orchestration-error-context.mjs`
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
- Targeted cart promotion, storefront staged checkout recovery, HTTP completion and
  payment-collection mapping, order payment settlement, order checkout recovery, order
  checkout compensation, pricing, payment collection, fulfillment checkout execution,
  admin fulfillment reconciliation, admin fulfillment routes, admin shipping-option,
  admin order-route, admin checkout-operation, admin payment-route, admin product-route and
  product shipping-profile prevalidation, order-change owner and orchestration mapping,
  admin order-return owner and orchestration mapping, admin order-detail payment and
  fulfillment mapping, and tax calculation validation, provider-contract, reconciliation,
  correlation, HTTP-envelope, and transport round-trip tests.

No verification command above was executed as part of this source wave.
