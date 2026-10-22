# rustok-web / CRATE_API

## Role
Axum HTTP boundary helpers providing neutral error response mapping, status code resolution, Content Security Policy nonces, and content-addressed browser asset serving.

## Public modules
- Root crate definitions: `HttpError`, `HttpResult`, `ErrorBody`, `CspNonce`, `BrowserAssetRegistry`, `ReleaseQualifiedAsset`, `ReleaseAssetSet`.

## Primary public types and functions
- `pub struct HttpError`: Axum-compatible HTTP error with `StatusCode`, error code, and public message.
- `pub type HttpResult<T> = Result<T, HttpError>`
- `pub struct ErrorBody`: Structured JSON error response body (`code`, `message`).
- `pub struct CspNonce`: Cryptographically secure per-response CSP nonce.
- `pub fn port_error_to_http_error(err: PortError) -> HttpError`: Maps typed module port errors to HTTP status codes.
- `pub fn json_response<T: Serialize>(status: StatusCode, data: T) -> Response`
- `pub struct BrowserAssetRegistry`: Serves release-qualified static browser assets with strong ETag validation.

## Contract invariants
- **Error sanitization**: Internal database or infrastructure details are never exposed to external HTTP callers; port errors map to sanitized status codes.
- **Cache immutability**: Release-qualified assets use content-addressed hashes with long-term cache headers and strong `ETag` / `If-None-Match` validation.
- **Fail-closed routing**: Unknown asset paths or corrupted ETag requests return strict 404/400 responses without path traversal.

## Errors
- [`HttpError`]: Returned on HTTP boundary violations with sanitized `StatusCode`.
