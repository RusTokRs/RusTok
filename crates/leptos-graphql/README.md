# leptos-graphql

## Purpose

`leptos-graphql` owns the shared GraphQL transport layer for Leptos applications in RusToK.

## Responsibilities

- Execute GraphQL requests over HTTP.
- Provide reactive query and mutation hooks for Leptos UI packages.
- Apply shared auth, tenant, and locale headers without duplicating transport glue across hosts.

## Entry points

- `execute`
- `use_query`
- `use_mutation`
- `use_lazy_query`
- `GraphqlRequest`
- `GraphqlResponse`
- `GraphqlHttpError`

## Interactions

- Used by Leptos UI packages and apps that talk to RusToK GraphQL surfaces.
- Used by `leptos-auth` as the fallback transport path for auth flows.
- Talks to `apps/server` GraphQL endpoints while staying free from module-specific schema ownership.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
