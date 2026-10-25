# rustok-rbac

## Purpose

`rustok-rbac` owns the tenant-scoped policy authorization runtime for RusToK.

## Responsibilities

- Provide `RbacModule` metadata for the runtime registry.
- Resolve effective permissions from relation data.
- Own persisted relation reads and tenant membership deletion; server adapters
  delegate mutations inside the host transaction.
- Own transaction-backed role-mutation fact collection, exact assignment checks,
  and active-super-administrator continuity for role/status/account changes.
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
- `SeaOrmRelationPermissionStore`
- `authorize_current_permission`
- `resolve_persisted_permissions_on`
- `PermissionResolver`
- `authorize_permission`
- `authorize_any_permission`
- `authorize_all_permissions`
- `has_effective_permission_in_set`
- `RbacArtifactPermissionAssignmentService`
- `SeaOrmArtifactPermissionAuthorizer`
- `graphql::RbacQuery` / `graphql::RbacMutation` (feature `graphql`)

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)

## Authorization model (canonical)

- The persisted relation graph (`roles`, `permissions`, `user_roles`,
  `role_permissions`) is the only source of truth for grants. Static built-in
  role permission sets in `rustok-core` are bootstrap defaults for new tenants
  only; after seeding, tenant grant changes are legitimate persisted state.
  System-role repair (an explicit operator action) restores missing default
  roles with their bootstrap grants and never rewrites an existing role's
  grants; it is not part of the normal request flow, so roles deleted through
  the control plane stay deleted.
- The decision engine is the platform's own set-based evaluator over the
  resolved permission set (see
  `DECISIONS/2026-02-26-rbac-relation-source-of-truth-cutover.md`).
- Default roles (`super_admin`, `admin`, `manager`, `customer`) are seeded
  as database records per tenant from the bootstrap defaults in `rustok-core`
  and are thereafter owned by the database. Only the super administrator
  role can never be deleted (break-glass account protection; its grant set
  is also immutable through the GraphQL control plane). Every other role —
  the seeded defaults and custom roles alike — can be renamed, have its
  grants edited within the actor's authority ceiling, and be deleted while
  it is not assigned to any user. Role mutations validate every grant
  against the canonical permission vocabulary and the actor's own
  effective authority ceiling.

## Module availability (dormant grants)

Modules own slices of the permission vocabulary; roles are tenant records
that mix grants across modules. Disabling a module for a tenant never
deletes or hides roles or their grants — the module's grants become
*dormant*:

- **Enforcement:** the host strips dormant grants from the request
  permission snapshot at authentication time (`PermissionAvailability`), so
  disabling a module immediately revokes its capabilities everywhere, while
  the persisted grants reactivate unchanged when the module is re-enabled.
  Platform-core permissions (no registered owner module) are always active.
- **Control plane:** `platformPermissions` annotates every permission with
  `moduleSlug` / `moduleStatus` (`core` / `enabled` / `disabled`), and
  `RoleInfo.inactivePermissions` lists a role's dormant grants so operators
  see the state instead of discovering it. The admin UI renders dormant
  grants greyed out.
- **Mutation guard:** creating a role with dormant grants, or adding a new
  dormant grant to an existing role, is rejected. Dormant grants already on
  a role are preserved across saves — removing them is an explicit operator
  decision, never a side effect of saving.
- Hosts that register no `RbacModuleAvailabilityProvider` treat every
  permission as platform-core; availability resolution failures on the
  authentication path fail closed.

## Known Limitations / Pending Implementation

- **Role assignment replaces all tenant memberships with a single role.**
  `assign_user_role` accepts a built-in role enum (`role`) or any
  tenant-persisted role slug (`roleSlug`), including custom roles. Built-in
  assignments enforce the canonical role hierarchy; custom-role assignments
  have no platform rank, so the owner requires every grant of the custom
  role to fit inside the actor's own effective permissions (authority
  ceiling), plus target-management and last-super-administrator continuity
  rules. Multi-role membership is not exposed as an assignment model: the
  read path unions permissions across all persisted memberships, but
  assignment always converges the user to exactly one tenant role.
- **`BuiltinTenantRbacCatalog` publishes platform defaults, not tenant
  state.** The synchronous `TenantRbacCatalog` contract cannot read the
  database, so it exposes the built-in role vocabulary. Tenant-persisted
  roles (including custom roles) are available through the GraphQL `roles`
  query. A database-backed tenant catalog is pending.
- **`SecurityContext::system()` is a trusted-runtime-only constructor.** It
  carries the static super-admin bootstrap set and must only be built by
  platform composition code, never from request data.
