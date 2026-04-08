# rustok-auth

## Purpose

`rustok-auth` owns authentication primitives for RusToK: password hashing, JWT lifecycle,
refresh-token helpers, auth config, and auth-related migrations.

## Responsibilities

- Provide `AuthModule` metadata for the runtime registry.
- Expose auth primitives used by `apps/server` transport adapters and lifecycle services.
- Publish the typed `users:*` RBAC surface through `RusToKModule::permissions()`.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Used by `apps/server` for REST and GraphQL auth flows, session handling, and user lifecycle.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService`; `rustok-auth` itself does not depend on `rustok-rbac`.
- Human-readable RBAC ownership for the auth module is `users:*`.

## Entry points

- `AuthModule`
- `AuthConfig`
- `Claims`
- `encode_access_token`
- `decode_access_token`
- `generate_refresh_token`
- `hash_password`
- `verify_password`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
