# 2026-05-18 — Control-plane lifecycle and migration ordering contracts

## Статус

Accepted.

## Контекст

Control plane больше не должен считать `modules.toml` runtime-source-of-truth: активный состав хранится в
`platform_state`, а `modules.toml` остаётся bootstrap/dev input. Перепроверка module lifecycle показала два
остаточных риска: неатомарный путь `platform_state -> builds` и lifecycle hooks, которые выполнялись после
изменения tenant state с частичным rollback только enabled-флага. Отдельно server migrator имел dependency-aware
ordering, но зависимость `product_tags -> taxonomy_tables` была зашита в server migrator.

## Решение

- Composition update и build enqueue считаются одним control-plane действием: CAS-update `platform_state` и insert
  в `builds` выполняются в одной DB transaction и используют общий `platform_state:<revision>` manifest ref.
- Immutable manifest artifact hash — SHA-256 от canonical JSON полного manifest snapshot, а не short hash от
  подмножества module fields.
- Enable/disable lifecycle фиксирует operation со статусом `running` до state mutation. Existing `on_enable` /
  `on_disable` трактуются как compat pre-hooks: при ошибке tenant state не меняется, operation становится `failed`.
  Успешный hook позволяет atomically изменить tenant state и завершить operation как `done`.
- Прямой model-level toggle tenant module flag больше не является public lifecycle API; оставлен только явно
  названный internal migration escape hatch.
- Cross-module migration ordering объявляется рядом с module-owned migration exporter через lightweight descriptor
  metadata. Server migrator выполняет topological sort и падает на missing dependency/cycle.

## Последствия

- Admin/runtime surfaces должны идти через canonical lifecycle/build contracts или тонкий adapter, сохраняющий
  transaction boundary и manifest hash semantics.
- Hooks, которым нужен post-commit side effect, должны быть переведены на отдельный idempotent/retryable post-phase
  перед тем, как считать это hard dependency lifecycle-а.
- Новые module-owned migrations не должны добавлять server-local hardcoded dependency `match`; dependency metadata
  добавляется рядом с exporter-ом владельца модуля.
