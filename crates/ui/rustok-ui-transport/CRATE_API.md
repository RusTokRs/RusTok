# rustok-ui-transport / CRATE_API

## Purpose

`rustok-ui-transport` defines framework-neutral transport execution abstractions, error evidence structures, and fallback policies across native server and GraphQL adapters for RusToK FFA UI packages.

## Architecture & Responsibilities

- **Dual-Transport Abstraction**: Unifies client calls across Leptos native server functions and GraphQL HTTP/WebSocket transports.
- **Fail-Soft Transparent Fallback**: Implements `execute_with_fallback` allowing read-only or idempotent operations to seamlessly recover on an alternate transport path.
- **Idempotency Guard**: Guarantees that non-idempotent operations classified as `UiTransportRetrySafety::AtMostOnce` are NEVER retried through a secondary transport when the primary attempt fails.
- **Structured Error Evidence**: Captures full failure diagnostics without masking: surfaces failed transport path, whether secondary fallback was attempted, and individual error strings from each path.
- **Zero Framework Bindings**: Pure asynchronous Rust library without dependencies on Leptos signals, Dioxus hooks, Next.js, or browser DOM types.

## Public Surface

### Primary Enums & Types
- `UiTransportPath`: Transport selector (`NativeServer`, `Graphql`).
- `UiTransportRetrySafety`: Retry idempotency classifier (`SafeToRetry`, `AtMostOnce`).
- `UiTransportError`: Comprehensive error container:
  - `pub surface: String`
  - `pub failed_path: UiTransportPath`
  - `pub fallback_attempted: bool`
  - `pub native_error: Option<String>`
  - `pub graphql_error: Option<String>`
- `UiTransportResult<T>`: Type alias for `Result<T, UiTransportError>`.

### Execution Functions
- `execute_selected_transport<T, ...>(surface, path, native, graphql) -> UiTransportResult<T>`: Executes the specified transport without fallback.
- `execute_with_fallback<T, ...>(surface, primary_path, native, graphql) -> UiTransportResult<T>`: Executes primary transport with automatic recovery on secondary path if primary fails.
- `execute_transport_policy<T, ...>(surface, path, retry_safety, fallback_allowed, native, graphql) -> UiTransportResult<T>`: Executes transport with strict enforcement of retry safety and policy permissions.

## Invariants & Guarantees

1. **At-Most-Once Mutation Safety**: Mutations and unsafe operations (`AtMostOnce`) MUST NEVER be retried via fallback, preventing duplicate writes or inconsistent remote states.
2. **Deterministic Error Logging**: `UiTransportError` preserves distinct messages for both native and GraphQL paths; neither error may be silently dropped.
3. **Lazy Execution**: Transport futures are lazily invoked only when selected; fallback futures are never constructed or polled unless primary execution fails.
4. **Stable Serialization**: `UiTransportPath` and `UiTransportRetrySafety` serialize to stable `snake_case` JSON identifiers (`"native_server"`, `"graphql"`, `"safe_to_retry"`, `"at_most_once"`).

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `serde`, `serde_json`: Transport error and metadata serialization.
- **Downstream Consumers**:
  - Storefront and admin module frontend adapters (`crates/modules/*/admin`, `crates/modules/*/storefront`).
  - Next.js and Leptos host transport providers.
- **Boundary Rules**:
  - Server endpoints, GraphQL schemas, and native server functions remain owned by their respective modules.
  - No HTTP client (`reqwest`), WebSocket, or DOM dependencies.
