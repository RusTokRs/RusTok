# План: FFA-рефактор UI пакетов и подготовка к Dioxus

## Контекст

Платформа уже фиксирует dual-path транспортный контракт для Leptos UI:

- native `#[server]` functions — preferred внутренний путь в SSR/hydrate runtime;
- GraphQL `/api/graphql` — обязательный параллельный контракт для headless hosts и fallback.

Цель этого плана — подготовить module-owned UI пакеты к FFA-паттерну
(shared core + transport adapters + host adapters), чтобы переход к Dioxus был
инкрементальным, а не вторым полным переписыванием.

## Цели

1. Сохранить текущий production-контракт (`native + GraphQL fallback`) без регресса.
2. Декомпозировать Leptos UI пакеты на framework-agnostic и framework-specific слои.
3. Подготовить инфраструктуру для Dioxus host/adapters без изменения доменной логики.
4. Удержать parity для headless клиентов (Next.js/mobile/external).

## Не-цели

- Немедленный полный перевод всех UI пакетов на Dioxus.
- Удаление GraphQL/REST контрактов.
- Изменение ownership модели (UI ownership остаётся у модулей).

## Инварианты

- GraphQL нельзя удалять из-за появления/расширения native server path.
- UI package должен продолжать работать в SSR/hydrate и standalone CSR compatibility режиме.
- Host application остаётся mount/wiring/navigation слоем, а не владельцем доменного UI.

## Фазы реализации

## Phase A — Baseline и инвентаризация (1–2 недели)

### A1. Выбор пилотов

- Пилот 1 (средняя сложность): `rustok-pages` или `rustok-blog`.
- Пилот 2 (высокая сложность): `rustok-search` или `rustok-commerce`/`rustok-cart`.

### A2. Карта связности

Для каждого пилота зафиксировать:

- Leptos-specific точки (`#[component]`, router hooks, reactive state);
- transport binding точки (`#[server]`, GraphQL requests, fallback branches);
- места смешения UI/state/business логики.

### A3. Contract freeze

- Зафиксировать текущие GraphQL/native surfaces и smoke-скрипты.
- Добавить checklist parity: SSR native path, GraphQL fallback, headless path.

## Phase B — FFA-декомпозиция в пилотах (2–4 недели)

Для каждого пилотного UI пакета ввести 3 слоя:

1. `core/` (framework-agnostic)
   - use-cases, typed state transitions, view-model mapping;
   - ошибки и policy-результаты в transport-agnostic форме.
2. `transport/`
   - `native_server_adapter` (текущий Leptos native path);
   - `graphql_adapter` (fallback/headless-compatible path).
3. `ui/leptos/`
   - только render/bind слой без transport/business ownership.

Ключевое правило: компоненты не вызывают transport напрямую; только через core ports.

## Phase C — Shared platform abstractions (1–2 недели)

Вынести повторяющиеся контракты в shared crate(s):

- `RequestMeta`, `EffectiveLocale`, `TenantScope`;
- типизированные query/filter/pagination контракты;
- единый UI error envelope.

Отдельно подготовить portability-порт для route/query plumbing:

- текущий Leptos implementation остаётся;
- добавляется transport/framework-agnostic контракт для будущего Dioxus routing adapter.

## Phase D — Wave rollout по остальным UI пакетам (3–6 недель)

### Wave 1 (низкая/средняя сложность)

- `pages`, `blog`, `region`, `product`.

### Wave 2 (высокая сложность)

- `search`, `cart`, `commerce`, `workflow`.

Для каждого пакета обязательный DoD:

- core отделён от Leptos runtime;
- native + GraphQL adapters работают и покрыты integration тестами;
- Leptos UI слой стал thin adapter;
- docs модуля и central docs обновлены при изменении контрактов.

## Phase E — Dioxus pilot (2–4 недели)

1. Поднять минимальный Dioxus host shell.
2. Подключить 1–2 пилотных module UI surface через уже выделенный core.
3. Реализовать Dioxus-specific UI adapter + native transport adapter.
4. Подтвердить parity с Leptos по сценариям и отказам.

## Верификация

Для каждого затронутого модуля/волны:

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`

При изменении host/UI wiring дополнительно:

- `npm run verify:i18n:ui`
- `npm run verify:i18n:contract`
- `npm.cmd run verify:storefront:routes`

## Документация и governance

При platform-level изменениях:

1. обновить локальные docs затронутых модулей;
2. обновить central docs в `docs/`;
3. поддержать актуальность `docs/index.md`;
4. оформить ADR в `DECISIONS/`, если меняется platform transport/UI contract.

## Риски и mitigation

1. **Риск:** core слой останется связанным с Leptos типами.
   - **Mitigation:** CI-check, запрещающий `leptos*` зависимости в `core` crates.

2. **Риск:** fallback path перестанет реально проверяться.
   - **Mitigation:** обязательные parity integration suites для native и GraphQL adapters.

3. **Риск:** divergence поведения Leptos и Dioxus.
   - **Mitigation:** contract tests на уровне shared use-cases + snapshot тесты ключевых state transitions.

## Критерии готовности программы

Программа считается завершённой, когда:

- минимум 2 сложных модуля прошли FFA-декомпозицию и parity verification;
- Dioxus pilot подтверждает reuse shared core без дублирования доменной логики;
- headless контракты не деградировали;
- документация и ADR отражают новое целевое состояние.
