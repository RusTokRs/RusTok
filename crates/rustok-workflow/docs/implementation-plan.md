# План реализации `rustok-workflow`

Статус: workflow module уже имеет рабочий execution/runtime baseline; ключевая
задача сейчас — удерживать границу между orchestration layer, event contracts и
capability integrations без дрейфа и битой документации.

## Область работ

- удерживать `rustok-workflow` как owner workflow execution domain;
- синхронизировать triggers, steps, transport/UI surfaces и local docs;
- не допускать превращения workflow в отдельный event transport или generic scripting bucket.

## Текущее состояние

- workflow storage и execution journal уже определены внутри модуля;
- engine, trigger handlers, cron/manual/webhook/event triggers и базовые step types уже составляют рабочий baseline;
- GraphQL, REST/webhook ingress и module-owned admin UI уже живут внутри модуля;
- интеграция с `alloy` уже является capability-level step integration, а не registry-level hard dependency.

## Этапы

### 1. Contract stability

- [x] закрепить workflow engine и execution journal как module-owned runtime;
- [x] зафиксировать transport adapters и admin UI внутри модуля;
- [x] нормализовать local docs и убрать битую кодировку из module docs;
- [ ] удерживать sync между workflow runtime contract, UI surfaces и module metadata.

### 2. Execution hardening

- [ ] довести integration tests для реальной БД и execution history flows;
- [ ] завершить production-grade реализацию `alloy_script` и `notify` шагов;
- [ ] оценить DAG/branching expansion только при реальном product pressure, не ломая текущий linear-step contract.

### 3. Operability

- [ ] развивать системные события `workflow.execution.*` и execution observability;
- [ ] документировать новые runtime guarantees одновременно с изменением trigger/step semantics;
- [ ] удерживать local docs и `README.md` синхронизированными с live code.

## Проверка

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- targeted tests для triggers, steps, execution journal, tenant isolation и admin/runtime contracts

## Правила обновления

1. При изменении workflow runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении event/alloy integration expectations обновлять связанные docs у foundation и capability modules.
