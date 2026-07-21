# Multilingual database transport drift

Last reviewed: 2026-07-21

This document records transport/read-model code that is incompatible with the
accepted language-agnostic database schema. It is not permission to restore
localized copy to base tables.

## OAuth application copy

The authoritative schema removes `oauth_apps.name` and
`oauth_apps.description`. Localized presentation copy lives in
`oauth_app_translations` and runtime `und` fallback is forbidden.

The following adapters still contain raw projections of the removed base
columns and must be cut over to the owner OAuth read/admin port or an exact
locale-scoped translation join:

- `crates/rustok-auth/admin/src/transport/native_server_adapter.rs`;
- `apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs`;
- `crates/rustok-channel/admin/src/transport/native_server_adapter.rs`.

Required correction:

1. Extract the host-resolved `RequestContext.locale`; do not reconstruct locale
   from UI defaults.
2. Normalize and reject storage-only `und` for runtime reads.
3. Read `oauth_app_translations` with exact
   `(tenant_id, app_id, locale)` identity, or call the owner OAuth admin/read
   port that already enforces this contract.
4. Do not silently choose English, tenant default, `und`, or an arbitrary first
   translation row inside an adapter.
5. Preserve stable `slug` only as a language-neutral identifier fallback for
   security-critical consent presentation where the owner contract explicitly
   permits it.
6. Delete raw SQL references to `oauth_apps.name` and
   `oauth_apps.description` after the port cutover.

Until these paths are removed, the OAuth DB storage contract is implemented but
its complete mounted native/SSR transport compatibility is not verified.
