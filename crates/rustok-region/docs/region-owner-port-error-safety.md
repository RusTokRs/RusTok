# Region owner port error safety

Status: **source-ready / unvalidated**

## Scope

This contract closes the currently identified public-message and diagnostic-payload gaps in the owner `RegionReadPort` implementation in `crates/rustok-region/src/ports.rs`.

The two owner operations remain unchanged:

- `RegionReadPort::read_region`;
- `RegionReadPort::list_regions_for_tenant`.

Region lookup, country-code resolution, list projection, locale fallback, DTOs, owner service delegation, and read policy remain owned by the existing Region implementation.

## Public outcomes

Stable public codes, `PortErrorKind` values, and retryability are preserved.

- `region.not_found` remains a non-retryable not-found outcome.
- `region.read_failed` remains a retryable unavailable outcome with the existing static storage message.
- `region.validation` remains a non-retryable validation outcome, but owner-supplied validation and country-code payloads are no longer returned as public text. Both now use `region request is invalid`.
- `region.tenant_id_invalid` remains a non-retryable validation outcome with a bounded request-context message.
- Direct empty country-code validation retains its existing stable code and static message.

## Bounded diagnostics

Owner events retain the correlation id, exact owner operation, stable code, retryability where applicable, and the Region owner boundary label.

Request context is represented only through bounded context shape:

- tenant and actor-id character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, trace, and idempotency presence/length;
- locale length and deadline.

Read requests are represented only through bounded selector and locale shape:

- selector kind;
- whether a UUID selector is non-nil;
- country-code presence and length;
- requested and tenant-default locale presence and length.

Owner failures retain a closed error-variant label plus aggregate text, UUID, and opaque-payload facts. Database errors, validation text, country codes, region UUIDs, and complete `PortError` envelopes are not recorded.

## Severity

Database failures remain error severity. Not-found, validation, policy-admission, tenant-parse, and direct request-validation outcomes remain warning severity.

## Deliberate boundary

This is a source-only Region owner-boundary change. Storefront transport behavior, GraphQL/native selection, Region policy, persistence, module manifests, Commerce topology, and FBA/FFA status are unchanged.

The broader ecommerce correlation-safe mapper cleanup and runtime validation remain open.

## Evidence

- `crates/rustok-region/contracts/evidence/region-owner-port-error-safety-source.json`
- `crates/rustok-region/contracts/evidence/region-owner-port-error-safety-source-review.json`
- `scripts/verify/verify-region-owner-port-error-safety.mjs`

No tests, Node verifiers, Cargo commands, formatting, workflows, CI, or mounted runtime validation was executed.
