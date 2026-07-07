# rustok-graphql-leptos

`rustok-graphql-leptos` provides Leptos hooks on top of the framework-agnostic
`rustok-graphql` HTTP client contracts.

Module transport adapters that only need `execute`, `GraphqlRequest` and
`GraphqlHttpError` should depend on `rustok-graphql` directly.
