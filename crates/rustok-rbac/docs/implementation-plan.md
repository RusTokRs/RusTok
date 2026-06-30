# План реализации `rustok-rbac`

Статус: переход на single-engine Casbin runtime завершён; модуль удерживается в
steady-state hardening и drift-prevention режиме.

## Execution checkpoint

- Current phase: phase_b_in_progress
- Last checkpoint: RBAC admin FFA guardrail добавил fast boundary verifier `scripts/verify/verify-rbac-admin-boundary.mjs` и fixture suite `scripts/verify/verify-rbac-admin-boundary.test.mjs` для canonical split, legacy `api.rs`, Leptos-specific core, raw adapter calls, package-local GraphQL fallback и misplaced `#[server]` endpoints без долгой Rust-компиляции.
- Next step: Расширить operator flows/verification для role and permission management surfaces; GraphQL/REST fallback добавлять только если такой remote/headless admin contract будет утверждён, а текущий native-only overview удерживать быстрыми boundary guardrails.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и central FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-19T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` проверяет `crates/rustok-rbac/contracts/evidence/rbac-provider-runtime-order-smoke.json`: read policy предшествует request validation и claims evaluation, а fallback/degraded metadata остаются синхронны registry; статус остаётся `in_progress` до live host execution;
  - admin package split introduced `admin/src/core.rs` for Leptos-free overview view-model/error formatting, `admin/src/transport/` for the native server-function bootstrap facade, and `admin/src/ui/leptos.rs` as the only render adapter;
  - current admin bootstrap is an intentional temporary native-only single-adapter state because `rustok-rbac` had no legacy GraphQL/REST operator contract for this overview;
  - central FFA/FBA readiness board is synchronized in `docs/modules/registry.md`;
  - FBA provider slice: `crates/rustok-rbac/src/ports.rs` declares `RbacPermissionDecisionPort` / `rbac.permission_decision.v1` for admin permission-decision consumers with typed `PortContext`/`PortError`, read deadline semantics, claims-scope preservation and serializable DTOs; `crates/rustok-rbac/contracts/rbac-fba-registry.json` plus `crates/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` lock planned contract cases and fallback profiles under `npm run verify:rbac:fba` while runtime fallback smoke remains pending before `boundary_ready`;
  - `scripts/verify/verify-rbac-admin-boundary.mjs` and `scripts/verify/verify-rbac-admin-boundary.test.mjs` enforce Leptos-free core, facade-only UI transport calls, native-only overview exception, typed transport error envelope and server-function adapter placement without full Rust compilation.

## Область работ

- удерживать `rustok-rbac` как единственную каноническую границу RBAC runtime;
- синхронизировать permission contracts, integration events и server adapters;
- не допускать возврата к shadow-runtime, rollout-mode или server-owned policy logic.

## Текущее состояние

- relation-store остаётся source of truth для role/permission assignments;
- live authorization выполняется только через Casbin-backed evaluator;
- `RuntimePermissionResolver` и related contracts уже живут в модуле, а `apps/server` держит только adapters и observability;
- operator-facing admin overview уже опубликован через `rustok-rbac-admin` и разделён по FFA слоям (`core`, native-only `transport`, `ui/leptos`);
- local docs, root `README.md` и manifest metadata входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] зафиксировать single-engine runtime contract;
- [x] перенести policy/evaluator semantics и resolver APIs в модуль;
- [x] стандартизировать integration events для role-assignment changes;
- [ ] удерживать sync между runtime contracts, server adapters и module metadata (tenant module adapters выровнены: `module_registry`/`tenant_modules` и tenant admin bootstrap теперь проверяют tenant-scoped read/list/manage permissions);
- [ ] контрактные тесты покрывают все публичные use-case для permission resolution, authorization decisions, cache semantics и integration events.

### 2. Drift prevention

- [ ] держать periodic verification зелёным для RBAC/server integration;
- [ ] продолжать вычищать presentation-only role inference вне primary authorization path;
- [~] расширять guardrails при появлении новых RBAC-managed surfaces; текущий admin overview уже показывает live permission snapshot и module-declared catalog через FFA native-only transport.

### 3. Operability

- [ ] удерживать decision/cache/latency telemetry частью live contract;
- [ ] документировать runbooks и adapter expectations вместе с изменениями runtime surface;
- [ ] покрывать новые event contracts и resolver paths точечными integration tests.

## Проверка

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- targeted tests для permission resolution, authorization decisions, cache semantics и integration events

## Правила обновления

1. При изменении RBAC runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata, dependency graph или verification expectations синхронизировать `rustok-module.toml` и профильные verification docs.
4. При изменении live contract обновлять также `apps/server/docs/README.md`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
