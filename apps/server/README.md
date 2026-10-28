# RusToK Server

## Purpose

`apps/server` owns the main backend composition root for RusToK.

## Responsibilities

- Compose platform modules into the live runtime.
- Expose GraphQL, HTTP, and Leptos server-function surfaces.
- Host runtime orchestration, background tasks, and platform-owned control planes.

## Entry points

- `src/main.rs`
- `/api/graphql`
- `/api/fn/*`
- server task/runtime entrypoints under `src/tasks` and `src/services`

## Interactions

- Uses `crates/libs/rustok-core` and platform/domain modules as the backend composition root.
- Serves `apps/admin`, `apps/storefront`, `apps/next-admin`, and `apps/next-frontend`.
- Hosts platform-owned runtime layers such as MCP management, module composition, and orchestration bridges.

## Production container configuration

The production Docker image is fail-closed by default: it sets `RUSTOK_ENV=production`, `RUSTOK_CONFIG_DIR=/app/config`, and `RUSTOK_HTTPS=true`. Mount the operator-managed `production.yaml` at `/app/config/production.yaml` (or provide an equivalent config through the configured directory). Development and test configuration files are deliberately not copied into the production image.

## Auth config

`apps/server/src/auth.rs` adapts the host configuration snapshot into
`rustok-auth`.
`auth.jwt.secret` remains the HS256 default secret source. Optional runtime
overrides live under `settings.rustok.auth`:

- `refresh_expiration`, `issuer`, `audience`
- `algorithm`: `HS256` or `RS256`
- `rsa_private_key_env`, `rsa_public_key_env`: preferred production key sources
- `rsa_private_key_pem`, `rsa_public_key_pem`: development/test fallback values

When `algorithm` is `RS256`, both RSA keys are required and boot-time config
assembly must fail instead of silently falling back to `HS256`.

## Request trust and client IP

The host ignores `Forwarded` and `X-Forwarded-*` by default. To enable
forwarded client identity, configure `rustok.runtime.request_trust` with
`forwarded_headers_mode: trusted_only` and explicit
`trusted_proxy_cidrs`. Never use a public client subnet as a trusted proxy.

When the immediate peer is trusted, client identity is resolved from the
rightmost `X-Forwarded-For` / RFC `Forwarded` hop towards the caller, stopping
at the first untrusted address. This identity is also used for API rate-limit
keys. Missing or malformed trusted-side hops fall back to the actual peer;
an untrusted peer cannot supply an authoritative forwarding header. If both
header formats are present, `X-Forwarded-For` takes precedence.

**Ingress requirement:** the edge proxy must discard untrusted incoming
forwarded headers or append the actual observed client IP. A proxy that
blindly passes through client-supplied forwarding headers without appending
its observed peer cannot supply verifiable client identity. Keep the trusted
CIDRs narrow, including only proxies under operator control.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
