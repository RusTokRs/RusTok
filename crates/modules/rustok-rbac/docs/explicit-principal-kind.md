# RBAC explicit principal-kind contract

## Purpose

RBAC control-plane admission must consume one trusted principal classification.
It must not independently infer authority from OAuth metadata such as
`client_id`, `grant_type`, or `session_id`.

## Shared contract

`rustok-api::AuthPrincipalKind` is a host-neutral enum with three closed values:

- `DirectUser` — a direct authenticated user with no OAuth client and a non-nil
  session;
- `DelegatedUser` — an authorization-code user represented by an OAuth client
  and no direct session;
- `Service` — a client-credentials service principal represented by an OAuth
  client and no user session.

The enum is available without the `rustok-api/server` feature so owner crates can
consume it without depending on Axum or Async-GraphQL. Server-enabled builds also
expose `AuthPrincipalContext`, the request-scoped typed carrier.

## Construction boundary

The server authentication resolver validates token subject, tenant, app, grant,
session, user status, scopes, and effective permissions. During that resolution it
calls `AuthPrincipalKind::from_authenticated_facts` once and stores the result on
`CurrentUser`.

HTTP/native middleware and GraphQL HTTP/WebSocket composition only copy
`CurrentUser.principal_kind` into `AuthPrincipalContext`. They are forbidden from
reinterpreting grant, client, or session metadata. An unknown or inconsistent
combination is rejected by the authentication resolver before any request context
or module policy is constructed.

## Control-plane admission

`RbacControlPlanePrincipal` contains only:

- the authenticated tenant id;
- the explicit `AuthPrincipalKind`.

The owner policy admits only `DirectUser` with authenticated/routed tenant
equality. `DelegatedUser` and `Service` remain valid for data-plane operations
when their effective permission and OAuth scope checks pass, but they cannot
read or mutate RBAC control-plane state.

GraphQL role reads/writes, REST artifact-role permission writes, and native RBAC
Admin bootstrap all require `AuthPrincipalContext` before checking
`settings:read`, `users:manage`, or `modules:manage`. Missing typed context is an
authentication failure. There is no compatibility fallback to transport metadata.

## Preserved boundaries

- `AuthContext` continues to carry authenticated identity, permissions, scopes,
  and transport metadata for existing consumers.
- The explicit kind is a separate required request context so unrelated
  `AuthContext` fixtures and data-plane consumers are not forced through a
  repository-wide compatibility migration.
- `rustok-rbac` remains independent of Axum and the `rustok-api/server` feature.
- The change adds no second authorization engine, no compatibility wrapper, and
  no parallel legacy/new RBAC execution path.

## Verification

Source guard:

```bash
node scripts/verify/verify-rbac-explicit-principal-kind.mjs
```

Targeted execution evidence:

```bash
cargo check -p rustok-api
cargo check -p rustok-api --features server
cargo check -p rustok-rbac --all-features
cargo check -p rustok-rbac-admin --features ssr
cargo check -p rustok-server --lib
cargo test -p rustok-api --features server authenticated_facts_classify_fail_closed
cargo test -p rustok-server --lib token_claim_classifier_returns_explicit_principal_kinds
cargo test -p rustok-rbac --all-features control_plane
cargo test -p rustok-server --test rbac_artifact_permission_control_plane_guard
node scripts/verify/verify-rbac-admin-tenant-scope.mjs
```

No command result is implied by this document. Same-revision execution and live
negative transport evidence remain required before the RBAC verification item
can be completed.
