# rustok-rbac / CRATE_API

## Public Modules
`bootstrap`, `catalog`, `consistency`, `dto`, `error`, `graphql`, `module`, `ports`, `presentation`, `services`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `RbacModule` implementing `RusToKModule` (slug `"rbac"`, kind `ModuleKind::Core`, dependencies `["outbox"]`) and `MigrationSource`.
- **Runtime Extensions**: Registers `SharedTenantRbacCatalog` providing builtin tenant RBAC catalogs.
- **Permission Decision Port**:
  - `RbacPermissionDecisionPort` (`rbac.permission_decision.v1`): Transport-neutral permission decision evaluator (`check_permissions`).
  - `RbacPermissionDecisionProvider` wrapping authoritative `PermissionResolver`.
- **Permission Resolution & Evaluation**:
  - `PermissionResolver`, `RuntimePermissionResolver`, `RelationPermissionStore`
  - `evaluate_all_permissions`, `evaluate_any_permission`, `evaluate_single_permission`
  - `authorize_all_permissions`, `authorize_any_permission`, `authorize_permission`
  - `check_all_permissions`, `check_any_permission`, `check_permission`
  - `has_effective_permission_in_set`, `missing_permissions`
- **Role & Invalidation Management**:
  - `RbacRoleAssignmentDbWriter`, `RbacRoleMutationPlan`, `replace_persisted_user_role_on`
  - `read_permission_invalidation_generation`, `reserve_permission_invalidation_generation`
  - `plan_system_role_repair`, `apply_system_role_repair_in_transaction`

## Domain Collaborations
- **Core Platform Contract**: Reuses canonical `Permission` vocabulary from `rustok-api` and `rustok-core`.
- **Database Migrations**: Applies migration history for RBAC relation tenant integrity, invalidation state, artifact permission catalogs, and localized presentations.
- **Transactional Outbox**: Cooperates with `rustok-outbox` for role mutation events and invalidation broadcasts.

## Primary Public Types and Signatures
- `pub struct RbacModule`
- `pub trait RbacPermissionDecisionPort`
- `pub struct RbacPermissionDecisionProvider<R>`
- `pub struct RbacPermissionCheckRequest`
- `pub struct RbacPermissionCheckResponse`
- `pub struct BuiltinTenantRbacCatalog`

## Dependencies on Other RusToK Crates
- `rustok-api`
- `rustok-core`
- `rustok-events`

## Common AI Mistakes
- Confuses `Resource/Action/Permission` from core with local DTOs.
- Adds permission checks in the wrong layer (instead of application/service boundary).
- Circumvents the permission cache or invalidation generation when updating role assignments.

## Minimum Contract Set

### Input DTOs/Commands
- `RbacPermissionCheckRequest` (`permissions`, `mode`)
- `RbacUserRoleMutationRequest`
- `RbacRoleAssignmentCommand`
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Super admin continuity invariant: Cannot revoke or delete the last active super admin for a tenant.
- Tenant isolation: All role queries and permission evaluations are strictly tenant-scoped.
- Permission invalidation: Role mutations must advance the permission invalidation generation before commit.

### Events / Outbox Side Effects
- Role assignments and mutations emit lifecycle events through the canonical outbox before commit.

### Errors / Failure Codes
- `RbacError`, `RbacRoleMutationPolicyError`, `RbacSystemRoleRepairError`, `RbacPresentationStoreError` represent typed failure contracts.
