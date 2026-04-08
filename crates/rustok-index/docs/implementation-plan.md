# План реализации `rustok-index`

Статус: модуль зафиксирован как canonical index/read-model layer; локальная
документация приведена к единому формату.

## Область работ

- удерживать `rustok-index` как infrastructure module для indexed reads и denormalized projections;
- не смешивать index/read-model слой с product-facing search responsibilities;
- синхронизировать ingestion contract, rebuild semantics и local docs.

## Текущее состояние

- базовая crate/module structure уже встроена в workspace;
- canonical direction зафиксирован: `index` отвечает за ingestion и indexed reads, а не за ranking/UX поиска;
- модуль уже рассматривается как substrate для cross-module filtering и link-aware queries;
- root `README.md`, local docs и manifest metadata входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] зафиксировать роль `rustok-index` как canonical index/read-model module;
- [x] отделить boundary `index != search` на уровне локальной документации и ADR;
- [ ] удерживать sync между ingestion contracts, runtime dependencies и host integration tests.

### 2. Working index module

- [ ] довести ingestion lifecycle: bootstrap, incremental sync, rebuild, retry;
- [ ] зафиксировать canonical query surface для cross-module filtering и counts;
- [ ] довести tenant/locale scoping indexed records до production-ready contract.

### 3. Operability

- [ ] покрыть consistency drift, rebuild duration и sync lag наблюдаемыми метриками;
- [ ] добавить operator flows для health verification и rebuild control;
- [ ] документировать новые query/ingestion guarantees одновременно с изменением runtime surface.

## Проверка

- `cargo xtask module validate index`
- `cargo xtask module test index`
- targeted tests для ingestion, rebuild, filtering, consistency drift и tenant/locale scoping

## Правила обновления

1. При изменении index/read-model contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или dependency graph синхронизировать `rustok-module.toml`.
4. При изменении boundary между `index` и `search` синхронизировать ADR и central docs.
