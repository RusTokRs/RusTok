# Реестр implementation plans (crate-level)

Этот реестр — единая операционная точка для сопровождения implementation plans по crate-ам.
Используйте его как "single pane of glass": сначала обновляйте статус здесь, затем переходите в локальный план модуля.

## Области покрытия

Каждый implementation plan в crate должен включать два обязательных направления в одном документе:

- feature delivery (функциональные этапы),
- quality backlog (тесты, документация, DX и quality gates).

Отдельный второй план для quality **не нужен**: качество ведётся в том же `docs/implementation-plan.md` через отдельную секцию/чеклист.

## Как работать с реестром

1. Найдите первую запись со статусом `in_progress`, `not_started` или `blocked`.
2. Откройте linked plan и выполните ограниченный по времени итерационный шаг (рекомендуется 30–60 минут или 1 PR).
3. Обновите:
   - локальный план (checkpoint-блок),
   - этот реестр (`status`, `progress`, `last_updated_at`, `last_checkpoint`, `next_action`, `blockers`).
4. Передайте следующий шаг следующему агенту через поле `next_action`.

## Статусы

- `not_started` — работа не начата.
- `in_progress` — есть активная итерация.
- `blocked` — есть внешний блокер, требуется разблокировка.
- `done` — план завершён, verification пройден, docs синхронизированы.
- `archived` — план закрыт/заменён другим документом.

## Шаблон checkpoint-блока для локальных планов

В начало каждого implementation plan добавляйте и поддерживайте блок:

```md
## Execution checkpoint

- Current phase:
- Last checkpoint:
- Next step:
- Open blockers:
- Hand-off notes for next agent:
- Last updated at (UTC):
```


## Global board

| Module / crate | Plan doc | Status | Progress | Owner | Last updated (UTC) | Last checkpoint | Next action | Blockers | Verification gate |
|---|---|---|---|---|---|---|---|---|---|
| _example: rustok-product_ | `crates/rustok-product/docs/implementation-plan.md` | `in_progress` | `45%` | `agent:planner-1` | `2026-05-20T00:00:00Z` | Completed admin server function parity for list/read + added quality backlog section with top-3 test/doc gaps | Implement write-path SSR tests and close P0 quality backlog item | No blocking issues | `cargo test -p rustok-product --lib && cargo clippy -p rustok-product -- -D warnings` |

> Удалите примерную строку после заполнения реальными crate-планами.

## Round-robin protocol (для агентов)

1. Выбрать верхнюю запись со статусом `in_progress` или первую `not_started`.
2. Выполнить один осмысленный инкремент.
3. Обновить checkpoint в локальном плане.
4. Обновить статус в этом реестре.
5. Если возник блокер — перевести запись в `blocked` и явно зафиксировать условие разблокировки.

## Weekly sweep

Раз в неделю отдельный агент/ответственный выполняет sweep:

- отмечает stale-элементы (`last_updated_at` старше 7 дней),
- поднимает приоритеты для `blocked` записей,
- формирует краткий список "next up" для нового круга.
