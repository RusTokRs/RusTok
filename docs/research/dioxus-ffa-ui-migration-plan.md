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
- Базовая карта связности пилотов (`rustok-pages`, `rustok-search`) зафиксирована в `docs/research/dioxus-ffa-pilot-connectivity-map.md`.

### A3. Contract freeze

- Зафиксировать текущие GraphQL/native surfaces и smoke-скрипты.
- Добавить checklist parity: SSR native path, GraphQL fallback, headless path.
- Базовый checklist закреплён в `docs/verification/ffa-ui-parity-checklist.md` и обязателен для phase-gate evidence.

## Phase B — FFA-декомпозиция в пилотах (2–4 недели)

Для каждого пилотного UI пакета ввести 3 слоя:

1. `core.rs` или `core/` (framework-agnostic)
   - use-cases, typed state transitions, view-model mapping;
   - ошибки и policy-результаты в transport-agnostic форме;
   - `core.rs` допустим для маленького среза, `core/` обязателен при появлении нескольких поддоменов (`view_model`, `policy`, `error`, `ports`, `identifiers`).
2. `transport/`
   - `native_server_adapter` (текущий Leptos native path);
   - `graphql_adapter` (fallback/headless-compatible path);
   - если срез временно имеет только один adapter, это фиксируется как temporary single-adapter state с next-step parity plan.
3. `ui/leptos.rs` или `ui/leptos/`
   - только render/bind слой без transport/business ownership;
   - `ui/leptos.rs` допустим для одного adapter file, `ui/leptos/` используется при разрастании render adapter слоя.

Ключевое правило: UI adapter не вызывает raw GraphQL/native functions напрямую. Он может обращаться только к module-owned `transport/` facade; request/command/state construction, validation и business/policy decisions остаются в `core` ports/helpers.

## Phase C — Shared platform abstractions (1–2 недели)

Вынести повторяющиеся контракты в shared crate(s):

- `RequestMeta`, `EffectiveLocale`, `TenantScope`;
- типизированные query/filter/pagination контракты;
- единый UI error envelope.

Отдельно подготовить portability-порт для route/query plumbing:

- текущий Leptos implementation остаётся;
- добавляется transport/framework-agnostic контракт для будущего Dioxus routing adapter;
- shared foundation для первых wave вынесен в `rustok-api`: `normalize_ui_text`, `parse_ui_csv`, `UiRouteQueryUpdate`, а Leptos adapter применяет эти intents через `leptos-ui-routing`.

## Phase D — Wave rollout по остальным UI пакетам (3–6 недель)

### Wave 1 (низкая/средняя сложность)

- `pages`, `blog`, `region`, `product`.

### Wave 2 (высокая сложность)

- `search`, `cart`, `commerce`, `workflow`.

Для каждого пакета обязательный DoD:

- structural shape зафиксирован как минимум до `core_only`, а для phase-gate — до `core_transport_ui`;
- core отделён от Leptos runtime (`core.rs` и `core/` не содержат `leptos*` imports);
- native + GraphQL adapters работают и покрыты integration тестами либо temporary single-adapter state явно отмечен с next-step parity plan;
- Leptos UI слой стал thin adapter и не вызывает raw GraphQL/native functions напрямую;
- docs модуля и central docs обновлены при изменении контрактов.

## Параллельный host-track для admin/storefront

Админки и фронтенды переводятся **параллельно, но не как первый слой**:

1. Сначала module-owned UI packages выделяют `core/transport/ui` и сохраняют Leptos UI как thin adapter.
2. Одновременно host-приложения (`apps/admin`, `apps/storefront` и будущие Dioxus shells) получают только переносимые host contracts: route/query, locale, auth/session, tenant scope, mount registry и manifest wiring.
3. Host-приложения не становятся владельцами доменной UI-логики; они монтируют module surfaces через adapters.
4. Dioxus host подключается после готовности 1–2 пилотных module cores и проверяет reuse без удаления Leptos или GraphQL/headless paths.

Это означает, что изменение host wiring требует отдельной parity-проверки, но перевод доменной логики остаётся в module UI packages.

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

## Принцип исполнения backlog (одна задача за итерацию)

Чтобы не накапливать архитектурный drift и противоречивые записи, программа выполняется
строго по принципу **"одна задача -> все UI surfaces -> двойная документационная сверка"**:

1. Берём **одну конкретную задачу** (например, выделение `core` для выбранного use-case).
2. Применяем её **во всех релевантных UI пакетах/host surfaces**, где этот контракт должен быть одинаковым.
3. Обновляем документацию:
   - локальные docs модулей;
   - central docs в `docs/`;
   - при необходимости ADR/decision trail.
4. Делаем **двойную сверку документации** перед переходом к следующей задаче:
   - проход №1: проверить, что новые формулировки полностью соответствуют фактическому коду;
   - проход №2: целевой поиск и удаление/правка старых формулировок, которые вводят в заблуждение
     (устаревшие "Leptos-only" или конфликтующие transport-описания и т.п.).
5. Только после этого закрываем задачу и переходим к следующей.

Этот режим обязателен для фаз B–E, чтобы не получить частичный rollout, где код и docs расходятся
между модулями или хостами.



## Политика актуализации verification scripts

Verification scripts (`scripts/verify/*`) считаются частью живого platform contract и
обновляются вместе с изменением правил, которые они проверяют.

Обязательные правила:

1. Если migration-задача меняет transport/UI/doc contract, она **обязана** включать
   обновление соответствующих verify-скриптов в том же PR/итерации.
2. Задача не считается завершённой, если contract уже изменён, а verify-скрипты не
   отражают новые правила.
3. После каждой wave (Phase D) выполняется отдельный review verify-скриптов на предмет
   устаревших паттернов/исключений и добавления новых anti-pattern checks.
4. Перед закрытием phase-gate владелец задачи прикладывает вывод запуска актуальных
   verify-скриптов как часть evidence.

Минимальный ритм плановой ревизии: не реже 1 раза в 2–4 недели и обязательно по
завершению каждой волны rollout.

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


## Сверка с текущим кодом (на 2026-05-23)

Ниже зафиксирована привязка плана к текущему состоянию репозитория.

### 1) Фактический dual-path контракт уже закреплён в docs

- `docs/UI/graphql-architecture.md` фиксирует модель: native `#[server]` preferred + GraphQL как обязательный параллельный контракт.
- `apps/storefront/docs/README.md` фиксирует native-first в SSR/hydrate и обязательный GraphQL fallback для storefront surfaces.

### 2) UI-пакеты в коде сейчас Leptos-specific

- Базовые shared UI crates завязаны на Leptos:
  - `crates/leptos-ui/Cargo.toml`
  - `crates/leptos-ui-routing/Cargo.toml`
  - `crates/leptos-graphql/Cargo.toml`
  - `crates/leptos-auth/Cargo.toml`
- Module-owned UI пакеты активно используют `leptos::*`, `#[component]`, `leptos_router` и Leptos hooks (пример: `rustok-search`, `rustok-workflow`, `rustok-commerce`, `rustok-cart`).

### 3) Данные уже ходят через native/GraphQL гибрид

- В `crates/rustok-*/storefront/src/api.rs` и `crates/rustok-*/admin/src/api.rs` видны GraphQL adapters (`leptos_graphql`) и `#[cfg(feature = "ssr")]` ветви для native SSR paths.
- Это означает, что план не придумывает новую модель, а формализует уже существующий runtime split и переводит его в FFA-структуру.

### 4) Кандидаты пилота подтверждены текущей сложностью

- `rustok-pages`/`rustok-blog`: меньший объём UI state и проще сценарии CRUD/read.
- `rustok-search` и `rustok-commerce`/`rustok-cart`: выраженная сложность по state/fallback flows и SSR branches.

### 5) Команды сверки, которыми обновлялся этот документ

```bash
rg -n "Dioxus|Leptos|headless|server functions|UI packages|GraphQL" docs crates apps
rg -n "^use leptos|#\[component\]|#\[server\]|leptos =|leptos_router|leptos_ui_routing|cfg\(feature = "ssr"\)" crates/rustok-*/admin crates/rustok-*/storefront crates/leptos-* --glob "*.rs" --glob "Cargo.toml"
nl -ba docs/UI/graphql-architecture.md
nl -ba apps/storefront/docs/README.md
npm run verify:ffa:ui:migration
```

### 6) Следствие для исполнения плана

План выполняется **без смены продуктового контракта**: сначала рефактор структуры пакетов (core/transport/ui), затем Dioxus adapter pilot. GraphQL/REST остаются обязательными контрактами для headless parity на каждом этапе.

## Phase-gate критерии (обязательные переходы между фазами)

- **A -> B**: завершена карта связности пилотов, зафиксированы current native/GraphQL surfaces, составлен parity checklist.
- **B -> C**: в пилотах реально выделены `core/transport/ui`, UI не ходит напрямую в transport, parity тесты проходят в pilot scope.
- **C -> D**: shared abstractions согласованы с владельцами модулей, portability-порт для route/query принят как contract.
- **D -> E**: минимум одна wave завершена без doc drift, двойная documentation verification выполнена для всех затронутых модулей.
- **E -> Program done**: Dioxus pilot прошёл parity/KPI проверки и не нарушил headless контракты.

## KPI parity (измеримые пороги)

- Функциональный parity: все обязательные сценарии pilot checklist проходят и в native path, и в GraphQL fallback path.
- Error parity: доля расхождений по error-classification между адаптерами = 0 для обязательных сценариев.
- Performance guard: p95 latency новых adapter-path не ухудшается более чем на 15% относительно baseline пилота.
- Contract guard: 0 случаев удаления/ослабления headless GraphQL/REST контракта в рамках migration PR.
- Docs guard: 0 известных конфликтующих/устаревших transport-формулировок после двойной сверки.

## RACI (кто принимает phase-gates)

- **Responsible (R):** владелец конкретного модуля UI пакета + исполнитель migration task.
- **Accountable (A):** platform foundation team (финальный gate по transport/UI contract).
- **Consulted (C):** владельцы `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend` по host parity.
- **Informed (I):** смежные module owners и observability/QA владельцы.

Phase-gate не считается пройденным без явного подтверждения `A` и отметки о двойной documentation verification.

