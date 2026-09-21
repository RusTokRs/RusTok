# rustok-graphql

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client contracts for
RusToK UI adapters.

It provides request/response types, one shared pooled HTTP client, the shared
HTTP execution function, error mapping and persisted-query extension helpers.
It does not depend on Leptos, Dioxus, Next.js, GraphQL schema crates or host UI
context.

The client does not automatically retry GraphQL operations. Retry safety belongs
to the operation owner because an HTTP `POST` does not distinguish an idempotent
query from a mutation. Browser and native targets use the same no-hidden-retry
contract. Native requests use the shared client timeout; browser request timing
is owned by the browser fetch runtime and higher-level cancellation policy.
