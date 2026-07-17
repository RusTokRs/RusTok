# `rustok-rbac` Documentation

`rustok-rbac` — canonical RBAC runtime module in RusToK. Local
documentation for this module must live inside the crate, not spread across
`docs/architecture/*` or server-only notes.

## Purpose

- publish a unified RBAC runtime contract for permission resolution and checking;
- keep permission policy/evaluator and integration event contracts inside the module;
- keep `apps/server` in the adapter/wiring layer role, not as a second RBAC runtime.

## Scope

- relation-based source of truth: `roles`, `permissions`, `user_roles`, `role_permissions`;
- `PermissionResolver`, `RuntimePermissionResolver`, policy/evaluator and tenant policy authorization flow;
- cross-module event contracts for role assignment changes;
- permission-aware runtime contracts and typed RBAC primitives in conjunction with `rustok-core`;
- absence of rollout-mode and shadow-runtime logic in the live surface.

## Integration

- `apps/server` owns only the adapter/wiring layer: store adapters, cache integration, transport extractors and observability;
- GraphQL role query/mutation/types live in `rustok-rbac`; `apps/server` only composes roots and passes adapter role records to runtime persistence;
- `rustok-core` remains the owner of typed primitives (`Permission`, `Resource`, `Action`, `SecurityContext`);
- live authorization goes only through tenant policy evaluation, without a relation-only/shadow parity path;
- `RbacPermissionDecisionPort` resolves its tenant/user decision through the
  authoritative `PermissionResolver`; request claims are not used as an
  independent permission source;
- `RbacArtifactPermissionCatalog` is the durable owner adapter for immutable
  artifact permission vocabulary. It stores localized labels/descriptions by
  scope and admitted installation identity, is idempotent for retries, and
  never writes `roles` or `role_permissions` during registration. Its owner
  migration is aggregated by `rustok-migrations::Migrator`, the installer and
  CLI schema path used by production hosts;
- `RbacArtifactPermissionAssignmentService` owns explicit, idempotent
  tenant-role grants and revocations for that vocabulary in
  `rbac_artifact_role_permissions`; it validates the exact installation and
  platform-or-tenant catalog scope before writing, records the acting operator
  in its durable operation ledger, and never mutates static `role_permissions`.
  `SeaOrmArtifactPermissionAuthorizer` resolves the matching role-derived grant
  for an exact tenant, user, installation, and permission key. The platform
  artifact HTTP and command routes are runtime consumers; they never interpret
  a module-defined permission as a static `Permission` enum value;
- the operator-facing admin overview lives in `rustok-rbac-admin` and is structured as FFA `core` + native-only `transport` + `ui/leptos` adapter;
- new public RBAC surfaces and event contracts require synchronization of module docs, server docs and verification plan.

## Observability and release gates

Canonical runtime signals:

- `rustok_rbac_permission_cache_hits`
- `rustok_rbac_permission_cache_misses`
- `rustok_rbac_permission_checks_allowed`
- `rustok_rbac_permission_checks_denied`
- `rustok_rbac_claim_role_mismatch_total`
- `rustok_rbac_engine_decisions_policy_total`
- `rustok_rbac_engine_eval_duration_ms_total`
- `rustok_rbac_engine_eval_duration_samples`

Release gates for changes in the module:

- update unit tests for changed domain logic;
- verify compatibility with server adapters;
- synchronize `README.md`, local docs and verification docs;
- do not reintroduce rollout-mode or a second live authorization path.

## Verification

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- targeted tests for permission resolution, tenant policy decisions and integration events

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Verification plan](../../../docs/verification/rbac-server-modules-verification-plan.md)
