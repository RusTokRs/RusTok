# rustok-rbac

## Purpose

`rustok-rbac` owns the tenant-scoped policy authorization runtime for RusToK.

## Responsibilities

- Provide `RbacModule` metadata for the runtime registry.
- Resolve effective permissions from relation data.
- Evaluate permission checks through the single live policy engine.
- Publish the typed `settings:*` and `logs:*` platform-admin surface used by server adapters.
- Own explicit tenant-role grants for immutable artifact permission keys without
  extending the static `Permission` enum or writing `role_permissions`.
- Own the RBAC relation-integrity and durable invalidation-generation migrations.

## Interactions

- Depends on `rustok-core` for permission vocabulary and module contracts.
- Used by `apps/server` through `RbacService`, RBAC extractors, and permission-aware
  `SecurityContext` creation.
- Owns its GraphQL role query/mutation/types under `rustok_rbac::graphql`; `apps/server`
  only composes those roots and provides a role-writer adapter to the runtime persistence layer.
- Exposes a module-owned Leptos admin overview through `rustok-rbac-admin`.
- Other runtime modules do not need a direct dependency on `rustok-rbac`; they publish typed
  permissions via `rustok-core`, and server transport layers enforce them through this module.
- Manual role-based authorization in `apps/server` is not part of the live contract.

## Entry points

- `RbacModule`
- `RuntimePermissionResolver`
- `PermissionResolver`
- `authorize_permission`
- `authorize_any_permission`
- `authorize_all_permissions`
- `has_effective_permission_in_set`
- `RbacArtifactPermissionAssignmentService`
- `SeaOrmArtifactPermissionAuthorizer`
- `graphql::RbacQuery` / `graphql::RbacMutation` (feature `graphql`)

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
