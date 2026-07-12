# rustok-web

## Purpose

`rustok-web` owns small Axum HTTP boundary helpers used by server and module HTTP
adapters.

## Responsibilities

- Provide shared HTTP error envelopes and response mapping.
- Keep Axum controller adapters consistent.
- Host reusable web-boundary helpers that are not domain logic.

## Entry Points

- `HttpError`
- `HttpResult`
- `ErrorBody`
- `json_response`

## Interactions

- Used by `apps/server` and module HTTP controllers for shared response mapping.
- May map neutral backend errors into HTTP responses.
- Does not own runtime composition, domain errors, FBA metadata, CLI contracts, or UI transport.

See [docs](docs/README.md).
