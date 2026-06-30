# План реализации `alloy`

Статус: capability runtime зафиксирован; локальная документация и module
contract приведены к единому формату.

## Execution checkpoint

- Current phase: runtime_hardening_verified
- Last checkpoint: Restored executable Alloy compile/test evidence by enabling `rustok-api/server` for the crate, wiring native Rhai operation/call-depth/string/array/map limits into `ScriptEngine`, and keeping compile-stage string pressure mapped to Alloy resource-limit semantics; `cargo xtask module test alloy`, `cargo test -p alloy --lib`, `npm run verify:alloy:runtime-contract`, and `npm run verify:ai-alloy:policy` passed.
- Next step: Promote remaining static route/schema/pagination/scheduler/hook source locks into executable router/schema/runtime integration checks where host test fixtures permit, then continue MCP/Admin Alloy draft-review surface work.
- Open blockers: None for the Alloy crate validation path.
- Hand-off notes for next agent: Alloy compile/test gates are no longer blocked. Keep `rustok-api/server` enabled for `alloy` while HTTP/GraphQL controllers use server-gated API context types. Rhai sandbox limits are applied natively in `ScriptEngine::new`; do not remove them or the runtime hardening contract will drift from executable behavior. Static contract paths remain `crates/alloy/contracts/alloy-runtime-contract.json`, `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`, and `scripts/verify/verify-alloy-runtime-contract.mjs`.
- Last updated at (UTC): 2026-06-30T00:00:00Z

## Область работ

- удерживать `alloy` как capability-oriented модуль платформенного script/runtime слоя для скриптов, scheduler и hook execution;
- синхронизировать runtime contract, `ModuleRegistry` wiring и local docs;
- развивать script platform без превращения `alloy` в tenant-scoped бизнес-модуль.

## Текущее состояние

- storage, migrations и execution log уже встроены в capability crate;
- `ScriptEngine`, `ScriptOrchestrator`, `Scheduler` и bridge/helper слой уже составляют базовый runtime;
- GraphQL/HTTP transport surfaces живут внутри `alloy`, а host подключает их через generated module wiring;
- `AlloyModule` зарегистрирован как обычный optional модуль и публикует script permission surface;
- локальные docs и root `README.md` теперь входят в scoped module audit path.

## Этапы

### 1. Contract stability

- [x] нормализовать local docs и убрать битую кодировку из module docs;
- [x] удерживать `alloy` в module-standard verification path;
- [x] удерживать sync между host wiring, transport surfaces и capability metadata.

### 2. Runtime hardening

- [x] довести resource limits, timeout semantics и sandbox guarantees до стабильного production contract;
- [x] удерживать audit log и execution history как каноническую операторскую поверхность с DB-level pagination и exact scoped total metadata;
- [x] выровнять in-memory registry pagination с DB ordering contract для deterministic non-DB runtime/test paths;
- [x] зафиксировать runtime route/schema/pagination/sandbox/scheduler/hook/script CRUD validation contract в machine-readable static gate без компиляции;
- [x] расширять integration helpers только через явные phase-aware contracts.

### 3. Operability

- [x] развить runbook для scheduler/runtime failures и hook debugging;
- [x] покрыть execution, scheduler, bridge invariants и canonical transport field mapping точечными tests;
- [x] документировать новые runtime guarantees одновременно с изменением capability surface.

## Проверка

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- `npm run verify:alloy:runtime-contract`
- targeted runtime tests для script execution, scheduling, tenant isolation и bridge semantics

## Правила обновления

1. При изменении runtime contract сначала обновлять этот файл.
2. При изменении public/capability surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или host wiring синхронизировать `rustok-module.toml`.


## Quality backlog

- [x] Актуализировать no-compile static coverage по ключевым route/schema/pagination сценариям модуля.
- [ ] Повысить static coverage до executable Rust integration tests после снятия запрета на компиляцию.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
