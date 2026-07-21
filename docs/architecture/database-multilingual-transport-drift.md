# Multilingual database transport drift

Last reviewed: 2026-07-21

This document records transport/read-model compatibility with the accepted
language-agnostic database schema. It is not permission to restore localized
copy to base tables.

## OAuth application copy — resolved

The authoritative schema removes `oauth_apps.name` and
`oauth_apps.description`. Localized presentation copy lives in
`oauth_app_translations`, and runtime `und` fallback is forbidden.

The three formerly incompatible adapters are now cut over:

- `crates/rustok-auth/admin/src/transport/native_server_adapter.rs` calls the
  owner `OAuthAdminPort` with the host-resolved `RequestContext.locale`;
- `apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs`
  delegates to the owner Auth admin transport instead of querying OAuth tables;
- `crates/rustok-channel/admin/src/transport/native_server_adapter.rs` keeps
  the Channel permission boundary and performs an exact
  `(tenant_id, app_id, locale)` translation join for PostgreSQL, MySQL, and
  SQLite.

All paths reject absent, invalid, or storage-only `und` runtime locale. They do
not select English, tenant default, `und`, or an arbitrary translation row.
Missing exact copy fails closed rather than silently hiding or relabeling an
OAuth application.

The executable guard is part of the `oauth_apps` surface in
`database-multilingual-contract.json`; the standard DB verifier rejects any
return of raw `oauth_apps.name` / `oauth_apps.description` projections.
