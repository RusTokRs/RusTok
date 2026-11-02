# rustok-web / CRATE_API

## Role
Axum HTTP boundary helpers providing neutral error response mapping, status code resolution, Content Security Policy nonces, and content-addressed browser asset serving.

## Public modules
- Root crate definitions: `HttpError`, `HttpResult`, `ErrorBody`, `CspNonce`, `BrowserAssetRegistry`, `ReleaseQualifiedAsset`, `ReleaseAssetSet`.

## Primary public types and functions
- `pub struct HttpError`: Axum-compatible HTTP error with `StatusCode`, error code, public message, and optional owner retry advice.
- `HttpError::with_retryable(bool) -> Self`: Attach exact owner advice; it is independent of status and durable outcome.
- `pub type HttpResult<T> = Result<T, HttpError>`
- `pub struct ErrorBody`: Structured JSON error response body (`code`, `message`, optional `retryable`).
- `pub struct CspNonce`: Cryptographically secure per-response CSP nonce.
- `pub fn port_error_to_http_error(err: PortError) -> HttpError`: Maps typed module port errors to HTTP status codes.
- `pub fn json_response<T: Serialize>(value: T) -> Response`
- `pub struct BrowserAssetRegistry`: Serves release-qualified static browser assets with strong ETag validation.

## Contract invariants
- **Error sanitization**: Internal database or infrastructure details are never exposed to external HTTP callers; port errors map to sanitized status codes.
- **Exact retry advice**: `HttpError::with_retryable(bool)` preserves the supplied owner decision;
  `port_error_to_http_error` forwards `PortError.retryable` unchanged for every error kind.
  Both `Some(true)` and `Some(false)` are serialized. Constructors without an owner decision
  keep `None`, omit the wire member, and never infer retryability from status or code.
- **Strict optional wire member**: an absent `retryable` means unknown. A present member must
  be a JSON boolean; null, strings, numbers, arrays, and objects are invalid. `ErrorBody`
  deserialization rejects those values, and its OpenAPI property is optional and non-nullable.
  Existing `code`, `message`, status mapping, and infrastructure redaction are unchanged.
- **Not outcome evidence**: retry advice does not establish whether an operation committed,
  authorize transport fallback or automatic mutation replay, or permit a new idempotency key.
  Operation owners retain immutable request, receipt, journal, and recovery authority.
- **Cache immutability**: Release-qualified assets use content-addressed hashes with long-term cache headers and strong `ETag` / `If-None-Match` validation.
- **Fail-closed routing**: Unknown asset paths or corrupted ETag requests return strict 404/400 responses without path traversal.

## Source verification (2026-10-30)

Five new unit-test functions cover absent advice, false/true JSON round trips, malformed
present values, both bits for all seven port kinds with unchanged status/redaction, and
optional/non-nullable OpenAPI shape. These regression sources are **unrun**. Source-only
handoff evidence is tracked in [the implementation plan](docs/implementation-plan.md).

## Errors
- [`HttpError`]: Returned on HTTP boundary violations with sanitized `StatusCode`.
