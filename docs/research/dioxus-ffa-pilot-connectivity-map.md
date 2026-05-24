# FFA/Dioxus Pilot Connectivity Map (Phase A baseline)

Документ фиксирует исполнение шагов **A1 (выбор пилотов)** и **A2 (карта связности)**
из `docs/research/dioxus-ffa-ui-migration-plan.md`.

## A1. Выбранные пилоты

### Pilot 1 (средняя сложность): `rustok-pages`

Причины выбора:
- ограниченный storefront read/edit surface;
- меньшее число cross-module зависимостей по сравнению с commerce/search;
- хороший кандидат для первого выделения `core -> transport -> ui/leptos`.

### Pilot 2 (высокая сложность): `rustok-search`

Причины выбора:
- выраженная search-state логика (query/filter/pagination/sort);
- чувствительность к route/query parity и locale/tenant context;
- наличие fallback ветвей и runtime-ветвления для SSR/GraphQL path.

## A2. Карта связности

## `rustok-pages`

### Leptos-specific точки
- `#[component]` поверхности storefront/admin;
- router/query hooks и навигационные binding-слои;
- reactive state и signal-производные для page selection/render.

### Transport binding точки
- native `#[server]` handlers для SSR/hydrate path;
- GraphQL fallback через module-owned API adapters;
- `cfg(feature = "ssr")` ветвление для runtime split.

### Риски смешения слоёв
- прямые вызовы transport из UI-компонентов;
- смешение view-state и domain mapping в Leptos hooks;
- дублирование error mapping между native и GraphQL path.

## `rustok-search`

### Leptos-specific точки
- `#[component]` search surfaces и filters UI;
- routing/query state binding (включая URL-driven selection state);
- reactive derived state для paging/sorting/empty/error views.

### Transport binding точки
- native `#[server]` read/search path в SSR/hydrate;
- GraphQL fallback adapters для headless/CSR-compatible flow;
- runtime условные ветви для fallback/degredation режима.

### Риски смешения слоёв
- связывание transport payload формата с UI-моделью напрямую;
- неявные policy/validation checks в UI слое;
- расхождение query normalization между native и GraphQL path.

## Phase A deliverables status

- [x] A1 pilot selection зафиксирован.
- [x] A2 connectivity map зафиксирован.
- [x] A3 contract freeze evidence полностью приложен: parity checklist зафиксирован, verify-команда `npm run verify:ffa:ui:migration` добавлена в обязательный evidence path.

## Следующий шаг (one-task-per-iteration)

Следующая итерация: для `rustok-blog` выделить **один** целевой use-case и провести
структурный срез `core/transport/ui` без изменения продуктового dual-path контракта,
с обязательным evidence по checklist:
`docs/verification/ffa-ui-parity-checklist.md`.

## Связанные документы

- `docs/research/dioxus-ffa-ui-migration-plan.md`
- `docs/verification/ffa-ui-parity-checklist.md`
- `docs/UI/graphql-architecture.md`


## Статус выполнения по модулям (Phase B tracking)

- [x] `rustok-pages` — выполнен первый slice декомпозиции: в `storefront` выделен `core` слой
  для selected-page presentation logic; Leptos UI делегирует эту логику в `core`.
- [x] `rustok-search` — slices #1-#9 выполнены по storefront/admin core extraction plan.
- [x] `rustok-blog` — storefront slice #1 завершён: formatting/fallback helper-логика вынесена в `storefront/src/core.rs`, UI использует `core::*` без изменения dual-path transport контракта.

### Что уже сделано в `rustok-pages`

- добавлен `crates/rustok-pages/storefront/src/core.rs`;
- `SelectedPageCard` в `crates/rustok-pages/storefront/src/lib.rs` переведён на `core::*` функции;
- dual-path transport контракт (`native #[server]` + GraphQL fallback) не менялся.


### Перепроверка после выполненного (double-check)

- [x] Проход №1 (code/docs consistency):
  - `rustok-pages/storefront` фактически использует выделенный `core` слой для selected-page logic;
  - dual-path transport (`native #[server]` + GraphQL fallback) сохранён без удаления fallback surface.
- [x] Проход №2 (устранение устаревших формулировок):
  - в текущих central docs для этого шага не осталось формулировок, противоречащих `core`-срезу в `rustok-pages`.

### Следующий модуль (новая итерация)

- [x] Стартовали и завершили текущий набор `rustok-search` pilot slices (#1-#9).
- [x] Цель итерации достигнута: core use-cases последовательно выделены в `crates/rustok-search/storefront` и синхронизированы с `admin` surface без изменения продуктового transport-контракта.


### Scope matrix для `rustok-search` (чтобы ничего не пропустить)

- [x] `crates/rustok-search/storefront` (Leptos storefront UI package)
  - [x] выделен первый `core` use-case (query/filter input normalization: `parse_csv`, `optional_text`);
  - [x] выбранный use-case вынесен в `storefront/src/core.rs` и используется UI-слоем.
- [x] `crates/rustok-search/admin` (Leptos admin UI package)
  - [x] impact того же use-case проверен;
  - [x] тот же `core`-подход применён в `admin/src/core.rs` без расхождения контракта.
- [x] Headless parity (Next/mobile/external)
  - [x] подтверждено, что GraphQL fallback path не деградировал;
  - [x] route/query/i18n contract не получил drift относительно host expectations.

### Evidence чек перед закрытием итерации `rustok-search`

- [x] `cargo xtask module validate search`
- [x] `cargo xtask module test search`
- [x] docs double-check pass #1 (code/docs consistency)
- [x] docs double-check pass #2 (cleanup stale wording)


### Выполнено в текущей итерации (`rustok-search`, slice #1)

- добавлены `crates/rustok-search/storefront/src/core.rs` и `crates/rustok-search/admin/src/core.rs`;
- в storefront/admin UI удалены локальные дубли `parse_csv`/`optional_text` и подключены `core::*`;
- dual-path transport (`native #[server]` + GraphQL fallback) не изменялся.

- `rustok-search` slice #2: facet name normalization вынесена в core для storefront/admin (`facet_display_name`).
- `rustok-search` slice #3: facet bucket label formatting вынесен в core (`facet_bucket_label`) для storefront/admin.
- `rustok-search` slice #4: snippet fallback rendering вынесен в core (`snippet_or_fallback`) для storefront/admin.
- `rustok-search` slice #5: score label normalization вынесена в core (`score_label`) для storefront/admin.
- `rustok-search` slice #6: entity/source/status labels вынесены в core для storefront/admin.
- `rustok-search` slice #7: score template value extraction переведён на core helper (`score_value`) без string hacks в UI.
- `rustok-search` slice #8: error message composition (`<context>: <error>`) вынесена в core для storefront/admin.
- `rustok-search` slice #9: score rendering unified across storefront/admin to direct core helpers, removing template/trim coupling in UI.

- `rustok-pages` slice #2: admin form helpers (`slugify`, `parse_channel_slugs`, error composition) вынесены в `admin/src/core.rs`.


### Pages completion checklist (Phase B pilot)

- [x] `rustok-pages/storefront` core slice #1 (`selected_page_*`, `summarize_page_content`)
- [x] `rustok-pages/admin` core slice #2 (`slugify`, `parse_channel_slugs`, `error_with_context`)
- [x] `cargo xtask module validate pages`
- [x] `cargo xtask module test pages` (долгий прогон завершён, evidence приложен)
- [x] docs double-check pass #1/#2 for pages
- `rustok-pages` slice #3: status badge class mapping вынесен в `admin/src/core.rs` (`status_badge_class`).
- `rustok-pages` slice #4: admin busy-key composition вынесен в core (`busy_key_with_id`, `busy_key_for_save`).
- `rustok-pages` slice #6: admin page-list load error rendering переведён на core `error_with_context`.
- `rustok-pages` slice #7: status badge css composition moved to core (`status_badge_css`).
- `rustok-pages` slice #8: busy-key action matching moved to core (`busy_key_matches_action`).
- `rustok-pages` slice #9: raw body summary placeholder rendering moved to storefront core (`raw_body_format_summary`).
- `rustok-pages` slice #10: pages implementation tracker synchronized after double docs verification closure.
- `rustok-pages` slice #11: admin reset-form defaults delegated to core seed helper (`empty_edit_form_seed`).
- `rustok-pages` slice #12: admin table total-count label placeholder rendering moved to core (`count_label`).
- `rustok-pages` slice #13: storefront published-pages total count placeholder rendering moved to core (`count_label`).
- `rustok-pages` slice #14: admin editing-banner `{id}` placeholder rendering moved to core (`label_with_id`).
- `rustok-pages` slice #15: storefront open-link label composition moved to core (`open_link_label`).
- `rustok-pages` slice #16: storefront label/value pair rendering moved to core (`label_value_pair`).
- `rustok-pages` slice #17: storefront cleanup after full pages module-test evidence (remove unused import warning).


### Перепроверка после slices #2-#8 (rustok-pages/admin)

- [x] Проход №1 (code/docs consistency):
  - helper-логика формы, status badge и busy-key в `crates/rustok-pages/admin` вынесена в `admin/src/core.rs`;
  - storefront и admin surfaces используют `core::*` без изменения transport contract.
- [x] Проход №2 (cleanup stale wording):
  - в central docs удалены/обновлены формулировки, где эти helper-обязанности описывались как inline-логика `lib.rs`;
  - трекер pages синхронизирован с фактическим состоянием slice #2-#8.

### Какие модули поправили в этой итерации

- `rustok-pages/admin` — core helper extraction и выравнивание UI-call sites.
- `rustok-pages/storefront` — ранее завершённый core slice подтверждён повторной сверкой.
- `rustok-blog/storefront` — стартован новый slice: formatting/fallback helper-логика вынесена в `storefront/src/core.rs`.


### Pages pilot status (current checkpoint)

- [x] Planned `rustok-pages` pilot slices completed for current helper-extraction scope.
- [x] Validate + module test evidence attached in trackers.
- [x] Documentation double-check completed and synchronized.
- [x] Pilot can be treated as baseline reference sample for following module slices.


### Additional iteration evidence

- blog slice #1 evidence: `crates/rustok-blog/storefront/src/core.rs` используется `crates/rustok-blog/storefront/src/lib.rs`; transport split (`native #[server]` + GraphQL fallback) не менялся.
