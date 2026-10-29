# rustok-graphql / CRATE_API

## Public surface

### Constants
- `GRAPHQL_ENDPOINT`: default relative endpoint path (`/api/graphql`).
- `TENANT_HEADER`: HTTP header for tenant context (`X-Tenant-Slug`).
- `AUTH_HEADER`: HTTP authorization header (`Authorization`).
- `ACCEPT_LANGUAGE_HEADER`: HTTP accept language header (`Accept-Language`).
- `DEFAULT_TIMEOUT`: default request duration (30 seconds).

### Client Functions & Configuration
- `default_client() -> Result<&'static ClientWithMiddleware, GraphqlHttpError>`: returns the shared singleton HTTP client with connection pooling.
- `graphql_endpoint_from_base(base: &str) -> String`: resolves the GraphQL endpoint given a base URL.
- `default_graphql_url() -> String` (aliased as `graphql_url()`): resolves the default GraphQL target URL from environment or origin.
- `persisted_query_extension(sha256_hash: &str) -> serde_json::Value`: generates standard GraphQL persisted query extension payloads.

### Data Types
- `GraphqlRequest<V>`: typed GraphQL request container supporting optional query variables and protocol extensions.
- `GraphqlResponse<T>`: response container encapsulating optional typed data and structured error lists.
- `GraphqlError`: individual GraphQL error message container.
- `GraphqlHttpError`: typed transport and application error enum (`Network`, `Graphql`, `Http`, `Unauthorized`).

### Execution Functions
- `execute<V, T>(endpoint: &str, request: GraphqlRequest<V>, token: Option<String>, tenant_slug: Option<String>, locale: Option<String>) -> Result<T, GraphqlHttpError>`: sends a GraphQL request using the default client.
- `execute_with_client<V, T>(client: &ClientWithMiddleware, endpoint: &str, request: GraphqlRequest<V>, token: Option<String>, tenant_slug: Option<String>, locale: Option<String>) -> Result<T, GraphqlHttpError>`: sends a GraphQL request using an explicitly provided client.

## Invariants

- **Framework Independence**: Must not import or depend on Leptos, Dioxus, Next.js, or UI runtime contexts.
- **No Automatic Retry**: Operations (especially mutations) are never automatically retried by the HTTP layer; retry semantics belong to higher-level caller policies.
- **Consistent Headers**: Multi-tenant context (`X-Tenant-Slug`), authentication tokens (`Bearer ...`), and localization (`Accept-Language`) are explicitly propagated as HTTP headers.
- **Error Surface**: HTTP failures and GraphQL error arrays are mapped to typed `GraphqlHttpError` variants rather than panicking or swallowing details.
