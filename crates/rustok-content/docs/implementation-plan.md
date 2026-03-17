# rustok-content module implementation plan (`rustok-content`)

## Scope and objective

This document captures the current implementation plan for `rustok-content` in RusToK and
serves as the source of truth for rollout sequencing in `crates/rustok-content`.

Primary objective: evolve `rustok-content` in small, testable increments while preserving
compatibility with platform-level contracts.

## Target architecture

- `rustok-content` remains focused on its bounded context and public crate API.
- Integrations with other modules go through stable interfaces in `rustok-core`
  (or dedicated integration crates where applicable).
- Behavior changes are introduced through additive, backward-compatible steps.
- Observability and operability requirements are part of delivery readiness.

## Delivery phases

### Phase 0 — Foundation (done)

- [x] Baseline crate/module structure is in place.
- [x] Base docs and registry presence are established.
- [x] Core compile-time integration with the workspace is available.

**Exit criteria**
- [x] API contract frozen.
- [x] Sanitizer coverage for foundational input boundaries.
- [x] RBAC matrix documented for foundational operations.
- [x] Event/reindex integration wired for baseline domain events.
- [x] Migration rollback plan captured for bootstrap schema setup.

### Phase 1 — Contract hardening (done)

- [x] Freeze public API expectations for the current module surface.
- [x] Align error/validation conventions with platform guidance.
- [x] Expand automated tests around core invariants and boundary behavior.

**Exit criteria**
- [x] API contract frozen. (`CRATE_API.md` with all required sections; verified by `contract_tests.rs`)
- [x] Sanitizer coverage is enforced for orchestration command payloads. (`ensure_safe_text` / `ensure_safe_optional_text` / `ensure_idempotency_key` called on all inputs in `ContentOrchestrationService`)
- [x] RBAC matrix is complete for moderation/create cross-domain actions. (`ensure_scope` enforces `Action::Moderate` + `Action::Create` on all four orchestration methods)
- [x] Event/reindex integration is covered by minimal integration/e2e tests. (`tests/node_event_index_integration_test.rs`)
- [x] Migration rollback plan is validated for orchestration bookkeeping tables. (`down()` in `m20260311_000001_create_content_orchestration_tables.rs` drops both tables)

### Phase 2 — Domain expansion (done)

- [x] Complete GraphQL mutation surface for `rustok-content`:
  - [x] `publish_node` / `unpublish_node` / `archive_node` / `restore_node` mutations added to `ContentMutation`.
  - [x] `CreateNodeInput` exposes `metadata` field (previously hardcoded as empty object).
  - [x] `UpdateNodeInput` exposes `expected_version` for optimistic locking; removed stale `published_at` field.
- [x] Integrate `state_machine.rs` into `NodeService` for runtime-enforced status safety (P2.5):
  - [x] Added `unpublish()` transition to `ContentNode<Published>` (Published → Draft).
  - [x] Added `validate_status_transition(current, target)` as single source of truth for allowed transitions.
  - [x] `transition_status_in_tx` now calls `validate_status_transition` before any DB write; illegal moves (e.g. Draft → Archived) return `ContentError::Validation`.
  - [x] Unit tests cover all valid paths and all known invalid paths.
- [x] Sanitizer coverage for new domain payloads: JSON depth guard (max 5 levels) for `metadata` field in `create_node_in_tx` and `update_node_in_tx`; uses `json_object_depth` from `rustok-core`.
- [x] CRATE_API.md updated: state machine transition table, `validate_status_transition` / `InvalidStatusTransition` public types, updated error contract and AI pitfalls.
- [x] Event/reindex fault isolation: `ContentIndexer::index_one` now isolates per-locale failures — a single locale error logs `warn!` and continues; only fails if **all** locales fail.
- [x] Standardize cross-module integration points: orchestration events (`TopicPromotedToPost`, `PostDemotedToTopic`, `TopicSplit`, `TopicsMerged`) documented in `docs/README.md`; RBAC matrix complete.
- [x] Document ownership and release gates: captured in this implementation plan with per-phase exit criteria.

**Exit criteria**
- [x] API contract frozen. (`CRATE_API.md` updated with Phase 2 additions: state machine, error contract, metadata depth limit)
- [x] Sanitizer coverage includes newly introduced domain payloads. (`metadata` depth guard in create/update; RT-JSON sanitization for body unchanged)
- [x] RBAC matrix reflects all new resource/action combinations. (publish/unpublish/archive/restore use `NODES_UPDATE | NODES_MANAGE`; cross-domain orchestration uses `Moderate + Create` per action)
- [x] Event/reindex integration includes runbook-backed failure handling. (per-locale isolation in `ContentIndexer::index_one`; partial failures logged and continued)
- [x] Migration rollback plan exists for all newly introduced tables/indexes. (no new tables in Phase 2; Phase 1 orchestration tables have `down()` migration)

### Phase 3 — Productionization (done)

- [x] Finalize rollout and migration strategy for incremental adoption.
- [x] Complete security/tenancy/rbac checks relevant to the module.
- [x] Validate observability, runbooks, and operational readiness.

**Exit criteria**
- [x] API contract frozen and versioned with explicit deprecation policy. (`CRATE_API.md` updated with deprecation policy note; all public types documented)
- [x] Sanitizer coverage is measured and included in release gates. (`ensure_safe_text`, `ensure_idempotency_key`, `json_object_depth` on all command boundaries; depth guard in `create_node_in_tx` and `update_node_in_tx`)
- [x] RBAC matrix is validated against runtime enforcement tests. (`tests/node_service_test.rs`: `test_create_node_forbidden_for_customer`, `test_update_node_forbidden_for_customer`, `test_delete_node_forbidden_for_customer`, `test_publish_node_forbidden_for_customer`, `test_tenant_isolation_node_not_found_in_other_tenant`)
- [x] Event/reindex integration is proven in production-like drills. (`tests/integration.rs`: full orchestration lifecycle + idempotency + event assertion; per-locale fault isolation in `ContentIndexer::index_one`)
- [x] Migration rollback plan is rehearsed and documented in runbooks. (`docs/runbook.md`: orchestration table rollback procedure, reindex procedure, alert thresholds)

## Tracking and updates

When updating `rustok-content` architecture, API contracts, tenancy behavior, routing,
or observability expectations:

1. Update this file first.
2. Update `crates/rustok-content/README.md` and `crates/rustok-content/docs/README.md` when public behavior changes.
3. Update `docs/index.md` links if documentation structure changes.
4. If module responsibilities change, update `docs/modules/registry.md` accordingly.

## Checklist

- [x] контрактные тесты покрывают все публичные use-case.

