# rustok-ui-transport / CRATE_API

## Public surface

### Primary Enums & Types
- `UiTransportPath`: transport channel selector (`NativeServer`, `Graphql`).
- `UiTransportRetrySafety`: retry idempotency classifier (`SafeToRetry`, `AtMostOnce`).
- `UiTransportError`: structured transport error capturing failing path, fallback status, and underlying error messages.
- `UiTransportResult<T>`: type alias for `Result<T, UiTransportError>`.

### Execution & Fallback Functions
- `execute_selected_transport<T, ...>(surface, path, native, graphql) -> UiTransportResult<T>`: executes the requested transport without fallback.
- `execute_with_fallback<T, ...>(surface, primary_path, native, graphql) -> UiTransportResult<T>`: executes primary path with automatic secondary fallback.
- `execute_transport_policy<T, ...>(surface, path, retry_safety, fallback_allowed, native, graphql) -> UiTransportResult<T>`: executes transport adhering to safety and fallback policy constraints.

## Invariants

- **Idempotency Guard**: Mutations or non-idempotent operations classified as `AtMostOnce` must never be transparently retried on a secondary transport if the primary attempt fails.
- **Structured Error Evidence**: Transport failures record the failed path, whether fallback was attempted, and individual errors from both native server and GraphQL adapters without masking or swallowing details.
- **Framework Independence**: Pure asynchronous contracts independent of Leptos, Dioxus, or Next.js rendering runtimes.
