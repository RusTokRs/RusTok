# Pricing owner port error safety

Status: **source-ready / unvalidated**

## Scope

This contract tracks the currently identified Pricing public-message and payload-diagnostic
gaps across the complete canonical boundary:

- owner `crates/modules/rustok-pricing/src/ports.rs`;
- canonical read `crates/modules/rustok-pricing/src/read_context.rs`;
- canonical write `crates/modules/rustok-pricing/src/write_context.rs`.

The six `PricingReadPort` operations and four `PricingWritePort` operations remain
unchanged. Request and response DTOs, policy and write-semantics checks, service
delegation, price resolution, discount application, price-list scope/rule behavior,
event publication, and persistence behavior are unchanged.

## Owner outcomes

For the thirteen non-wrapper Foundation outcomes, codes, `PortErrorKind` values, and
retryability are preserved. Nested Core/Rich classification is defined by the continuation
below. Dynamic payload-bearing
owner messages are replaced with static messages:

- product and variant identities are not included in not-found messages;
- product/variant mismatch messages do not include either UUID;
- handles, locales, SKUs, and shipping-profile slugs are not included;
- requested and available stock values are not included;
- tenant and actor parser messages no longer expose internal `PortContext` wording.

All fifteen current `CommerceError` variants receive a closed static variant label.
Owner diagnostics retain only aggregate text, UUID, numeric, and opaque-payload shape.

## Canonical wrappers

Both canonical wrappers preserve their original owner delegation and local outcome
classification. Their diagnostics retain correlation ID, exact public/local operation,
stable code, retryability, message lengths, a closed error-kind label, and bounded
context/request shape.

They do not record:

- raw tenant, actor, channel, locale, causation, traceparent, or idempotency values;
- product, variant, region, channel, or price-list UUID values;
- exact read quantity or write minimum/maximum quantity values;
- public or original message text;
- debug-formatted `PortErrorKind` values;
- price, percentage, compare-at, currency, slug, handle, SKU, row, or projection payloads.

## Severity

Unavailable, timeout, and invariant outcomes use error severity, including nested Core/Rich
outcomes of those kinds. Nested domain/permission refusals are not blanket invariants.
Not-found, conflict, validation, and forbidden outcomes use warning severity. Direct mismatch and
direct not-found outcomes also remain warning severity.

## Boundary status

The owner `ports.rs`, canonical read wrapper, and canonical write wrapper are
**source-closed / unvalidated** for the currently identified Pricing public-message and
payload-diagnostic gaps.

The broader ecommerce cleanup remains open. Compile validation, focused and aggregate
verifier execution, verifier-test execution, CI, and mounted runtime evidence also remain
open.

## Evidence

- `crates/modules/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source.json`
- `crates/modules/rustok-pricing/contracts/evidence/pricing-owner-port-error-safety-source-review.json`
- `crates/modules/rustok-pricing/contracts/evidence/pricing-read-local-diagnostic-safety-source.json`
- `crates/modules/rustok-pricing/contracts/evidence/pricing-write-local-diagnostic-safety-source.json`
- `scripts/verify/verify-pricing-owner-port-error-safety.mjs`
- `scripts/verify/verify-pricing-read-local-context.mjs`
- `scripts/verify/verify-pricing-write-local-context.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`

The original source snapshot claimed no test, verifier, formatter, Cargo, workflow, CI, or
mounted runtime execution. See the dated continuation below for current source-only checks;
historical evidence is not current execution evidence.


## Nested error and public consumer continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Base main: `6716323bc5630dd87939988a4507eaf0fea53827`; branch:
`arena/8161797f-rustok`. This bounded remediation does not advance the full-stack audit cursor.

`ports.rs::pricing_error_policy` is the single Foundation-error classifier used by both
canonical ports and the direct-service public adapter. It exhaustively handles all fifteen
Foundation variants. The thirteen non-wrapper outcomes keep their owner code, static copy,
kind, and retryability. No second Foundation/Core/Rich classifier lives in the UI.

Nested Core has eleven exhaustive variants:

- database -> retryable Unavailable (`pricing.database_unavailable`);
- cache/external -> retryable Unavailable (`pricing.dependency_unavailable`);
- auth/forbidden -> Forbidden (`pricing.access_denied`);
- validation/invalid ID -> Validation (`pricing.validation`);
- not found -> NotFound (`pricing.resource_not_found`);
- serialization/scripting/internal -> InvariantViolation (`pricing.core_error`).

RichError classification uses only its eleven closed `ErrorKind` values:
validation/business logic -> Validation; unauthenticated/forbidden -> Forbidden;
not found -> NotFound; conflict -> Conflict; database/external service -> retryable
Unavailable; timeout -> retryable Timeout; internal -> InvariantViolation. RateLimited
maps to retryable Unavailable (`pricing.rate_limited`), because `PortErrorKind` has no
rate-limit variant. No HTTP 429 or Retry-After contract is invented. Arbitrary rich status,
user message, error code, trace ID, fields, and source are not public authority.
`pricing.rich_error` remains the Rich internal-invariant fallback, not a blanket wrapper code.

The Pricing-owned `PricingPublicError` descriptor contains static message/code, retryability,
and a fresh UUID reference. Product native pricing renders this descriptor; Commerce GraphQL
pricing/cart-promotion mutations use its port presentation and return the same reference
in extensions. Neither boundary exposes or logs raw error/message/code/context values.
Port presentation retains owner retryability and forces Unavailable/Timeout to retryable;
it never downgrades an owner-declared retryable refusal. The query facade retains its
query-specific static copy and optional-not-found behavior; remaining failures now preserve
owner retryability instead of silently forcing it false for domain kinds.

The direct service diagnostic uses the existing exhaustive owner shape extractor. All ten
owner operation entry events now use bounded context facts rather than tenant/resource IDs,
handles, currency, or other request values. Existing canonical owner/wrapper correlation
conventions remain unchanged; this slice does not certify arbitrary caller-supplied correlation
admission, the entire Pricing service/Translation/price-list pipeline, or other public consumers.
Request DTOs, successful snapshots, service calls, policy checks, transaction/receipt behavior,
and persistence/event publication are unchanged. No dependency or schema change is needed.

Verification is source-only: six affected Rust files formatted/idempotent with Scalar rust-fmt
0.3.0 (Rust 2024 parser), and twelve scoped standalone source guard entrypoints passed, including owner/read/write,
Commerce pricing mutation/query/contextual owner, and Product consumer regressions. JavaScript
syntax checks for seven affected guards and `git diff --check` also passed. Five Rust scenarios were
added but **not run**, covering legacy outcomes, Core11, Rich11 with poisoned metadata,
arbitrary PortError presentation/retryability, and canonical-port propagation. No tests,
Cargo check, CI, or mounted runtime execution were performed for this continuation.
Historical JSON evidence above is not promoted to current execution evidence.

Current guard: `node scripts/verify/verify-pricing-public-error.mjs`
(`npm run verify:pricing:public-error`), together with the existing owner/read/write and
Commerce pricing mutation/query diagnostic source guards. Compilation and runtime evidence,
maintainer review, and integration remain open.
