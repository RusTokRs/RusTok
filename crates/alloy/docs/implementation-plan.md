# План реализации `alloy`

Статус: capability runtime зафиксирован; локальная документация и module
contract приведены к единому формату.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

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
- [ ] удерживать sync между host wiring, transport surfaces и capability metadata.

### 2. Runtime hardening

- [ ] довести resource limits, timeout semantics и sandbox guarantees до стабильного production contract;
- [ ] удерживать audit log и execution history как каноническую операторскую поверхность;
- [ ] расширять integration helpers только через явные phase-aware contracts.

### 3. Operability

- [ ] развить runbook для scheduler/runtime failures и hook debugging;
- [ ] покрыть execution, scheduler и bridge invariants точечными integration tests;
- [ ] документировать новые runtime guarantees одновременно с изменением capability surface.

## Проверка

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- targeted runtime tests для script execution, scheduling, tenant isolation и bridge semantics

## Правила обновления

1. При изменении runtime contract сначала обновлять этот файл.
2. При изменении public/capability surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или host wiring синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
