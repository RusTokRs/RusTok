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
- `port_error_to_http_error(error)` for preserving typed module-port status semantics while
  redacting infrastructure failure details.

Use `json_response` in server or module HTTP adapters. Keep response formatting
inside this shared boundary.

Related guide: [Backend Module Implementation](../../../docs/backend/module-backend-implementation.md).
