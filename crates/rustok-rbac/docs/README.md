# rustok-rbac docs

В этой папке хранится документация модуля `crates/rustok-rbac`.

## Documents

- [Implementation plan](./implementation-plan.md)

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)


## Runtime contracts

- `PermissionResolver` / `PermissionResolution` define tenant-aware RBAC resolver contract for adapter-based integrations (`apps/server` and future transports), including default `has_permission/has_any_permission/has_all_permissions` use-cases powered by module evaluator APIs.
- `permission_policy` and `permission_evaluator` remain canonical policy/evaluation helpers for allow/deny semantics.
- `RuntimePermissionResolver` composes relation-store/cache/role-assignment adapters and supports unified resolver error mapping (`Into<E>`) so integration layers can keep their own adapter-specific error types; the same resolver contract is used for both read-path permission resolution and write-path role assignment/replacement/removal use-cases.
- `RbacAuthzMode` defines shared rollout-mode parsing for relation-only/dual-read authorization execution (`RUSTOK_RBAC_AUTHZ_MODE`, accepts `relation_only|relation-only|relation` and `dual_read|dual-read|dual`) and supports legacy rollout toggle compatibility via `RUSTOK_RBAC_RELATION_DUAL_READ_ENABLED` (aliases: `RBAC_RELATION_DUAL_READ_ENABLED`, `rbac_relation_dual_read_enabled`) when mode is not explicitly set. `RbacAuthzMode::try_parse` exposes typed validation errors (`RbacError::InvalidAuthzMode`) for strict callers, while `parse` keeps backward-compatible fallback to `RelationOnly`.
- `casbin_shadow_evaluator` exports `evaluate_casbin_shadow`, `compare_casbin_shadow_decision`, `evaluate_casbin_shadow_comparison`, and `permissions_for_shadow_check` for matcher-compatible in-module Casbin shadow checks and unified mismatch payload generation (`single|any|all`) without changing active decision path; `CasbinShadowComparison::checked_permissions_total()` provides stable mismatch cardinality for adapter telemetry.

- `shadow_decision` exports legacy-vs-relation shadow comparison helpers (`ShadowCheck` + `compare_shadow_decision`, as well as `compare_single_permission/compare_any_permissions/compare_all_permissions`) to keep dual-read decision semantics in the RBAC module; `ShadowCheck::as_str()` provides a stable mode label (`single|any|all`) for adapter-level observability tags.
- `shadow_dual_read` exports `evaluate_dual_read` + `DualReadOutcome` to centralize dual-read orchestration (`skipped` + compared decision with match/mismatch semantics) and keep transport layers adapter-only.
- `integration` exports canonical RBAC cross-module event contract for role-assignment change notifications: `RbacRoleAssignmentEvent`, `RbacIntegrationEventKind`, and stable event-type constants (`rbac.role_permissions_assigned`, `rbac.user_role_replaced`, `rbac.tenant_role_assignments_removed`, `rbac.user_role_assignment_removed`). Integration payloads are `serde`-serializable (`snake_case` enum tags) for transport-agnostic publish/consume flows.


## Ownership and release gates

- **Module owner:** Platform foundation team (`apps/server` + `crates/rustok-core` ownership group from repository map).
- **Change scope requiring owner review:**
  - public API exports in `crates/rustok-rbac/src/lib.rs`;
  - resolver contracts (`PermissionResolver`, `RuntimePermissionResolver`, relation/cache assignment traits);
  - rollout controls (`RbacAuthzMode`, dual-read contracts);
  - integration event contracts under `integration` (`rbac.*` event-type constants and payload structs).
- **Release gate checklist for RBAC module changes:**
  1. Unit tests for changed domain logic are present/updated in `crates/rustok-rbac/src/**` (or explained why not needed).
  2. `rustfmt` passes for touched Rust files.
  3. `apps/server` adapter compatibility is validated (compile/tests in network-enabled CI or documented local limitation).
  4. Module docs are updated (`crates/rustok-rbac/docs/*`) and, when migration milestones change, central architecture docs are synced (`docs/architecture/rbac-relation-migration-plan.md`).


## Incremental rollout strategy

For safe incremental adoption of relation RBAC in integration layers (`apps/server` and future transports), use the following sequence:

1. **relation-only baseline**: keep `RUSTOK_RBAC_AUTHZ_MODE=relation_only` (or alias `relation`) as the authoritative decision source.
2. **dual-read shadow**: switch to `RUSTOK_RBAC_AUTHZ_MODE=dual_read` for comparison with legacy-role shadow decisions while keeping relation decision authoritative.
3. **mismatch burn-down**: track mismatch/compare-failure signals in transport observability and resolve drift sources before cutover.
4. **relation-only cutover**: revert mode to `relation_only` and keep dual-read disabled after a stable zero-mismatch window.

Compatibility note: when `RUSTOK_RBAC_AUTHZ_MODE` is absent, legacy feature-flag aliases remain supported for transitional environments: dual-read (`RUSTOK_RBAC_RELATION_DUAL_READ_ENABLED`, `RBAC_RELATION_DUAL_READ_ENABLED`, `rbac_relation_dual_read_enabled`), relation enforcement (`RUSTOK_RBAC_RELATION_ENFORCEMENT_ENABLED`, `RBAC_RELATION_ENFORCEMENT_ENABLED`, `rbac_relation_enforcement_enabled`), and temporary legacy fallback (`RUSTOK_RBAC_LEGACY_ROLE_FALLBACK_ENABLED`, `RBAC_LEGACY_ROLE_FALLBACK_ENABLED`, `rbac_legacy_role_fallback_enabled`). In alias-only mode, precedence is: casbin enforcement > casbin shadow > relation dual-read > relation enforcement > legacy fallback.
