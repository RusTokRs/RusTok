# План реализации `rustok-index`

Статус: модуль зафиксирован как canonical index/read-model layer; локальная
документация приведена к единому формату.

## Execution checkpoint

- Current phase: phase_b_in_progress + fba_provider_runtime_smoke
- Last checkpoint: добавлен no-compile executable runtime fallback smoke для `IndexReadModelPort` / `IndexRebuildPort`: source-locked evidence packet проверяет read/list/rebuild policy gates, typed degraded-mode rebuild error, tenant-scope guard и bounded list limit без запуска Rust-компиляции.
- Next step: Подключить реальный in-process adapter к read/list/rebuild smoke и собрать Rust runtime contract evidence; до этого статус остаётся `in_progress`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и central FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - admin package split introduced `admin/src/core.rs` for Leptos-free view-model/error formatting, `admin/src/transport/` for the native server-function bootstrap facade, and `admin/src/ui/leptos.rs` as the only render adapter;
  - current admin bootstrap is an intentional temporary native-only single-adapter state because `rustok-index` had no legacy GraphQL/REST operator contract for this overview;
  - central FFA/FBA readiness board is synchronized in `docs/modules/registry.md`;
  - FBA provider slice: `crates/rustok-index/src/ports.rs` declares `IndexReadModelPort` / `index.read_model.v1` for indexed document reads and `IndexRebuildPort` / `index.rebuild.v1` for operator rebuild orchestration with shared `rustok_api::PortContext`/`PortError`, tenant-scope preservation, `PortCallPolicy::read()` deadline semantics and `PortCallPolicy::write()` idempotency/deadline semantics for rebuilds; `crates/rustok-index/contracts/index-fba-registry.json`, `crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json` and `crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json` lock planned contract cases, fallback profiles and no-compile source markers under `npm run verify:index:fba`; full Rust runtime contract execution remains pending before `boundary_ready`.

## Область работ

- удерживать `rustok-index` как infrastructure module для indexed reads и denormalized projections;
- не смешивать index/read-model слой с product-facing search responsibilities;
- синхронизировать ingestion contract, rebuild semantics и local docs.

## Текущее состояние

- базовая crate/module structure уже встроена в workspace;
- operator-facing admin overview уже опубликован через `rustok-index-admin` и разделён по FFA слоям (`core`, native-only `transport`, `ui/leptos`);
- canonical direction зафиксирован: `index` отвечает за ingestion и indexed reads, а не за ranking/UX поиска;
- модуль уже рассматривается как substrate для cross-module filtering и link-aware queries;
- event-driven consumers переведены на module-owned runtime path через `register_event_listeners(...)`, старый host/legacy listener path удалён;
- standalone `flex` ingestion теперь тоже живёт в `IndexModule`: `flex_indexer` поддерживает `index_flex_entries` как module-owned read model для `FlexEntry*` / `FlexSchema*` событий;
- boundary `index != search` теперь дополнительно удерживается contract-проверкой в `xtask`, чтобы read-model слой не начал снова экспортировать search-owned engine surfaces;
- root `README.md`, local docs и manifest metadata входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] зафиксировать роль `rustok-index` как canonical index/read-model module;
- [x] отделить boundary `index != search` на уровне локальной документации и ADR;
- [ ] удерживать sync между ingestion contracts, runtime dependencies и host integration tests.

### 2. Working index module

- [ ] довести ingestion lifecycle: bootstrap, incremental sync, rebuild, retry;
- [ ] зафиксировать canonical query surface для cross-module filtering и counts;
- [~] довести tenant/locale scoping indexed records до production-ready contract; текущий FBA smoke фиксирует tenant-scope guard и locale selector validation, но persistence-backed evidence ещё pending.

### 3. Operability

- [ ] покрыть consistency drift, rebuild duration и sync lag наблюдаемыми метриками;
- [x] добавить no-compile runtime fallback smoke для read/list/rebuild provider ports и degraded rebuild-disabled profile;
- [~] добавить operator flows для health verification и rebuild control; текущий admin overview уже показывает tenant/module/counter bootstrap через FFA native-only transport;
- [ ] документировать новые query/ingestion guarantees одновременно с изменением runtime surface.

## Проверка

- `cargo xtask module validate index`
- `cargo xtask module test index`
- targeted tests для ingestion, rebuild, filtering, consistency drift и tenant/locale scoping
- контрактные тесты покрывают все публичные use-case module-owned index/read-model contract, включая registration path для `flex_indexer`

## Правила обновления

1. При изменении index/read-model contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или dependency graph синхронизировать `rustok-module.toml`.
4. При изменении boundary между `index` и `search` синхронизировать ADR и central docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
