# `rustok-web` Documentation

`rustok-web` is the shared Axum boundary crate for RusToK.

It is not a web framework. It exists so that the Loco controller replacement does not
turn into many local copies of response envelopes, status mapping, and extractor glue.

Boundary rules:

- Domain errors stay in owner modules.
- Runtime access helpers belong in `rustok-runtime`.
- Public API contracts that are not Axum-specific stay in `rustok-api`.
- The crate must stay independent from Leptos and other UI frameworks.

Current entry points:

- `json_response(value)` for JSON response mapping in Axum handlers.
- `HttpError`, `HttpResult` and `ErrorBody` for HTTP boundary errors.

Use `json_response` when replacing `loco_rs::controller::format::json` in server or
module HTTP adapters. Do not add new Loco formatter imports.

Related guide: [Backend Module Implementation](../../../docs/backend/module-backend-implementation.md).
