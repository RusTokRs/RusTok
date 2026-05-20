# Реестр implementation plans (crate-level + library quality)

Этот реестр — единая операционная точка для сопровождения implementation plans по crate-ам, включая отдельный трек улучшения библиотек (тесты, документация, DX).
Используйте его как "single pane of glass": сначала обновляйте статус здесь, затем переходите в локальный план модуля.

## Области покрытия

Реестр обязателен не только для feature delivery, но и для library quality improvements:

- тестовое покрытие (unit/integration/property, где уместно),
- документация crate (`README.md`, `docs/`, примеры использования),
- quality gates (`cargo test`, `clippy`, `fmt`, docs checks),
- техдолг по API/контрактам и migration notes.

Для каждого crate допускаются **два параллельных плана**:

1. `feature_plan` — функциональные этапы;
2. `quality_plan` — тесты/документация/поддерживаемость.

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


## Первичный аудит покрытия (2026-05-20)

Проверка по каталогу `crates/` показала:

- Всего crate-директорий: `57`.
- Crate с `docs/implementation-plan.md`: `51`.
- Crate с `docs/quality-implementation-plan.md`: `0`.

Вывод: quality-трек пока нигде не формализован отдельным планом, его нужно bootstrap-нуть пакетно.

### Bootstrap policy для quality plans

1. Для каждого crate, где уже есть `docs/implementation-plan.md`, добавить `docs/quality-implementation-plan.md`.
2. В `quality-implementation-plan.md` сразу фиксировать:
   - baseline тестов (`existing`, `missing`, `target`),
   - baseline документации (`README`, module docs, examples),
   - baseline quality gates (`cargo test`, `clippy`, `fmt`, docs).
3. После создания файла добавить строку `quality_plan` в Global board и выставить `status=not_started`, `progress=0%`.
4. Первой итерацией каждого quality-плана делать не код, а audit + приоритизацию топ-3 хвостов.

## Global board

| Module / crate | Plan type | Plan doc | Status | Progress | Owner | Last updated (UTC) | Last checkpoint | Next action | Blockers | Verification gate |
|---|---|---|---|---|---|---|---|---|---|---|
| _example: rustok-product_ | `feature_plan` | `crates/rustok-product/docs/implementation-plan.md` | `in_progress` | `45%` | `agent:planner-1` | `2026-05-20T00:00:00Z` | Completed admin server function parity for list/read | Implement write-path SSR tests for variant pricing edits | No blocking issues | `cargo test -p rustok-product --lib` |
| _example: rustok-product_ | `quality_plan` | `crates/rustok-product/docs/quality-implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | Bootstrap baseline tests + crate README gaps audit | Need module owner confirmation for minimal test matrix | `cargo test -p rustok-product --lib && cargo clippy -p rustok-product -- -D warnings` |

> Удалите примерную строку после заполнения реальными crate-планами.

## Round-robin protocol (для агентов)

1. Выбрать верхнюю запись со статусом `in_progress` или первую `not_started` (чередуя `feature_plan` и `quality_plan`).
2. Выполнить один осмысленный инкремент.
3. Обновить checkpoint в локальном плане.
4. Обновить статус в этом реестре.
5. Если возник блокер — перевести запись в `blocked` и явно зафиксировать условие разблокировки.

## Weekly sweep

Раз в неделю отдельный агент/ответственный выполняет sweep:

- отмечает stale-элементы (`last_updated_at` старше 7 дней),
- поднимает приоритеты для `blocked` записей,
- формирует краткий список "next up" для нового круга.
