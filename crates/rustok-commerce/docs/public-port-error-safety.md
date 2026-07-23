# Public ecommerce port error safety

Status: `source_ready_unvalidated`

This source wave closes the public transport leak for technical owner-port errors
without claiming that every owner mapper has correlation-aware internal logging.

## Enforced invariant

`rustok-api::PortError` now treats these kinds as technical and fail-closed:

- `Unavailable` always exposes `the requested capability is temporarily unavailable`.
- `InvariantViolation` always exposes `the requested operation could not be completed safely`.

The rule is applied in all three boundary locations:

1. `PortError::new` and the typed constructors.
2. custom `Serialize` implementation before a port error crosses a transport.
3. custom `Deserialize` implementation when a remote port error enters a consumer.

A caller cannot bypass transport sanitization by mutating `PortError.message` or by
returning a remote payload containing raw SQL, SDK, provider, stack, or invariant text.
Validation, not-found, conflict, forbidden, and explicit timeout messages remain
available for actionable domain errors.

## Owner mapper hardening already present

- `rustok-channel`: database and serialization causes are logged internally and mapped
  to stable public messages.
- `rustok-region`: database causes are logged internally and mapped to a stable public
  message.
- `rustok-cart` checkout snapshot: validator and serialization causes are logged
  internally; request/projection and encoding failures use stable public messages.

## Still open

- Replace raw technical formatting in pricing and the remaining ecommerce owner mappers
  even though the central transport invariant now prevents public disclosure.
- Add structured owner-side logging with `correlation_id`, owner operation, stable error
  code, and the original cause before every technical `PortError` mapping.
- Audit validation/conflict mappings so internal parser or serializer text is not
  mislabeled as a domain error.
- Add compile and transport round-trip evidence before changing any FBA/FFA status.

## Verification

- `node scripts/verify/verify-port-error-public-safety.mjs`
- `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `cargo test -p rustok-api ports::tests`

No command above was executed as part of this source wave.
