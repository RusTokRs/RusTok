# План реализации `rustok-outbox`

Статус: core outbox baseline зафиксирован; модуль приведён к единому
manifest/doc contract.

## Execution checkpoint

- Current phase: fba_write_policy_alignment
- Last checkpoint: OutboxRelayPort использует canonical `rustok_core::ports` primitives без dependency cycle через `rustok-api`; relay control enforce-ит `PortCallPolicy::write()` через module-local policy helper, а relay-owned request/projection DTOs остались локальными.
- Next step: Расширить relay/backlog/DLQ evidence без долгой full-workspace компиляции и затем добавить targeted runtime contract/fallback smoke, когда компиляции снова разрешены.
- Open blockers: None.
- Hand-off notes for next agent: Сохранять read-only admin UI поверх module-owned transport facade; не переносить relay/runtime ownership в host UI.
- Last updated at (UTC): 2026-06-22T00:00:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence / notes:
  - пакетный owner gate `scripts/verify/verify-owner-fba-runtime-order.mjs` проверяет `crates/rustok-outbox/contracts/evidence/outbox-provider-runtime-order-smoke.json`: canonical `rustok_core::ports` write policy helper, deadline/idempotency error mapping, relay invocation до metrics projection и fallback/degraded parity; registry/manifest metadata исправлены с устаревшего `rustok_api::*` на фактический `rustok_core::ports::*`, статус остаётся `in_progress` до live relay execution;
  - admin UI имеет явный FFA split: `admin/src/lib.rs` только wiring/re-export, `admin/src/core.rs` содержит Leptos-free DTO/view-model helpers, `admin/src/transport/` владеет native server-function facade, `admin/src/ui/leptos.rs` владеет Leptos rendering;
  - GraphQL/REST fallback не добавлялся в этом срезе, потому что legacy outbox admin surface был native-only read-only bootstrap; это temporary single-adapter state до появления headless parity requirement для operator UI;
  - fast evidence: `cargo check -p rustok-outbox-admin --lib` (25.04s, без full-workspace build), `node scripts/verify/verify-outbox-admin-boundary.mjs`, `node scripts/verify/verify-outbox-admin-boundary.test.mjs`;
  - fast evidence: `cargo check -p rustok-outbox-admin --lib` (25.04s, без full-workspace build);
  - compile-free FFA evidence: `npm run verify:outbox:admin-boundary` validates that UI uses only the module-owned transport facade, `core.rs` remains Leptos/server-function free, generated native server functions stay private to `transport/native_server_adapter.rs`, and host-provided `UiRouteContext.locale` remains the locale source;
  - FBA provider slice: `crates/rustok-outbox/contracts/outbox-fba-registry.json` + `crates/rustok-outbox/src/ports.rs` declare `OutboxRelayPort` / `outbox.relay_control.v1` for relay worker control with canonical `rustok_core::ports::PortContext`/`PortError`, `PortCallPolicy::write()` deadline/idempotency semantics and static evidence packet `crates/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json` verified by `npm run verify:outbox:fba`; status remains below `boundary_ready` until executable runtime contract/fallback smoke lands.

## Область работ

- удерживать `rustok-outbox` как bounded-context модуль transactional publishing;
- синхронизировать relay/runtime contract, local docs и manifest metadata;
- развивать operational guarantees без размазывания event runtime contract по host-слою.

## Текущее состояние

- write-side transactional publishing contract уже реализован;
- relay/retry/DLQ semantics уже входят в базовый runtime surface;
- модуль публикует admin visibility через `rustok-outbox-admin`, где UI split выровнен до `core/transport/ui`;
- root README, local docs и manifest contract входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] выровнять root README, local docs и manifest metadata под единый standard path;
- [x] зафиксировать transactional publishing как основной bounded-context contract;
- [x] выделить FFA `core/transport/ui` boundary для read-only admin visibility surface;
- [x] добавить compile-free FFA boundary verifier для read-only admin visibility surface;
- [ ] удерживать sync между public crate API и server event-runtime tests;
- [ ] контрактные тесты покрывают все публичные use-case для transactional publishing, relay, retry и DLQ semantics.

### 2. Runtime hardening

- [x] добавить no-compile FFA boundary verifier для read-only admin split и fixture regression suite;
- [ ] расширить automated tests вокруг relay/backlog/DLQ boundary behavior;
- [ ] документировать новые runtime guarantees вместе с изменениями event transport contract;
- [ ] держать observability и operability частью delivery readiness, а не постфактум.

### 3. Productionization

- [ ] уточнить rollout и migration strategy для incremental adoption;
- [ ] завершить security/tenancy/rbac checks, которые реально относятся к модулю;
- [ ] удерживать incident runbook в sync с operational semantics.

## Проверка

- `npm run verify:outbox:admin-boundary`
- `npm run test:verify:outbox:admin-boundary`
- `npm run verify:outbox:fba`
- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- `node scripts/verify/verify-outbox-admin-boundary.mjs`
- `node scripts/verify/verify-outbox-admin-boundary.test.mjs`
- `npm run verify:outbox:fba`
- targeted event-runtime tests для transactional publish, relay, retry и DLQ semantics

## Правила обновления

1. При изменении transactional publishing или relay contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
