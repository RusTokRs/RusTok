# rustok-graphql / CRATE_API

## Purpose
`rustok-graphql` provides framework-agnostic GraphQL HTTP client contracts, transport helpers, header propagation, and error mapping for RusToK UI adapters (Leptos, Dioxus, Next.js proxy bridges).

## Architecture & Responsibilities
- **Singleton Pooled Client**: Lazily initializes and shares a connection-pooled HTTP client (`default_client()`) with standard timeouts and connection reuse.
- **Endpoint Resolution & Normalization**: Resolves GraphQL URLs relative to base host URLs (`graphql_endpoint_from_base`) and environment configuration (`default_graphql_url`).
- **Context Header Propagation**: Explicitly propagates multi-tenant headers (`X-Tenant-Slug`), bearer authorization tokens (`Authorization: Bearer <token>`), and localization preference (`Accept-Language`).
- **Typed Error Mapping**: Translates HTTP errors, network failures, 401 unauthenticated responses, and GraphQL error arrays into structured `GraphqlHttpError` types.
- **Persisted Query Support**: Emits Automated Persisted Queries (APQ) hash payloads compatible with Apollo and RusToK GraphQL servers.

## Public Surface
### Constants
- `GRAPHQL_ENDPOINT`: Default relative endpoint path (`/api/graphql`).
- `TENANT_HEADER`: HTTP header for tenant context (`X-Tenant-Slug`).
- `AUTH_HEADER`: HTTP authorization header (`Authorization`).
- `ACCEPT_LANGUAGE_HEADER`: HTTP accept language header (`Accept-Language`).
- `DEFAULT_TIMEOUT`: Default request timeout duration (30 seconds).

### Client Functions & Endpoint Resolvers
- `default_client() -> Result<&'static ClientWithMiddleware, GraphqlHttpError>`: Returns the shared singleton HTTP client with connection pooling.
- `graphql_endpoint_from_base(base: &str) -> String`: Resolves and normalizes the GraphQL endpoint given a base URL.
- `default_graphql_url() -> String` (aliased as `graphql_url()`): Resolves default target URL from environment or browser origin.
- `persisted_query_extension(sha256_hash: &str) -> serde_json::Value`: Generates standard GraphQL persisted query extension payloads.

### Data Types
- `GraphqlRequest<V>`: Typed GraphQL request container supporting optional query variables and protocol extensions.
- `GraphqlResponse<T>`: Response container encapsulating optional typed data and structured error lists.
- `GraphqlError`: Individual GraphQL error message container.
- `GraphqlHttpError`: Typed transport and application error enum (`Network`, `Graphql(String)`, `Http(String)`, `Unauthorized`).

### Execution Functions
- `execute<V, T>(endpoint: &str, request: GraphqlRequest<V>, token: Option<String>, tenant_slug: Option<String>, locale: Option<String>) -> Result<T, GraphqlHttpError>`: Sends a GraphQL request using the default client.
- `execute_with_client<V, T>(client: &ClientWithMiddleware, endpoint: &str, request: GraphqlRequest<V>, token: Option<String>, tenant_slug: Option<String>, locale: Option<String>) -> Result<T, GraphqlHttpError>`: Sends a GraphQL request using an explicitly provided client.
- `execute_with_raw_client<V, T>(client: &reqwest::Client, endpoint: &str, request: GraphqlRequest<V>, token: Option<String>, tenant_slug: Option<String>, locale: Option<String>) -> Result<T, GraphqlHttpError>`: Convenience helper wrapping a raw reqwest client.

## Invariants & Guarantees
- **Framework Independence**: Contains zero imports of Leptos, Dioxus, or Next.js types; usable across web, SSR, CLI, and integration testing harnesses.
- **No Automatic Retry**: Operations (especially mutations) are never automatically retried by the HTTP transport layer; idempotent retry semantics remain the caller's responsibility.
- **Header Isolation & Propagation**: Tenant, auth, and locale headers are attached per request only when explicitly supplied, preventing unintended cross-tenant leaks.
- **Structured Error Mapping**: HTTP failures and GraphQL error arrays are mapped to typed `GraphqlHttpError` variants rather than panicking or swallowing error details.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `reqwest`: HTTP transport client.
  - `reqwest-middleware`: Middleware pipeline wrapper.
  - `url`: Safe URL resolution and endpoint joining.
  - `serde`, `serde_json`: Request/response serialization.
  - `thiserror`: Typed error derive macro.
- **Boundary Rules**: `rustok-graphql` is a low-level platform UI utility; it has zero dependencies on domain crates (`rustok-commerce`, `rustok-content`, etc.) and does not hold domain schema models.
