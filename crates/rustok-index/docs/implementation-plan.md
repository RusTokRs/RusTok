# Implementation plan for `rustok-index`

Status: module is locked as the canonical index/read-model layer; local
documentation has been aligned to a unified format.

## Execution checkpoint

- Current phase: phase_b_in_progress + fba_provider_in_process_adapter_source_lock
- Last checkpoint: no-compile increment added source-locked in-process adapter seams: `InProcessIndexReadModelAdapter` implements `IndexReadModelPort` for read/list smoke with selector/tenant/type/locale/limit filters, `RebuildDisabledIndexAdapter` implements typed disabled `IndexRebuildPort`, and `verify:index:fba` locks adapter metadata without running Rust compilation.
- Next step: Connect persistence-backed adapter over the current in-process seams and collect Rust runtime contract evidence; until then, status remains `in_progress`.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block and the central FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-26T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - Admin native server-function transport no longer imports `loco_rs::app::AppContext`; it consumes host-provided `rustok_api::HostRuntimeContext`, and this is guarded by `scripts/verify/verify-api-surface-contract.mjs`;
  - Foundation FBA batch update: `npm run verify:index:fba` now runs `npm run verify:foundation:fba-runtime-smoke`, so `crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json` is checked together with `channel`, `tenant` and `email` runtime fallback evidence instead of only as a standalone index gate.
  - Boundary readiness update: `crates/rustok-index/contracts/index-fba-registry.json`, `crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json` and `crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json` are locked by `npm run verify:index:fba`; FBA status is `boundary_ready`, while persistence-backed Rust runtime contract execution remains the next step before `transport_verified`.
  - admin package split introduced `admin/src/core.rs` for Leptos-free view-model/error formatting, `admin/src/transport/` for the native server-function bootstrap facade, and `admin/src/ui/leptos.rs` as the only render adapter;
  - current admin bootstrap is an intentional temporary native-only single-adapter state because `rustok-index` had no legacy GraphQL/REST operator contract for this overview;
  - central FFA/FBA readiness board is synchronized in `docs/modules/registry.md`;
  - FBA provider slice: `crates/rustok-index/src/ports.rs` declares `IndexReadModelPort` / `index.read_model.v1` for indexed document reads and `IndexRebuildPort` / `index.rebuild.v1` for operator rebuild orchestration with shared `rustok_api::PortContext`/`PortError`, tenant-scope preservation, `PortCallPolicy::read()` deadline semantics and `PortCallPolicy::write()` idempotency/deadline semantics for rebuilds; `crates/rustok-index/contracts/index-fba-registry.json`, `crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json` and `crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json` lock planned contract cases, fallback profiles, no-compile source markers and source-locked in-process adapter seams (`InProcessIndexReadModelAdapter`, `RebuildDisabledIndexAdapter`) under `npm run verify:index:fba`; persistence-backed Rust runtime contract execution remains the next step before `transport_verified`.

## Scope of work

- keep `rustok-index` as an infrastructure module for indexed reads and denormalized projections;
- do not mix the index/read-model layer with product-facing search responsibilities;
- synchronize ingestion contract, rebuild semantics and local docs.

## Current state

- base crate/module structure is already embedded in the workspace;
- operator-facing admin overview is already published through `rustok-index-admin` and split by FFA layers (`core`, native-only `transport`, `ui/leptos`);
- canonical direction is locked: `index` is responsible for ingestion and indexed reads, not for ranking/UX search;
- module is already considered as substrate for cross-module filtering and link-aware queries;
- event-driven consumers are moved to module-owned runtime path through `register_event_listeners(...)`, old host/legacy listener path removed;
- standalone `flex` ingestion now also lives in `IndexModule`: `flex_indexer` supports `index_flex_entries` as module-owned read model for `FlexEntry*` / `FlexSchema*` events;
- boundary `index != search` is now additionally maintained by a contract check in `xtask`, so the read-model layer does not start exporting search-owned engine surfaces again;
- root `README.md`, local docs and manifest metadata are part of the scoped audit path.

## Stages

### 1. Contract stability

- [x] lock the role of `rustok-index` as canonical index/read-model module;
- [x] separate boundary `index != search` at the level of local documentation and ADR;
- [ ] maintain sync between ingestion contracts, runtime dependencies and host integration tests.

### 2. Working index module

- [~] bring ingestion lifecycle: bootstrap, incremental sync, rebuild, retry; current in-process/read-only adapter slice locks shared rebuild policy and degraded-mode fallback, but persistence-backed scheduling/retry is still pending;
- [ ] lock canonical query surface for cross-module filtering and counts;
- [~] bring tenant/locale scoping of indexed records to production-ready contract; current FBA smoke locks tenant-scope guard and locale selector validation, but persistence-backed evidence is still pending.

### 3. Operability

- [ ] cover consistency drift, rebuild duration and sync lag with observable metrics;
- [x] add no-compile runtime fallback smoke for read/list/rebuild provider ports and degraded rebuild-disabled profile;
- [x] lock source-locked in-process adapter seams for read/list and rebuild-disabled runtime profiles without compilation;
- [~] add operator flows for health verification and rebuild control; current admin overview already shows tenant/module/counter bootstrap through FFA native-only transport;
- [ ] document new query/ingestion guarantees simultaneously with changing runtime surface.
- [ ] replace in-memory smoke adapter with persistence-backed runtime adapter evidence when compilation is allowed.

## Verification

- `cargo xtask module validate index`
- `cargo xtask module test index`
- targeted tests for ingestion, rebuild, filtering, consistency drift and tenant/locale scoping
- contract tests cover all public use-case module-owned index/read-model contract, including registration path for `flex_indexer`

## Update rules

1. When changing index/read-model contract, update this file first.
2. When changing public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata or dependency graph, synchronize `rustok-module.toml`.
4. When changing boundary between `index` and `search`, synchronize ADR and central docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
