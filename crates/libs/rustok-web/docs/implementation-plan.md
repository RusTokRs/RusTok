# Implementation plan for `rustok-web`

## Current state

`rustok-web` owns the small shared Axum HTTP boundary: `HttpError`,
`HttpResult`, `ErrorBody`, and `json_response`. It keeps controller response
envelopes and status mapping consistent across Axum controller adapters.

The crate is not a web framework and does not own domain errors, runtime
composition, FBA metadata, CLI contracts, or UI transport. Domain errors remain
with their owner; stable neutral API contracts remain in `rustok-api`.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This Axum helper crate has no module-owned UI or FBA provider port.

## Open results

1. **Consolidate repeated controller helpers through the shared boundary.**
   Replace duplicate JSON/response helpers in server and module HTTP adapters
   during the next controller-owner slices.
   **Depends on:** controller-owner migration work.
   **Done when:** migrated controllers use `json_response`/`HttpError` without
   changing owner domain semantics or adding local response envelopes.

2. **Preserve neutral PortError-to-HTTP evidence.** The mapper already owns kind-to-status
   mapping and infrastructure redaction. The 2026-10-30 source continuation extends the same
   `HttpError` / `ErrorBody` with optional exact owner retry advice, not another envelope.
   Unknown advice remains absent; present false is not omitted; malformed present wire values
   are refused. See [CRATE_API.md](../CRATE_API.md) for the owning contract.
   **Verification status:** implementation and source-only checks are recorded below; no test
   execution or Rust compilation is implied.

3. **Lock web-boundary behavior as migration broadens.** Add source guardrails
   and focused status/body tests for controller migrations that consume this
   crate.
   **Depends on:** migrated controller examples.
   **Done when:** API surface verification confirms shared response formatting
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

## Canonical HTTP retry advice (2026-10-30; source only)

- `HttpError` and `ErrorBody` carry `Option<bool>` advice. Ordinary constructors retain unknown;
  the owner-aware builder and shared port mapper preserve both owner bits without classification.
- The same DTO derives its OpenAPI schema; a present member is a non-nullable boolean. Missing
  remains valid/unknown. Null, strings, numbers, arrays and objects are not an absent decision.
- The host still delegates `Error::Http` directly. Commerce/Media typed-owner adapters forward
  their already-known policy, including nested and staged-runtime errors. Stable status, code,
  public copy and infrastructure redaction remain unchanged.
- The live Next Admin registry REST helper consumes one typed shared HTTP projection. The reader
  admits at most 16 KiB of UTF-8 payload and waits up to two seconds for metadata, cancels oversized
  or unfinished reads, and releases its reader/timer. Failed or stalled cancellation is not
  awaited as a new response prerequisite. These bounds are source policy, not measured runtime
  timing or proof of upstream cancellation.
- The client distinguishes owner false/true, absent and invalid evidence. It does not parse
  HTML/raw/nested diagnostic bodies as public text, copy outcome/key claims, classify retryability
  from status, send another request, or change immutable commands/journals/recovery.

Five canonical HTTP, two host delegation and one host-composed OpenAPI regression functions
plus eleven client regression functions are added, all **unrun**. The existing API-surface source guard covers the completed
forwarding slice, canonical DTO/schema and actual client entrypoint. No Rust compilation,
OpenAPI generation, tests, database, mounted/native/browser/remote execution or readiness
promotion is claimed. The [Commerce continuation](../../../modules/rustok-commerce/docs/implementation-plan.md#canonical-http-retry-advice-2026-10-30)
records the final scoped static evidence and remaining Product step-8 work.
