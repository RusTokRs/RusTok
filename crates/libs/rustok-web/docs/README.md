# `rustok-web` Documentation

`rustok-web` is the shared Axum boundary crate for RusToK.

It is not a web framework. It exists so that controller adapters do not
turn into many local copies of response envelopes, status mapping, and extractor glue.

Boundary rules:

- Domain errors stay in owner modules.
- Runtime access helpers belong in `rustok-runtime`.
- Public API contracts that are not Axum-specific stay in `rustok-api`.
- The crate must stay independent from Leptos and other UI frameworks.

Current entry points:

- `json_response(value)` for JSON response mapping in Axum handlers.
- `HttpError`, `HttpResult` and `ErrorBody` for HTTP boundary errors.
- `port_error_to_http_error(error)` for preserving typed module-port status and exact retry advice
  while redacting infrastructure failure details.
- `HttpError::with_retryable(bool)` for adapters that already own a final retry decision.
  `ErrorBody.retryable` is optional/non-nullable in JSON and OpenAPI; a present false is retained,
  absent is unknown, and malformed present values are refused. It is never write-outcome evidence.
- `BrowserAssetRegistry`, `ReleaseQualifiedAsset`, and `ReleaseAssetSet` for serving
  immutable, content-addressed browser assets with strict 404s and dual N/N+1 retention.

Use `json_response` in server or module HTTP adapters. Keep response formatting
inside this shared boundary.

Related guide: [Backend Module Implementation](../../../../docs/backend/module-backend-implementation.md).

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Crate README](../README.md)
- [CRATE_API](../CRATE_API.md)

The [HTTP retry-advice contract](../../../../docs/architecture/api.md#http-error-retry-advice)
records transport scope and the actual client projection. No automatic mutation replay or new
idempotency identity is granted by this field.
