# RusToK Server

## Purpose

`apps/server` owns the main backend composition root for RusToK.

## Responsibilities

- Compose platform modules into the live runtime.
- Expose GraphQL, HTTP, and Leptos server-function surfaces.
- Host runtime orchestration, migrations, background tasks, and platform-owned control planes.

## Entry points

- `src/main.rs`
- `/api/graphql`
- `/api/fn/*`
- server task/runtime entrypoints under `src/tasks` and `src/services`

## Interactions

- Uses `crates/rustok-core` and platform/domain modules as the backend composition root.
- Serves `apps/admin`, `apps/storefront`, `apps/next-admin`, and `apps/next-frontend`.
- Hosts platform-owned runtime layers such as MCP management, module composition, and orchestration bridges.

## Auth config

`apps/server/src/auth.rs` is the only bridge from Loco config into `rustok-auth`.
`auth.jwt.secret` remains the HS256 default secret source. Optional runtime
overrides live under `settings.rustok.auth`:

- `refresh_expiration`, `issuer`, `audience`
- `algorithm`: `HS256` or `RS256`
- `rsa_private_key_env`, `rsa_public_key_env`: preferred production key sources
- `rsa_private_key_pem`, `rsa_public_key_pem`: development/test fallback values

When `algorithm` is `RS256`, both RSA keys are required and boot-time config
assembly must fail instead of silently falling back to `HS256`.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
