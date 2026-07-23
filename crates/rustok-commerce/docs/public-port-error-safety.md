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
- `rustok-pricing`: every read/write owner-port mapper receives the `PortContext` and
  operation name. Database, rich, and core causes are logged with `correlation_id`,
  tenant, operation, and stable code; public messages are stable. Pricing validation
  cause text stays internal and the public boundary returns `pricing request is invalid`.

## Still open

- Audit order, payment, fulfillment, inventory, customer, tax, promotion, and remaining
  ecommerce adapters for technical text mislabeled as validation/conflict errors.
- Add structured owner-side logging with `correlation_id`, owner operation, stable error
  code, and the original cause before every remaining technical `PortError` mapping.
- Remove raw technical text from non-`PortError` public REST, GraphQL, native, and
  operator error envelopes.
- Add compile and transport round-trip evidence before changing any FBA/FFA status.

## Verification

- `node scripts/verify/verify-port-error-public-safety.mjs`
- `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `cargo test -p rustok-api ports::tests`
- `cargo check -p rustok-pricing --all-features`
- Targeted pricing database, validation, rich/core invariant, correlation, and transport
  round-trip tests.

No verification command above was executed as part of this source wave.
