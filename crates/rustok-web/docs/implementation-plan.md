# Implementation plan for `rustok-web`

## Current state

`rustok-web` owns the small shared Axum HTTP boundary: `HttpError`,
`HttpResult`, `ErrorBody`, and `json_response`. It keeps controller response
envelopes and status mapping consistent as Loco controller APIs are removed.

The crate is not a web framework and does not own domain errors, runtime
composition, FBA metadata, CLI contracts, or UI transport. Domain errors remain
with their owner; stable neutral API contracts remain in `rustok-api`.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This Axum helper crate has no module-owned UI or FBA provider port.

## Open results

1. **Migrate repeated Loco controller helpers through the shared boundary.**
   Replace duplicate JSON/response helpers in server and module HTTP adapters
   during the next Loco-exit controller slices.
   **Depends on:** the Loco exit plan and controller-owner migration work.
   **Done when:** migrated controllers use `json_response`/`HttpError` without
   changing owner domain semantics or adding local response envelopes.

2. **Add neutral PortError-to-HTTP mapping when a cutover needs it.** Design the
   mapper from `rustok-api::PortError` only from real controller behavior,
   preserving owner error codes and retryability.
   **Depends on:** the first controller using a public port error path.
   **Done when:** typed port errors map to stable HTTP status/body responses with
   focused tests and no domain error classification in this crate.

3. **Lock web-boundary behavior as migration broadens.** Add source guardrails
   and focused status/body tests for controller migrations that consume this
   crate.
   **Depends on:** migrated controller examples.
   **Done when:** API surface verification prevents new Loco formatter imports
   and regressions in shared error response semantics.

## Verification

- `npm run verify:api:surface-contract`
- Focused `HttpError`/`ErrorBody` status and JSON response tests.
- Targeted controller tests whenever an HTTP adapter migrates.

## Change rules

1. Keep this crate Axum-specific and domain-neutral.
2. Update local docs and server/module controller documentation with a changed
   web-boundary contract.
3. Do not introduce runtime composition or UI dependencies here.
