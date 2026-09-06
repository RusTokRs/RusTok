# rustok-graphql

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client contracts for
RusToK UI adapters.

It provides request/response types, the shared HTTP execution function, error
mapping and persisted-query extension helpers. It does not depend on Leptos,
Dioxus, Next.js, GraphQL schema crates or host UI context.
