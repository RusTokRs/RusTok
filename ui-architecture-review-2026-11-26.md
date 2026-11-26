# RusToK: обзор UI-слоя, таблиц, дизайн-системы и стилей

Дата: 2026-11-26
Ветка: `arena/a3d16606-rustok` (от `29f184e`)
Режим: только аудит, код не менялся (кроме этой записки)

> **Ограничение окружения.** В песочнице нет `cargo`/`rustc`, поэтому «собирается / тесты зелёные» по Rust здесь не проверялось и не утверждается. Всё, что ниже, получено чтением исходников, подсчётами по коду и реальными прогонами `node`-гейтов, а CSS собран настоящим Tailwind CLI (`npx tailwindcss`) из `apps/admin`, то есть тем же способом, что и в штатной сборке.

---

## 0. Резюме

1. **Архитектура UI сделана хорошо и она по-настоящему многослойна**: framework-agnostic ядро (`rustok-ui`) → Leptos-адаптер (`rustok-ui-leptos`) → headless-грид (`rustok-grid`) → Leptos-адаптер грида (`DataGrid`) → модульные админки с контрактом `core`/`transport`/`ui`. Все 46 модулей имеют `core` + `transport`, 44 имеют `ui`; регистрация страниц в `apps/admin` вообще кодогенерируется из `rustok-module.toml` при сборке.
2. **Но дизайн-система почти не используется.** Leptos-адаптер ДС импортируют **9 из 46** модулей. Сырых `<button>`/`<input>`/`<select>` в модульных админках — **1173** против **252** компонентов ДС (4.6 : 1). `rustok-channel` — один из 37 модулей без ДС вообще.
3. **Таблицы — самый зрелый кусок.** `DataGrid` умеет pagination (client/server), filters (5 типов), selection, resize, column visibility, infinite scroll, i18n-каталог (`en/ru/fr`) и a11y (`aria-label`, `aria-sort`). 25 модулей реально рисуют таблицы им.
4. **Функционал таблиц на практике используется на треть, и один пункт сломан системно:** сортировка. 30 мест вызова передают `filters` (21), `selection` (21), `pagination` (29), но `sort_state` — **0 из 30**. При этом `sortable` по умолчанию `true`, то есть во всех таблицах заголовки выглядят кликабельными, показывают стрелку и сбрасывают страницу, **но порядок строк не меняется** (грид не сортирует данные сам — `display_items` только режет страницу).
5. **Найдены два дефекта стилей уровня «видно в проде, не видно в dev»:**
   - в `apps/admin/input.css` в `@source` **нет** `crates/ui/rustok-grid/leptos/src/**/*.rs` → из собранного CSS выпадают 13 утилит грида (в т.ч. `border-collapse`, `align-middle`, `max-h-56`, фокус-ринги). Воспроизведено сборкой: 114 258 Б → 116 515 Б после добавления одной строки;
   - `DataGrid` задаёт ширины колонок/ячеек только через атрибут `style=`, а CSP приложения — `style-src-attr 'none'`. То есть в проде ширины и resize **не применяются**. Гейт, который это должен ловить (`verify-csp-inline-style-exceptions`), падает с `ENOENT` ещё до проверок (известный baseline-провал).
6. **Токены разъехались**: `UI/tokens/base.css` (`--iu-*`, 50 строк) не имеет **ни одного** потребителя, его тёмная тема висит на `[data-theme="dark"]`, тогда как приложение ставит класс `.dark`; радиусы в `tokens.json` (6/10/14 px) не совпадают с тем, что реально применяет Tailwind (`--radius: 0.5rem` → 6/8 px).

Сводная оценка по срезам:

| Срез | Состояние | Комментарий |
| :--- | :--- | :--- |
| Слоистость / FFA-границы | ✅ хорошо | ядро без фреймворка, транспорт через facade, гейты на границы модулей |
| `DataGrid` как библиотека | ✅ хорошо | продуманный API, SSR-корректный `total`, i18n-каталог, a11y |
| Использование `DataGrid` | ⚠️ средне | функции есть, но 2/3 не подключены; сортировка не работает нигде |
| Лептос-адаптер ДС | ✅ хорошо | 22 компонента, a11y из коробки (`aria-busy`/`aria-invalid`/`aria-describedby`/`data-slot`) |
| Внедрение ДС в модули | ❌ плохо | 9/46 модулей; 1173 сырых контрола; гейта на это нет |
| Tailwind-пайплайн | ⚠️ с дефектом | v4 + `@config` + `@source`; пропущен один каталог → дыры в CSS |
| Токены / темы | ⚠️ с дефектом | два параллельных набора переменных, мёртвый `--iu-*`, дрейф радиусов |
| CSP vs UI | ❌ конфликт | `style-src-attr 'none'` против инлайн-`style` в гриде; профильный гейт мёртв |
| Витрина `/design-system` | ⚠️ неполная | нет `Select`, `Dialog`, `Tabs`, `Avatar`, `Skeleton`, `Spinner`, `Progress`, **`DataGrid`** |
| Тесты UI | ⚠️ частично | Playwright только у `next-*`; у Leptos-админки e2e нет |

---

## 1. Карта UI-слоёв

### 1.1 Крейты и их вес

| Крейт | Файлы / строк | Роль | Зависимости |
| :--- | :--- | :--- | :--- |
| `crates/ui/rustok-ui` | 2020 строк | framework-agnostic ДС: `classes.rs` (резолверы классов), `tokens.rs`, `tokens_generated.rs`, `types.rs` (варианты/размеры), `contracts.rs` | только `serde` |
| `crates/ui/rustok-ui/leptos` (`rustok-ui-leptos`) | 24 файла / 3355 строк | Leptos 0.8 SSR-first компоненты: alert, avatar, badge, button, card, checkbox, dialog, input, label, language_toggle, progress, richtext, select, separator, skeleton, spinner, success_message, switch, tabs, textarea, toc | `leptos`, `rustok-ui` |
| `crates/ui/rustok-grid` | 13 файлов / 3900 строк | headless-грид: cell, column, facet, facet_panel, filter, messages, pagination, resize, selection, sort, state (+ `contract_tests.rs`) | `serde`, `rustok-ui-i18n` |
| `crates/ui/rustok-grid/leptos` | 1640 строк | `DataGrid` + grid/header/row/toolbar/pagination/filter_inputs/resize_handle | `leptos`, `web-sys` (pointer events) |
| `crates/ui/rustok-ui-core` | — | framework-agnostic route context (`UiRouteContext`), query-параметры, хлебные крошки | — |
| `crates/ui/rustok-ui-i18n` | — | каталог сообщений, нормализация локали, Fluent | `fluent` |
| `crates/ui/rustok-ui-forms` (+`leptos`) | — | schema-driven формы и валидация | — |
| `crates/ui/rustok-ui-transport` | — | выбор профиля транспорта (`execute_selected_transport`) | — |
| `crates/ui/fly` (+`leptos`) | — | движок рендера и редактор страниц (page builder) | — |
| `packages/rustok-ui-grid` (TS) | 13 файлов / 2948 строк | зеркало headless-ядра для Next-админки (`cell/column/facet/filter/pagination/resize/selection/sort/state/url/order`) | TS, без React |
| `UI/tokens` | `base.css` 50 строк + `tokens.json` + `rustok_tokens.g.dart` | «канонические» токены и кодоген для Flutter | — |

Технологический стек (по `Cargo.toml` / `package.json`): Leptos **=0.8.21**, `leptos_axum` 0.8.9, `leptos_router` 0.8.13, `leptos-use` 0.19.0; Tailwind v4 (`tailwindcss ^4.2.1`, `@tailwindcss/cli ^4.3.5` — фактически ставится 4.3.5); Next 16.4.6 / React 19.3.1 / TS 7.0.2 в `apps/next-admin`.

### 1.2 Как страница модуля попадает в админку

`apps/admin/build.rs` на этапе сборки читает `crates/modules/*/rustok-module.toml` и генерирует `$OUT_DIR/module_registry_codegen.rs`, который вызывает `register_page(AdminPageRegistration { module_slug, route_segment, title, child_pages, render: render_<slug>_admin_page })` (`build.rs:392`). `ChannelAdmin` нигде не импортируется руками — только через `pub use ui::leptos::ChannelAdmin;` в крейте модуля (проверено: в `apps/admin/src` прямых упоминаний `ChannelAdmin` нет, маршрут один — `/modules/:module_slug` → `ModuleAdminPage` → реестр).

Плюсы: нет ручного списка маршрутов, навигация и порядок групп тоже генерируются.
Минус: `child_pages` попадают в навигацию, но их наличие в UI никем не контролируется (см. §7, P2-4).

### 1.3 Три параллельных фронтенда

- `apps/admin` — Leptos SSR/hydrate, основная админка (Trunk + `build.rs`-хук на Tailwind).
- `apps/next-admin` — Next.js референсный headless-клиент со своим зеркалом грида и своей витриной `/dashboard/design-system`.
- `UI/tokens` → `rustok_tokens.g.dart` — Dart-токены для мобильного (`rustok_mobile`).

Это осознанная стратегия (FFA), но она означает: **каждое правило UI нужно проверять трижды**, а `verify-ui-grid-parity` проверяет только наличие символов Rust↔TS (в TS при этом есть лишние `url.ts`/`order.ts`, которых нет в Rust; `facet_panel.rs` ↔ `panel.ts` — переименование), т.е. гейт односторонний по смыслу.

---

## 2. Как сделаны таблицы (`rustok-grid` + `DataGrid`)

### 2.1 Разделение ответственности

`rustok-grid` (без Leptos) держит **всё, что можно протестировать `assert_eq!`**: `GridColumnDef`, `ColumnFilters` + 5 типов фильтров, `SortState`, `RowSelection`, `GridPagination` (+ `slice_bounds`, `offset`), `ColumnWidths` (+ clamp), `facet`/`facet_panel`, `GridMessages` с каталогами `locales/{en,ru,fr}.ftl`. `DataGrid` — только рендер и связывание сигналов.

Это ровно то, что требует AGENTS («если логику можно проверить на чистых данных — она живёт в `core/`»), и это редкий случай, когда правило соблюдено не на словах.

### 2.2 Публичный API `DataGrid` (`leptos/src/grid.rs:26–88`)

| Группа | Пропсы |
| :--- | :--- |
| Данные | `columns: Vec<GridColumnDef>`, `data: Signal<Vec<T>>`, `key_fn`, `cell_renderer: Callback<(T, String), AnyView>` |
| Состояние | `is_loading`, `empty_message`, `on_row_click` |
| Пагинация | `pagination: Option<RwSignal<GridPagination>>`, `on_page_change`, `on_page_size_change`, `page_size_options`, `on_load_more` |
| Фильтры | `filters`, `on_filter_change` |
| Сортировка | `sort_state`, `on_sort_change` |
| Выбор строк | `selection`, `on_selection_change`, `bulk_actions` |
| Колонки | `column_widths`, `on_column_widths_change` |
| Доступность/i18n | `aria_label`, `messages: GridMessages` |

Колонка (`rustok-grid/src/column.rs:244`): `id, title, align, width{min,max}, sortable, resizable, filterable, visible, pinned, filter_type`. Все «флаги» по умолчанию `true` (`GridColumnDef::new`), плюс фабрика `checkbox()` для колонки выбора.

Типы фильтров (`filter.rs:28`): `Text`, `Select`, `NumberRange`, `DateRange`, `Boolean` — то есть по функциональности это полноценный «data grid», а не просто таблица.

### 2.3 Поведенческие решения, которые сделаны правильно

- **Uncontrolled по умолчанию, controlled по требованию**: любой сигнал можно не передавать — грид создаст локальный.
- **Режим пагинации выводится из наличия `on_page_change`** (`client_side = on_page_change.is_none()`), и в клиентском режиме `total` **деривируется**, а не проставляется эффектом — первый SSR-кадр уже корректный; `Effect` только досинхронизирует сигнал вызывающего.
- **keyed-рендер строк** по `key_fn`: строки не пересоздаются на каждом обновлении данных.
- **`provide_context(messages)`**: вложенные части (toolbar, footer, header) берут подписи из одного каталога.
- **a11y**: `<table aria-label>` (`grid.rs:285`), `scope="col"`, `aria-sort` на каждой сортируемой колонке (`header.rs:84`), `aria-label` у «выбрать все»/«выбрать строку», индетеринейт на чекбоксе шапки.
- **Сброс страницы на 1** при смене фильтра и сортировки; «выбрать все» ограничено видимой страницей (как и обещает чекбокс).
- **Resize** с clamp по `min/max` колонки (`handle_resize`).

### 2.4 Чего в гриде нет

| Возможность | Статус |
| :--- | :--- |
| Клиентская сортировка | **нет** — `display_items` только режет страницу (`grid.rs:108–116`); сортировка целиком на вызывающем |
| Клиентская фильтрация | **нет** — та же причина; фильтр только меняет состояние |
| Виртуализация строк | нет |
| Sticky header / sticky колонки | нет |
| Мульти-сортировка (`?sort=a,-b`) | нет: `SortState { column_id, direction }` — одна колонка |
| `pinned` (закрепление колонки) | **не реализовано**: поле влияет только на то, что колонку нельзя спрятать в «Столбцы» (`toolbar.rs:22–27`), sticky-рендера нет |
| Персистенция ширин/страницы | нет (можно сделать снаружи: `ColumnWidths` сериализуем, но `localStorage` грид не трогает) |
| Экспорт CSV / печать / плотность строк | нет |
| Клавиатурная навигация по ячейкам | нет |

Для админки такие умолчания приемлемы, но `pinned` — это «мёртвое поле в публичном контракте», и `verify-ui-grid-parity` его не ловит, потому что проверяет наличие символов, а не поведение.

---

## 3. Как таблицы реально используются

Срез: 30 файлов с `<DataGrid>` в `crates/modules/*/admin` (25 модулей), 0 в `apps/admin/src`.

| Возможность | Передают | Комментарий |
| :--- | ---: | :--- |
| `empty_message` | 30 / 30 | каждый модуль передаёт свою локализованную пустую подпись |
| `pagination` | 29 / 30 | |
| `on_row_click` | 26 / 30 | открытие карточки |
| `filters` | 21 / 30 | фильтрацию данных модуль делает сам (например, `rustok-auth/admin/src/ui/users.rs`: `data=Signal::derive(move \|\| filtered_users.clone())`) |
| `on_filter_change` | 20 / 30 | |
| `selection` | 21 / 30 | панель массовых действий модули рисуют сами снаружи |
| `is_loading` | 14 / 30 | |
| **`messages`** | **1 / 30** | остальные 29 показывают английскую обвязку |
| **`aria_label`** | **2 / 30** | имя таблицы для скринридера — из `en`-каталога |
| **`sort_state`** | **0 / 30** | сортировка не подключена |
| `on_sort_change` | 1 / 30 | |
| `on_page_change` | 2 / 30 | 28 таблиц грузят весь массив в браузер и режут сами |
| `bulk_actions` | 0 / 30 | мёртвый проп |
| `column_widths` | 0 / 30 | resize работает, но не запоминается |
| `page_size_options` | 0 / 30 | фиксированный список из ядра |
| `on_load_more` | 1 / 30 | |

### 3.1 Сортировка: интерфейс есть, эффекта нет

`sortable` по умолчанию `true`; в модулях явных `sortable: false` — 16, `sortable: true` — 36, `GridColumnDef::new` — 213 (т.е. абсолютное большинство колонок сортируемые). Клик по заголовку вызывает `handle_sort` (`grid.rs:170`): переключает локальный `SortState`, сбрасывает страницу на 1, зовёт `on_sort_change`, если он есть. Данные при этом не переупорядочиваются.

Итог: **во всех 30 таблицах заголовки выглядят интерактивными (стрелка + `aria-sort`), но клик ничего не сортирует.** Это либо нужно чинить централизованно (клиентская сортировка в гриде — по аналогии с клиентской пагинацией, которая уже есть), либо сделать `sortable` по умолчанию `false` и требовать `on_sort_change` для включения — тогда интерфейс не будет врать.

### 3.2 Локализация обвязки грида

Каталог грида полный и с корректной русской плюрализацией:

```
grid-selected = { $count ->
    [one] Выбрана { $count } строка
    [few] Выбрано { $count } строки
   *[many] Выбрано { $count } строк
    }
```

Но `messages` передан в **одном** месте из 30, поэтому `grid.name` («Таблица данных»), `grid.loading`, `grid.empty`, `grid.range`, `grid.rowsPerPage`, `grid.previous/next` у 29 таблиц берутся из `en`. При этом локаль в этих компонентах **уже есть** (`ui_locale.as_deref()`), т.е. фикс механический: `messages=GridMessages::for_locale(ui_locale.as_deref())`.

Показательный симптом: `rustok-auth/admin/src/ui/users.rs:381` вместо `GridMessages::selected(count)` (где есть ru-плюрализация) собирает строку руками:

```rust
format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "пользователей" } else { "users" })
```

— то есть «3 выбрано пользователей» вместо «Выбрано 3 пользователя».

---

## 4. Дизайн-система: что есть, и как её обходят

### 4.1 Что даёт `rustok-ui` + `rustok-ui-leptos`

Резолверы классов (`classes.rs`) собирают строку из констант: размер, вариант, `FOCUS_RING_CLASSES`, `DISABLED_CONTROL_CLASSES`, `shadow::XS`, `radius::MD`. Варианты (`types.rs`): `ButtonVariant` (Default/Destructive/Outline/Secondary/Ghost/Link), `BadgeVariant` (Default/Secondary/Destructive/Outline/**Success/Warning/Info**), `AlertVariant` (Default/**Info/Warning/Destructive/Success**), `CardVariant`, `Size` (Xs…Xl/Icon), `AvatarSize`, `SwitchSize`, `SkeletonVariant`, `Orientation` — всё с `dark:` в вариантах.

Компоненты дополнительно дают из коробки: `data-slot`, `aria-busy` при `loading`, `aria-invalid`, `aria-describedby`, `aria-label`, подавление клика при `disabled`/`loading`, `type` по умолчанию `button`.

### 4.2 Насколько ДС внедрена

| Метрика | Значение |
| :--- | ---: |
| Модулей с админ-UI | 46 |
| Импортируют `rustok_ui_leptos` | **9** (auth, billing, blog, cache, comments, forum, messenger, product, translation) |
| Используют `<DataGrid>` | 25 |
| Без `DataGrid` | 21 |
| Сырых `<button>`/`<input>`/`<select>` в модульных админках | **1173** |
| Компонентов ДС (`<Button>`/`<Input>`/`<Select>`) в модульных админках | 252 |
| Модулей с нулём сырых контролов | 3 (notifications, outbox, tenant) |
| Модулей «сырые есть, ДС нет вообще» | 38 |
| `apps/admin/src`: сырые / ДС | 106 / 71 |

Худшие по сырым контролам: `rustok-page-builder` (65 button / 127 input / 28 select, 0 ДС), `rustok-product` (88/81/30 при 27/2/7 ДС), `rustok-forum` (39/24/10, 0 ДС), `rustok-groups` (26/29/4), `rustok-pages` (37/18/2), `rustok-channel` (31/15/6).

Правило, которое при этом нарушается, записано в `AGENTS.md` (§ «Frontend and UI architecture»): «**MUST NOT** write raw `<button>` or `<input>` with custom styling, hand-roll modal dialogs, or build ad-hoc alerts», и там же про иконки: «**MUST NOT** … paste raw inline `<svg>` blocks».

### 4.3 Чем плохи ручные контролы — конкретно

Ручной input в `rustok-channel`:

```
w-full rounded-lg border border-input bg-background px-3 py-2 text-sm
```

Канонический `input_classes(Size::Md, false, None)`:

```
flex w-full border bg-background text-foreground file:border-0 file:bg-transparent
file:text-sm file:font-medium placeholder:text-muted-foreground rounded-md shadow-xs
transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring
disabled:cursor-not-allowed disabled:opacity-50 h-9 text-sm px-3 py-1 border-input
```

Разница не косметическая: у ручного **нет фокус-ринга** (клавиатурная навигация не видна), **нет `placeholder:text-muted-foreground`**, **нет `disabled:`-состояния**, **нет `text-foreground`** (в тёмной теме цвет текста зависит от наследования). То же с кнопками: каноническая несёт `focus-visible:ring`, `disabled:opacity-50`, `shadow-xs` и единую высоту по `Size`, ручные — набор из 4 разных высот (`h-10`, `py-1 text-xs`, `py-2`, `px-4`).

Дублирование классов (только `rustok-channel/admin`): 163 атрибута `class`, **78 уникальных строк**, топ повторов — input-стиль 15×, вторичная кнопка 7×, карточка секции 5×, «пустое состояние» 4×. Это ровно те пять компонентов (`Input`, `Button`, `Card`, `Badge`, `Alert`), которые уже есть.

### 4.4 Иконки — дырка в контракте

AGENTS требует «canonical Lucide-based icon set exposed via `rustok-ui`». В реальности: ни `pub fn Icon`, ни упоминаний `lucide` в `crates/ui` не существует, а инлайновых `<svg>` — **39** в 5 местах (`crates/ui` 16, `apps/admin/src` 10, `page-builder/admin` 7, `auth/admin` 5, `product-bundles/admin` 1). Т.е. правило «не вставлять `<svg>` руками» нарушается самим же платформенным кодом, потому что канонического способа нет.

### 4.5 Витрина `/design-system`

`apps/admin/src/pages/design_system.rs` (751 строка, маршрут `/design-system`) — интерактивный «UI Workbench»: `Button` (31 использование), `Badge` (13), `Card` (11), `Alert` (2), `Input` (2), `Switch`, `Checkbox`, `Textarea`, `Separator`, `RichTextEditorFrame`.
Отсутствуют в витрине: `Select`, `Dialog`, `Tabs`, `Avatar`, `Skeleton`, `Spinner`, `Progress`, `Toc`, `LanguageToggle` и **`DataGrid`**. Именно поэтому дефект с `@source` и ширинами колонок никто не увидел: грид не имеет ни витрины, ни e2e, а Playwright есть только у `next-admin`/`next-frontend`.

---

## 5. Стили: Tailwind v4, токены, темы

### 5.1 Пайплайн

`apps/admin/input.css`:

```css
@import "tailwindcss";
@config "./tailwind.config.js";
@source "./index.html";
@source "./src/**/*.rs";
@source "../../crates/**/admin/src/**/*.rs";
@source "../../crates/ui/rustok-ui/src/**/*.rs";
@source "../../crates/ui/rustok-ui/leptos/src/**/*.rs";

@import "../../UI/tokens/base.css";
```

Сборка: `apps/admin/scripts/tailwind-build.mjs` (хук Trunk) вызывает `node_modules/.bin/tailwindcss -i input.css -o <staging>/output.css --minify` с `cwd = apps/admin`. Автодетект контента в Tailwind v4 работает от корня проекта, `@source` — добавка; по факту набор сканируемых каталогов определяется этим списком (`crates/**/admin/src`, `rustok-ui/src`, `rustok-ui/leptos/src`).

Дизайн-токены shadcn-стиля: HSL-переменные в `:root` и `.dark` (`--background/--foreground/--card/--primary/--secondary/--muted/--accent/--destructive/--border/--input/--ring/--radius` + `--sidebar*`), в `tailwind.config.js`: `darkMode: ["class"]`, `borderRadius: { lg: var(--radius), md: calc(var(--radius) - 2px), sm: calc(var(--radius) - 4px) }`.

Тёмная тема реально работает: `apps/admin/index.html` до отрисовки ставит `document.documentElement.classList.toggle("dark", theme === "dark")`, в `apps/admin/src/shared/ui/mod.rs` — `Theme` + анимация иконки солнце/луна.

### 5.2 Дефект P1: в `@source` нет `rustok-grid/leptos`, часть CSS не генерируется

Проверено практикой (Tailwind CLI 4.3.5, `cwd = apps/admin`, тот же вызов, что в `tailwind-build.mjs`):

1. `npx tailwindcss -i input.css -o /tmp/output.css --minify` → **114 258 Б**.
2. Из `class="…"` в `crates/ui/rustok-grid/leptos/src/**` извлечено 122 токена, 18 из них встречаются **только** там. В собранном CSS **14 из 18 отсутствуют**.
3. Копия `input.css` с добавленной строкой `@source "../../crates/ui/rustok-grid/leptos/src/**/*.rs";` → **116 515 Б**, и появляются все 13 «настоящих» утилит (14-я, `group/th`, — маркерный класс, CSS не генерирует).

Что именно выпадало:

| Утилита | Где нужна | Что ломается без неё |
| :--- | :--- | :--- |
| `border-collapse` | `<table>` | двойные рамки таблицы |
| `align-middle` | `<th>`/`<td>` | вертикальное выравнивание ячеек |
| `text-foreground/80` | заголовки колонок | приглушённый цвет заголовка |
| `bg-border/60`, `h-3` | визуальная линия ресайз-хэндла | ресайз-хэндл не виден |
| `focus-visible:ring-primary/50`, `focus:ring-primary/30` | заголовок-кнопка сортировки, чекбокс шапки | **нет фокуса у элементов управления грида** |
| `max-h-56` | выпадающий список «Столбцы» (`toolbar.rs:109`) | список не ограничен по высоте |
| `-mt-2`, `h-1` | зона захвата в пагинации (`pagination.rs:175`) | зона захвата смещена |
| `h-3.5`, `w-3.5` | чекбоксы в «Столбцы» (`toolbar.rs:123`) | чекбоксы без размера |
| `w-1/2` | парные поля фильтров `NumberRange`/`DateRange` (`filter_inputs.rs:246…`) | два поля фильтра встают в полную ширину друг под другом |
| `last:border-r-0`, `hover:bg-muted/60` | ячейки | лишняя правая рамка у последней колонки, нет подсветки строки |

Т.е. таблицы во всех 25 модулях в собранном CSS выглядят не так, как задумано, причём пострадала доступность (фокус-ринги).

Фикс — одна строка в `apps/admin/input.css`. Гейта на это нет: `verify-csp-next-style-boundary.mjs:295` пинит в `input.css` только `:root {`, `color-scheme: light;`, `.dark {`, `color-scheme: dark;`, а grep `@source` по `scripts/verify` не находит ни одного скрипта.

### 5.3 Дефект P1: CSP `style-src-attr 'none'` против инлайн-`style` в гриде

`apps/server/src/middleware/security_headers.rs:28`:

```rust
const UI_CSP_TEMPLATE: &str = "… script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; …";
```

`select_csp` (там же, строка 226) применяет этот шаблон ко **всем** не-API и не-richtext поверхностям, а nonce генерируется всегда (`:62–63`), т.е. SSR-документ админки уходит в браузер именно с `style-src-attr 'none'`.

`DataGrid` же задаёт геометрию только атрибутом `style`:

- `leptos/src/header.rs:76` — `style = format!("width: {}px; min-width: …; max-width: …")` на каждом `<th>`;
- `leptos/src/row.rs:80` — то же на каждой `<td>`.

Следствие: в проде **ширины колонок и resize не применяются** (`style=`-атрибут блокируется), при этом таблица остаётся рабочей за счёт auto-layout — поэтому баг не виден. В dev (`trunk serve`, без CSP-заголовков) всё «работает».

Гейт, который обязан это ловить, **мёртв**: `node scripts/verify/verify-csp-inline-style-exceptions.mjs` падает с `ENOENT: crates/modules/rustok-pages/admin/src/starters.rs` до всяких проверок; скрипт числится в известных baseline-провалах (333 провала на базе, 331 на ветке — `verify-csp-inline-style-exceptions` в обоих списках). Реестр `docs/security/csp-inline-style-attribute-exceptions.json` содержит **0 исключений** и ratchet «ноль инлайн-стилей».

Инвентарь инлайн-стилей (весь репозиторий — 4 места, все в `crates/ui`): `rustok-grid/leptos/src/header.rs:76`, `rustok-grid/leptos/src/row.rs:80`, `rustok-ui/leptos/src/switch.rs:129` (honeypot-чекбокс, дополнительно к `sr-only` — косметика), `fly/leptos/src/editor.rs:658` (`canvas_style` — здесь конфликт архитектурный: редактор брендового контента по определению хочет инлайн-стили).

### 5.4 Токены: два набора, один мёртвый, радиусы разъезжаются

| Источник | Что объявляет | Кто потребляет |
| :--- | :--- | :--- |
| `UI/tokens/base.css` | `--iu-radius-{sm,md,lg}` = 6/10/14px, `--iu-space-*`, `--iu-text-*`, `--iu-shadow-*`, `--iu-bg/fg/muted/…` + тёмная тема `[data-theme="dark"]` | **никто**: `grep 'var(--iu-'` по репозиторию не даёт ни одного использования |
| `apps/admin/input.css` | shadcn `--background…--ring`, `--radius: 0.5rem`, тёмная тема `.dark` | весь admin-UI |
| `apps/admin/tailwind.config.js` | `borderRadius.md = calc(var(--radius) - 2px)` = 6px, `lg = var(--radius)` = 8px, `sm` = 4px | резолверы ДС (`radius::MD` → `rounded-md`) |
| `apps/storefront/assets/input.css` | свои `:root` с `--radius: 0.875rem` (14px) | storefront |
| `UI/tokens/tokens.json` / `rustok_tokens.g.dart` | radius 6/10/14, свои цвета | Flutter + `verify-ui-parity` |

Два независимых расхождения:

1. **Мёртвый слой**: `base.css` импортируется в `input.css`, но его переменные не используются, а его тёмная тема привязана к `[data-theme="dark"]`, хотя приложение ставит **класс** `.dark`. Даже если завтра кто-то начнёт писать `var(--iu-bg)`, тёмная тема не включится.
2. **Радиусы**: `rounded-md` в админке = 6px (совпало с `tokens.json` случайно), `rounded-lg` = 8px против 10px в `tokens.json`, `sm` = 4px против 6px; во Flutter при этом 6/10/14. Т.е. «единый контракт токенов» на вебе не соблюдается, а `verify-ui-parity` проверяет лишь наличие полей/маркеров (`--iu-radius-sm|md|lg` в `base.css`) и дрейф не видит.

Отдельно: `apps/admin` объявляет `tailwindcss ^4.2.1` и `@tailwindcss/cli ^4.3.5` — caret скрывает рассинхрон, фактически ставится 4.3.5 (проверено `npx tailwindcss --help`), но пины стоит выровнять.

### 5.5 Тёмная тема и палитровые классы

Так как тема переключается классом `.dark`, любой цвет без `dark:`-варианта — дефект. В `rustok-channel/admin` 42 палитровых класса (`rose` 15, `emerald` 9, `amber` 9, `sky` 9) и **0** `dark:`; в остальных модулях картина лучше (в `apps/admin/src` доля `dark:` нормальная), но системного гейта на это нет.

---

## 6. Кейс `rustok-channel` (то, что смотрел в первую очередь)

### 6.1 Что сделано хорошо

- FFA-контракт соблюдён: `core.rs` (608 строк) — чистые функции/команды/лейблы, `transport/` — facade без fallback, `ui/leptos/` — рендер; `verify-channel-admin-boundary` — 16/16 green.
- Функционал полный: rename, activate/deactivate, make default, delete с подтверждением, targets CRUD, module bindings CRUD (с `is_enabled`), OAuth app bindings CRUD, policy sets create/activate, rules CRUD + reorder (вверх/вниз), выбор канала через query-параметры (`AdminQueryKey::ChannelId/TargetId`) — т.е. состояние в URL, а не в локальном `Signal` (это правильнее, чем у большинства модулей).
- Нет инлайн-`style`, нет `<svg>`, нет hex-цветов, семантика токенов используется (`border-border` 46, `text-muted-foreground` 40, `bg-background` 22, `text-card-foreground` 21).

### 6.2 Что не так с UI

| Проблема | Факт |
| :--- | :--- |
| Сырые контролы | 31 `<button>` (24 `type="button"`, 7 `type="submit"`), 15 `<input>`, 6 `<select>`, 0 компонентов ДС |
| Инлайн-классы | 163 атрибута `class`, 78 уникальных строк |
| Палитра без тёмной темы | 42 класса `rose/emerald/amber/sky`, **0** `dark:` |
| Доступность | **0** `aria-*`, **0** `role=`, 4 `<label>`, **0** `focus-visible`, 22 `disabled:` |
| Wire-токены в UI | `<option value="web_domain">"web_domain"</option>` ×5 типов; `<option value="http">"http"</option>` |
| Неиспользованный перевод | рендер цели: `format!("{}{}", target.target_type, if target.is_primary { " · primary" } else { "" })` (`channel_card.rs:906`) — **английское** «· primary» при наличии ключа `channel.targets.primarySummary` и готового русского перевода в `admin/locales/ru.ftl:132` («{ $type } · основной») |
| Мёртвый код | `_targets_primary_summary_template` (`channel_card.rs:54`) — тот самый шаблон, который должен был использоваться в строке 906 |
| Нет поиска/фильтра/пагинации по каналам | список каналов рендерится целиком карточками; у тенанта с десятками каналов найти нужный нечем |
| Секции не разделены | targets / modules / oauth / policies — всё на одной длинной странице, без табов/аккордеонов |
| Вторичная навигация ведёт в никуда | в `rustok-module.toml` объявлены `child_pages: targets, apps`, навигация ссылки рисует, но `ChannelAdmin` подпуть не читает (`route_context.route_segment` используется только для подсказки URL) |

Показательно, что статус-бейджи и баннеры в этом модуле рисуются руками, хотя ДС уже содержит ровно те же цвета с `dark:`: `BadgeVariant::{Success,Warning,Info}` и `AlertVariant::{Info,Warning,Destructive,Success}`.

Сравнение с «эталонным» модулем `rustok-auth/admin`: `ui/users.rs` использует `DataGrid` + `selection` + панель выбранных, колонки описаны в `core.rs` (`user_grid_columns()`, `GridFilterType::Text/Select`, локализованные заголовки) и покрыты юнит-тестом локализации (`core.rs:1085`). Вот это и есть целевая форма.

---

## 7. Дефекты, приоритизированные

| ID | Severity | Дефект | Доказательство | Фикс | Эффект |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **P1-1** | высокий | В `apps/admin/input.css` в `@source` нет `crates/ui/rustok-grid/leptos/src` → 13 утилит грида отсутствуют в собранном CSS | сборка 114 258 → 116 515 Б; список утилит в §5.2 | +1 строка в `input.css` + гейт на покрытие | таблицы с рамками/выравниванием, фокус-ринги в гриде |
| **P1-2** | высокий | Ширины/resize колонок задаются атрибутом `style`, а CSP — `style-src-attr 'none'`; профильный гейт мёртв (`ENOENT`) | `header.rs:76`, `row.rs:80`, `security_headers.rs:28`, падение гейта | перевести ширины на CSS-переменные/`<colgroup>`+классы либо внести легитимное исключение в реестр; починить гейт | resize начинает работать в проде, CSP-гигиена восстанавливается |
| **P1-3** | высокий | Сортировка не работает ни в одной таблице (0/30 `sort_state`), при `sortable: true` по умолчанию | §3.1 | клиентская сортировка в гриде **или** `sortable: false` по умолчанию + гейт «sortable ⇒ on_sort_change» | интерфейс перестаёт врать |
| **P2-1** | средний | `messages` передан 1/30 → английская обвязка грида в ru-админке; ручные склейки вместо `GridMessages::selected` | §3.2 | проставить `messages` во всех 30 местах; гейт «DataGrid без messages — ошибка» | локализация, корректная плюрализация |
| **P2-2** | средний | ДС внедрена в 9/46 модулей; 1173 сырых контрола; гейта нет | §4.2 | ratchet-гейт по числу сырых контролов на модуль + миграция по волнам | единый вид, фокус/disabled/a11y из коробки |
| **P2-3** | средний | `rustok-channel` UI: 0 ДС, 0 `dark:`, 0 `aria-*`, wire-токены в `<option>`, неиспользованный ru-перевод, мёртвая переменная | §6.2 | переписать рендер на ДС + `Badge/Alert`, добавить `aria-*`, использовать `primarySummary` | модуль перестаёт быть выбросом |
| **P2-4** | средний | `child_pages` объявлены, но UI подпуть не читает: `channel`, `groups`, `pages`, `seo` (4 из 9 модулей с child pages) | §6.2, эвристика по коду | читать `module_path`/передавать активную секцию; гейт «если объявлены child_pages — они обрабатываются» | ссылки вторичной навигации перестают быть пустыми |
| **P2-5** | средний | Витрина `/design-system` не показывает `Select`, `Dialog`, `Tabs`, `Avatar`, `Skeleton`, `Spinner`, `Progress`, `Toc`, `DataGrid` | §4.5 | дополнить витрину (особенно `DataGrid` с resize/filters/selection) | дефекты CSS/ширин видно глазами, а не в проде |
| **P3-1** | низкий | `GridColumnDef::pinned` не реализован (только скрывает из «Столбцы») | `toolbar.rs:22` | реализовать sticky либо убрать поле | контракт без мёртвых полей |
| **P3-2** | низкий | Мёртвый токен-слой `--iu-*`, тема на `[data-theme]` вместо `.dark`; радиусы 4/6/8 vs 6/10/14 vs 14 | §5.4 | либо подключить `--iu-*` в Tailwind (`theme.extend`), либо удалить слой; привести `tokens.json`/`--radius` к одному масштабу | один источник истины для веб/Flutter |
| **P3-3** | низкий | Нет виртуализации, мульти-сортировки, экспорта, клавиатуры и персистенции ширин. Sticky header — **только паритет разметки**: Leptos-шапка получила те же `sticky top-0 z-10`, что у Dioxus-двойника, но липкой она от этого не стала и у двойника тоже не была. Причин три, и все проверяемые по коду: ближайший scrollport — обёртка `overflow-x-auto`, которая никогда не скроллит вертикально (её высота ничем не ограничена); над ней ещё один `overflow-hidden`; а `position: sticky` на `<thead>` внутри таблицы с `border-collapse: collapse` в Chromium ненадёжен. Чтобы шапка действительно липла, нужно решение по layout (ограниченная высота + `overflow-y-auto` либо sticky-`<th>` с `border-collapse: separate`) и браузер для проверки. Попутно выяснилось, что «одинаковые API адаптеров», обещанные в обоих `CRATE_API.md`, ничем не проверялись и уже не совпадали: новый гейт `verify-ui-grid-adapter-parity` сверяет компоненты и пропсы по именам и требует ревьюированного исключения с причиной на каждое расхождение. Найдено 5 настоящих расхождений (см. таблицу ниже): три — открытые дыры, два — разные механизмы при одинаковом результате. | §2.4 | по потребности | — |
| **P3-4** | низкий | Иконки нарисованы руками: 39 инлайновых `<svg>` в админке и модулях, у каждого своя толщина обводки, свой размер и своё решение про `aria-hidden`; один глиф был одновременно подписью кнопки, другой объявлялся скринридеру дважды. | §6.4 (обход `<svg>`) и гейт `verify-ui-ds-adoption`: «1322 raw, 21 hand-drawn `<svg>`, 43 DS import sites». Негативный прогон: лишний `<svg>` в подставном крейте даёт «hand-drawn `<svg>` 0 → 1 (+1)», после удаления гейт снова зелёный. `verify-ui-parity` — витрина 18/18; сборка Tailwind 120 174 Б (`shrink-0` уже в бандле). | `Icon`/`IconName` в `rustok-ui-leptos`: 21 форма, Lucide-геометрия инлайном, без внешних крейтов; доступность решена в одном месте — по умолчанию `aria-hidden="true"`, с `aria_label` становится `role="img"`, третьего состояния нет. Где админка уже рисовала глиф сама (`Shield`, `Package`), сохранена её точная геометрия, чтобы конверсия была пиксель-в-пиксель. SSR-тесты: декоративность, именованный режим, отсутствие дублей и пустых наборов путей. Третий ratchet-счётчик `svg` в `verify-ui-ds-adoption`. Волна 1: `modules_list.rs` (щит + пакет), `user_menu.rs` (шеврон раскрытия). | Новый рисованный глиф в хосте или модуле падает так же, как новый сырой `<button>»; 21 сайт остаётся в хвосте — строка ниже. |
| **P3-4 (хвост)** | низкий | 21 сайт, каждый — отдельное решение (двадцатый и двадцать первый нашлись при глубоком ревью: метрика `svg` читала только `<svg>` и не видела Dioxus-форму `svg { … }`, после расширения всплыл глиф в `crates/modules/rustok-page-builder/admin/src/ui/dioxus_canvas.rs`; сам счётчик baseline 20 → 21): анимированный переключатель темы в `apps/admin/src/shared/ui/mod.rs` (два наложенных глифа переходят CSS-транзишеном — пересоздание через компонент сбросило бы анимацию), переключатель сайдбара в `widgets/app_shell/header.rs` (глиф со стрелкой, зависящей от состояния), иконки модулей в `module_card.rs` (своя геометрия «wifi»), динамические пути в `user_menu.rs`, `auth_admin.rs`/`login.rs` (старая feather-геометрия — конверсия изменила бы вид), а также `<rect>`-глифы page-builder (структурная графика редактора, не иконки). | — | — | — |
| **P3-5** | низкий | Дублирование классов в модулях (78 уникальных строк в channel) | §4.3 | извлечь локальные подкомпоненты поверх ДС | минус ~1000 строк |
| **P3-6** | низкий | Нет e2e/визуальных тестов Leptos-админки (Playwright только в `next-*`) | `apps/admin/tests` — только Rust-тест; `playwright.config.*` в `next-admin`, `next-frontend` | smoke на `/design-system` + одна таблица | ловит P1-1/P1-2 |
| **P3-7** | низкий | Паритет Rust↔TS был двусторонним **по перечисленным** файлам: `MODULES` сравнивает 10 модулей, а `url.ts` жил отдельным спец-блоком и `order.ts` не сравнивался вообще — новый `.ts` мог появиться в пакете без всякого решения. | Гейт: «10 module twins, 212 Rust items, 211 TypeScript exports, 146 reviewed pairs, 32 justified one-sided symbols»; `verify-ui-grid-parity.test.mjs` 0 fail; три негативных прогона на копии дерева — новый `extra.ts` («neither compared nor listed»), новый экспорт в `order.ts` («a new boundary symbol needs a decision»), локальное определение в `index.ts` («barrel defines a symbol instead of re-exporting one»). | Гейт требует решения для **каждого** `.ts`-файла пакета: либо он в `MODULES`, либо это баррель `index.ts` (и тогда ему запрещено что-то определять самому, только реэкспорт), либо он в новой таблице `TS_ONLY_FILES` с причиной и **точным** списком экспортов. `order.ts` описан как обоснованный TS-only: Rust сравнивает `str` по байтам UTF-8, то есть по кодовым точкам, двойник — `Ord`, а не функция; `<` в JS сравнивает UTF-16 и расходится вне BMP. `url.ts` — двойник `apply_ui_query_pairs` из `rustok-ui-core` плюс `queryParam` как boundary-only. Описано в `packages/rustok-ui-grid/README.md` (таблица Layout + Known Limitations). | Новый `.ts` без решения падает гейтом, а не считается «раз собрали — значит сравнили». |

---

## 8. Что делать (волны)

**Волна 0 — «одна строка, много пользы» (полдня):**
1. `@source "../../crates/ui/rustok-grid/leptos/src/**/*.rs";` в `apps/admin/input.css`.
2. `messages=GridMessages::for_locale(ui_locale.as_deref())` во всех 30 местах `DataGrid` (механика), `aria_label` рядом.
3. Починить либо явно запарковать `verify-csp-inline-style-exceptions` (сейчас он падает `ENOENT` и ничего не проверяет) — иначе реестр CSP остаётся фикцией.

**Волна 1 — «интерфейс не врёт» (2–3 дня):**
4. Решить судьбу сортировки: (а) клиентская сортировка внутри `DataGrid` (по образцу клиентской пагинации — там уже есть деривация), либо (б) `sortable: false` по умолчанию + гейт. Вариант (а) чинит 30 модулей одним изменением.
5. Ширины колонок — на классы/`<colgroup>` вместо `style=`; resize проверять на реальной CSP-странице (`apps/server`), а не в `trunk serve`.
6. ~~Дополнить `/design-system`~~ — **сделано**: `Select`, `Dialog`, `Tabs`, `Avatar`, `Skeleton`, `Spinner`, `Progress`, `TableOfContents`, `DataGrid` (сортировка + пустое состояние) в трёх секциях, покрытие закреплено гейтом.

**Волна 2 — «ДС становится обязательной» (1–2 недели, ratchet):**
7. Гейты: `verify-tailwind-sources` (все каталоги, эмитящие классы, покрыты `@source`), `verify-grid-messages` (`<DataGrid` без `messages`/`aria_label` — ошибка), `verify-ui-child-pages` (объявленные `child_pages` обрабатываются), `verify-ui-grid-adapter-parity` (Leptos- и Dioxus-адаптеры `rustok-grid` не расходятся без исключения — **реализован**, `npm run verify:ui:grid-adapter-parity`), `verify-ui-ds-adoption` (ratchet `raw`↓/`ds`↑ по 47 крейтам — **реализован**, `npm run verify:ui:ds-adoption`), `verify-ui-token-parity` (tokens.json → base.css → Tailwind → сгенерированные артефакты — **реализован**, `npm run verify:ui:token-parity`), `verify-ui-dark-variants` (палитровые классы обязаны иметь `dark:` — **реализован** как ratchet: enforced-root'ы обязаны держать ноль, остальное печатается как цензус; `npm run verify:ui:dark-variants`, CI-шаг в `ui-i18n-verify.yml`).
8. Мигрировать модули по волнам, начиная с лидеров по нарушениям: `page-builder` (237), `product` (205), `forum` (79), `groups` (65), `ai` (61), `pages`/`seo` (по 59). `channel` — **сделан** (51 → 0, см. §11 P2-2): он и был обкаткой паттерна, по которому теперь можно идти по остальным. Всего сырых контролов по модулям 1322 в 42 крейтах + 117 в `apps/admin`.

**Волна 3 — «токены и иконки»:**
9. Один масштаб радиусов (решить: 6/10/14 из `tokens.json` или 4/6/8 из `--radius`) и либо подключить `--iu-*` в Tailwind, либо удалить `base.css` из импортов, чтобы не было двух источников истины.
10. `Icon`-компонент в `rustok-ui-leptos` и замена 39 инлайновых `<svg>` — **сделано частично**: компонент есть, ratchet-счётчик `svg` заведён, волна 1 закрыта, осталось 20 сайтов, каждый требует отдельного решения (см. «P3-4 (хвост)»).

---

## 9. Как это перепроверить (команды)

```bash
# 1) CSS: собрать и увидеть дыры
cd apps/admin && npm ci
npx tailwindcss -i input.css -o /tmp/before.css --minify
# добавить строку @source для rustok-grid/leptos во временную копию input.css и собрать снова
grep -c 'border-collapse\|align-middle\|max-h-56' /tmp/before.css

# 2) Гейты UI (все были GREEN на момент обзора)
for g in verify-ui-grid-parity verify-ui-i18n-keys verify-ffa-ui-migration-contract \
         verify-ffa-ui-boundary-sweep verify-ffa-ui-transport-profile-sweep \
         verify-ui-client-dependency-boundary verify-ui-parity; do
  node scripts/verify/$g.mjs; done

# 3) Гейт CSP — на момент обзора падал до проверок (ENOENT .../pages/admin/src/starters.rs).
#    Починен в §11: путь перетаргетирован, в область добавлен crates/ui.
node scripts/verify/verify-csp-inline-style-exceptions.mjs

# 3b) Гейты, добавленные по §11 (все в CI)
node scripts/verify/verify-ui-tailwind-sources.mjs
node --test scripts/verify/verify-ui-tailwind-sources.test.mjs
node scripts/verify/verify-ui-grid-messages.mjs
node scripts/verify/verify-ui-child-pages.mjs
node scripts/verify/verify-ui-grid-adapter-parity.mjs
node scripts/verify/verify-ui-token-parity.mjs
node scripts/verify/verify-ui-ds-adoption.mjs
node scripts/verify/verify-ui-dark-variants.mjs

# 4) Подсчёты, на которых построены таблицы §3–§4
grep -rl '<DataGrid' --include=*.rs crates/modules | wc -l                 # 30
grep -rl 'rustok_ui_leptos' --include=*.rs crates/modules | cut -d/ -f3 | sort -u | wc -l   # 9
grep -ro '<button\|<input\|<select' --include=*.rs crates/modules/*/admin/src | wc -l      # 1173
grep -rn 'sort_state' --include=*.rs crates/modules | wc -l               # 0 в местах вызова
```

---

## 10. Оговорки

- Rust не компилировался и тесты не запускались: `cargo`/`rustc` в песочнице отсутствуют. Все утверждения о поведении кода — из чтения исходников (с точными ссылками `файл:строка`), о CSS — из реальной сборки Tailwind.
- Подсчёты «модулей/файлов» сделаны по регулярным выражениям по `crates/modules/*/admin/src/**/*.rs`; эвристика «модуль обрабатывает `child_pages`» — по наличию `subpath`/`child_page`/`module_path` в коде модуля, это не 100 %-й анализ потока данных.
- `npm install` в `apps/admin` выполнялся только для сборки CSS; `node_modules` в снапшот не попадает, файлов в репозитории эта проверка не изменила (единственный добавленный файл — эта записка).
- Прогон 1224 гейтов в этом обзоре не повторялся: авторитетны базовый (`29f184e`, 333 провала) и веточный (331) списки; новых провалов ветка не даёт.

---

## 11. Статус исправлений (обновлено 2026-11-26, ветка `arena/a3d16606-rustok`)

Здесь фиксируется, что из §7 закрыто кодом и/или гейтом, а что осталось. Правило простое:
строка считается закрытой только если регресс ловит гейт **и** поведение соответствует
документации, а не наоборот.

Два прохода глубокого ревью по уже сделанному (§11 ниже — актуальное состояние, а не первая
редакция): первый нашёл, что `<Show when=move || flag>` нельзя навешивать на статические флаги —
`ChildrenFn` захватывает соседние локальные `String`/`bootstrap`, поэтому все такие обёртки
переписаны в `{if flag { view! { … }.into_any() } else { ().into_any() }}`; второй — что проверка
`dark:`-пар смотрела только `class="…"`-атрибуты и пропустила классы, возвращаемые функциями, что
`<select>` в channel-карточке не предвыбирал тип таргета на SSR, что `aria-label` поля ввода брался
из placeholder, что `<colgroup>` в `table-auto` был рекомендацией, а не шириной, и что вставленные
`aria_label=` строки нарушали порядок `use`-блоков и стиль разделителей в `view!`. Всё это
исправлено в этой же ветке, вместе с мелочами, найденными по ходу: отступы в переписанных блоках,
порядок `use` в `rustok-forum`/`rustok-product`/`rustok-grid`, имя теста, которое утверждало не то,
что проверяет.

| ID | Статус | Что сделано | Чем это доказано |
| :--- | :--- | :--- | :--- |
| **P1-1** | ✅ закрыт | Новый гейт `scripts/verify/verify-ui-tailwind-sources.mjs` (+ `.test.mjs`), `package.json` (`verify:ui:tailwind-sources`), CI-шаг «UI Tailwind source coverage». Корни — каталоги, а не файлы; оба хоста и `content`, и `@source`. | Гейт: «2 hosts, 77 class-emitting roots, both @source and content in sync», `node --test` 5/5; сборка storefront 93 184 → 96 498 Б, у `rustok-ui-forms` пропавших утилит 11 → 0 |
| **P1-2** | ✅ закрыт | Ширины колонок — `<colgroup><col width>` вместо `style=` (`grid.rs`, `header.rs`, `row.rs`); preview-ширина Fly-канваса — `data-fly-viewport` + правило в уже разрешённом `<style nonce>` (`fly/leptos/src/editor.rs`). Гейт `verify-csp-inline-style-exceptions` починен: путь `rustok-pages/admin/src/starters.rs` → `rustok-pages/src/services/starter_presets.rs`, в область сканирования добавлен `crates/ui` (без него гейт физически не мог увидеть исходный дефект). | Гейт зелёный (0 сайтов, ratchet 0/0) **и** падает от искусственно внедрённого `style=` в `crates/ui/rustok-grid/leptos/src/row.rs` (проверено, затем откатано); новый Rust-тест `canvas_preview_width_is_data_not_an_inline_style` |
| **P1-3** | ✅ закрыт | Клиентская сортировка в `DataGrid`: проп `sort_cell: (row, column) -> CellValue`, сортировка до нарезки страницы через `SortState::sort_items`, сброс страницы, `sortable` в шапке включается только если сортировка реально возможна (`sort_enabled`). | `sort_cell` используется в `rustok-auth/admin/src/ui/users.rs`; API и поведение описаны в `rustok-grid/leptos/README.md` (§Sorting) и `CRATE_API.md`; сетка `verify-ui-grid-parity` зелёная |
| **P2-1** | ✅ закрыт | Локализация обвязки вынесена в одно место: `GridMessages::locale()`, контекст `GridLocale` + `provide_grid_locale`/`use_grid_locale`, админ-хост проставляет локаль один раз. Плюс `aria_label` там, где на странице несколько таблиц (18 сайтов, ключи в FTL en/ru). Новый гейт `verify-ui-grid-messages` + CI-шаг. | Гейт: «43 `<DataGrid>` call site(s), 5 multi-table page(s) all named»; `verify-ui-i18n-keys` 3 588 ключей, 0 некаталогизированных; `verify-ui-i18n-parity` 77 каталогов |
| **P2-2** | ✅ закрыт (гейт + волны 1–2) | Ratchet-гейт `scripts/verify/verify-ui-ds-adoption.mjs` (`raw`/`svg` только вниз, `ds` только вверх, baseline в `scripts/verify/ui-ds-adoption-baseline.json`). Волна 1: форма создания канала — `Input`/`Button`/`Alert`. **Волна 2: весь `crates/modules/rustok-channel/admin/src/ui/leptos` переведён на ДС целиком — 51 сырой контрол → 0**: `channel_card.rs` (31 → 0: жизненный цикл канала, панели Targets/Modules/OAuth Apps), `policy_set_card.rs` (16 → 0: карточка набора, кнопки правил, форма правила), `policy_workbench.rs` (4 → 0: форма создания набора). Заодно ушли два рукописных `<select>` с `prop:value`+`selected`-обвязкой и два `<input type=checkbox>` с ручным `event_target_checked`, то есть исчез целый класс дублирования, а не только разметка. **Что вскрылось в самом ДС:** `Button::on_click` — это `Box<dyn Fn()>`, поэтому обработчик, которому нужен `MouseEvent` или который «съедает» данные (строка таблицы, async-запрос), напрямую не выражается. Рабочий паттерн (он же в `rustok-blog`): `on_click=Box::new({ let id = row.id.clone(); …; move \|\| action(row_payload.clone()) })` — дорогие клоны один раз при сборке view, внутри замыкания только дешёвые `clone()`/`set()`. Для полей: `Input::value/set_value` (`ReadSignal`/`SignalSetter`), `Input::derived_value` + `on_change` там, где значение производное (число приоритета), `Select::options: Signal<Vec<SelectOption>>`, `Checkbox::checked/set_checked`. **Видимые изменения** (осознанные, не пиксель-в-пиксель): кнопки h-10 → h-9 (размер `Md` ДС), «удалить» — вместо `border-rose-200 text-rose-700` нейтральная обводка с `text-destructive` (класс `dark:`-пары у розовой пары был, но это был единственный цвет вне токенов), поля ввода — `h-9 px-3` ДС. Шесть новых FTL-ключей вместо догадок о доступных именах: `channel-card-nameLabel`, `channel-oauth-roleLabel`, `channel-policies-priority\|actionChannel\|surface\|oauthApp` (en+ru). | Гейт: «47 crate(s), 1322 raw control(s) (baseline 1322), 20 hand-drawn <svg>, 43 design-system import site(s)» — было 1373/20/40, то есть `raw` −51 и `ds` +3; `verify-ui-dark-variants` по enforced-root 0 непарных; `verify-ui-i18n-keys` 3602 вхождения, 0 некаталогизированных; `verify-channel-admin-boundary`, `verify-ui-parity`, `verify-ui-tailwind-sources` зелёные |
| **P2-3** | 🟡 частично | `rustok-channel`: hardcoded `" · primary"` заменён на каталожный `channel-targets-primarySummary` (мёртвая переменная `_targets_primary_summary_template` оживлена); wire-токены в `<option>` заменены на 5 переводимых меток + `aria-label`; `<select>` получил `prop:value` (при редактировании таргета тип больше не сбрасывался на первый); 33 пары `dark:` для всех 42 палитровых классов, непарных 0. **Перепроверка нашла дыру в самой правке**: классы, которые возвращает функция (`resolution_outcome_badge_class`), не были покрыты — инструмент смотрел только `class="…"`-атрибуты; три бейджа (Matched/Miss/Rejected) остались без тёмной темы и исправлены. Чтобы это не вернулось, добавлен гейт-ратчет `verify-ui-dark-variants` (enforced root = `rustok-channel/admin`, 0 находок; остальной репозиторий измеряется: 354 строки — 288 `crates/modules`, 24 `crates/ui`, 42 `apps/admin/src`). | `node`-проверка «unpaired palette tokens: 0»; FTL: `channel-targets-type*` в en/ru, `verify-ui-i18n-keys` для `rustok-channel/admin` 154 ключа; `verify-channel-admin-boundary` 16/16 |
| **P2-4** | ✅ закрыт | Все объявленные `child_pages` диспетчеризуются **модулем**: `channel` (`targets`/`apps` → `ChannelAdminSection`, карточки показывают только свою секцию, форма создания и policy workbench скрыты вне landing), `groups` (4 воркспейса, до этого все стопкой на одной странице), `pages` (`all` — список без формы создания, `new` — только форма), `seo` (`overview` → панель Diagnostics, `?tab=` имеет приоритет). Плюс явные ветки там, где работал только fallthrough: `blog` (`posts`/`new`), `forum` (`topics`/`categories`), `product` (`products`). У `search` из манифеста убран `engines`: такой поверхности нет — выбор движка живёт в Overview, и ссылка вела в тот же экран. Новый гейт `verify-ui-child-pages` + CI-шаг. | Гейт: «25 declared surface(s) across 9 module(s)»; до правок он падал на 8 модулях (channel, groups, pages, seo, forum, product, search, workflow) — реальный негативный прогон, не гипотеза |
| **P2-5** | ✅ закрыт | Витрина `/design-system` дополнена тремя секциями: **Selection & Overlays** (`Select` — controlled и с placeholder, `Tabs` с `on_select`/`active` от сигнала, `Dialog` c `DialogTitle/Description/Close` и controlled `open`), **Status, Identity & Navigation** (`Spinner` трёх размеров, `Progress` determinate + indeterminate, `Skeleton` в трёх вариантах, `Avatar` четырёх размеров, `TableOfContents`), **Data Grid** (`DataGrid` с `sort_cell` и типизированным `cell_renderer` + отдельная сетка с `empty_message`, плюс кнопки «очистить/восстановить», чтобы пустое состояние было видно). Строк стало 751 → 1101. Гейт `verify-ui-parity` теперь пинит обязательную поверхность витрины: 18 компонентов, которых не хватало девяти. | `verify-ui-parity`: «Leptos UI Workbench renders the whole required surface (18 components)»; негативный прогон — переименование `<Progress` в несуществующий компонент даёт `[FAIL] Leptos UI Workbench does not render: Progress`; `verify-ui-grid-messages` 43 → 45 call site(s), 6 многотабличных страниц, все с `aria_label`; сборка Tailwind 120 174 Б |
| **P3-1** | 🟡 частично (документация приведена к поведению) | `pinned` — не «мёртвое поле», а маркер структурной колонки: `checkbox()`/`actions()` помечают свои колонки, чтобы тулбар не мог их скрыть. Что было неправдой — обещание sticky-рендера: `crates/ui/rustok-grid/dioxus/CRATE_API.md` утверждал «pinned columns», хотя ни один адаптер их не позиционирует. Теперь обе `CRATE_API.md` говорят явно: колонка остаётся в потоке и уезжает при горизонтальном скролле; `pinned` = «не показывать в меню колонок». Остаётся продуктовый выбор: реализовать sticky (нужны кумулятивные `left`/`right` из `ColumnWidths`) или убрать поле вместе с `PinnedSide`. | Документация совпадает с поведением; `verify-ui-grid-parity` зелёный; поведение не менялось |
| **P3-2** | ✅ закрыт | Один масштаб радиусов — токенный (6/10/14, как в `tokens.json` и `.g.dart`): `apps/admin/tailwind.config.js` и `apps/storefront/tailwind.config.cjs` теперь берут `borderRadius` из `var(--iu-radius-lg\|md\|sm)`, приватные `--radius` (0.5rem в админке, 0.875rem в витрине) удалены, `apps/storefront/assets/input.css` импортирует `UI/tokens/base.css` — слой `--iu-*` перестал быть мёртвым и стал единственным источником радиусов в сети. Тёмный блок `base.css` теперь ловит и `.dark` (класс, который реально ставит `apps/admin/index.html`), и `[data-theme="dark"]`. Новый гейт `verify-ui-token-parity.mjs` пинит цепочку tokens.json → base.css → Tailwind-конфиги обоих хостов → сгенерированные Rust/Dart. | Гейт: «2 host(s) (admin, storefront), 16 token value(s) in sync, radius scale sm=6px md=10px lg=14px»; сборки Tailwind (`--minify`) проходят: admin 120 174 Б, витрина 96 498 → 97 642 Б (+1 144 Б — импорт токен-слоя), в обоих бандлах `.rounded-lg { border-radius: var(--iu-radius-lg) }` и определены `--iu-radius-*`; негативные прогоны на подставном дереве: падает на приватном `--radius` в конфиге, на пропавшем `@import`, на `[data-theme]`-only и на расхождении 12px vs 14px |
| **P3-3** | 🟡 частично (паритет разметки шапки + гейт паритета адаптеров) | Sticky-шапка: `<thead>` Leptos-сетки получил `sticky top-0 z-10`, как у Dioxus-двойника — расхождение никто не видел, потому что API двух адаптеров не сравнивал ни один гейт. **Перепроверка глубоким ревью показала, что это только паритет разметки**: у обоих адаптеров ближайший scrollport — `overflow-x-auto`-обёртка без ограничения высоты (вертикально не скроллит никогда), над ней `overflow-hidden`, а sticky-`<thead>` в таблице с `border-collapse: collapse` в Chromium ненадёжен. Липкость остаётся открытой и требует решения по layout плюс браузера; в коде это записано в doc-комментарии `GridHeader`, чтобы никто не принял класс за работающее поведение. Новый гейт `scripts/verify/verify-ui-grid-adapter-parity.mjs`: извлекает `#[component] pub fn` и имена пропсов из обоих адаптеров, падает на любом одностороннем пропсе без ревьюированного исключения и на устаревшем исключении. Извлечение оказалось не самым простым местом: doc-комментарий с `DataGrid(...)` сдвигал глубину скобок (пропсы обрезались до 5 и гейт был ложно-зелёным), generic-компоненты `DataGrid<T, K, KF>`/`GridRow<T>` не матчились вовсе, а `#[prop(optional)] name: …` в одну строку терялось вместе со строкой. Теперь 7 компонентов и 64 общих пропса видны полностью. | Гейт: «7 component(s) in both adapters, 64 shared prop(s), 8 reviewed one-sided prop(s)»; негативные прогоны на копии дерева: лишний пропс в Leptos-тулбаре, переименованный компонент в Dioxus, устаревшее исключение и исключение с причиной «TODO» — все четыре дают exit 1; CI-шаг «Grid adapter parity» + `npm run verify:ui:grid-adapter-parity`. Остаток P3-3 и почему он не закрыт здесь: виртуализация (нужно решение по порогу строк и замена `<For>` на оконный рендер — это переписывание рендера, а не правка), мульти-сортировка (меняет контракт `SortState` и формат `aria-sort`), экспорт (продуктовое решение: CSV на клиенте или серверный endpoint с правами), клавиатурная навигация по сетке (правильный вариант — roving tabindex/lib `grid`-паттерн; построчный `tabindex` на каждой ячейке даёт по десять точек табуляции на строку, то есть хуже, чем сейчас, — поэтому не «полу-сделано»), персистенция ширин (нужен выбор хранилища на хосте). Ни одно из них не compile-проверяемо в этой песочнице: `cargo`/`rustc` отсутствуют. |
| **P3-4** | 🟡 частично (компонент + ratchet + волна 1) | `Icon`/`IconName` в `rustok-ui-leptos`; гейт `verify-ui-ds-adoption` получил третий ratchet-счётчик `svg`; волна 1: `modules_list.rs`, `user_menu.rs`. **Глубокое ревью нашло дыру в самом счётчике**: `RAW_SVG = /<svg\b/` видел только Leptos-разметку, а Dioxus рисует `svg { … }` внутри `rsx!`, то есть все Dioxus-глифы считались нулём; шаблон расширен до `<svg\b\|(?<![\w])svg\s*\{`, ratchet тут же поймал ранее невидимый сайт в page-builder, baseline 20 → 21. Негативные прогоны: RSX-глиф в подставном крейте даёт «hand-drawn <svg> 0 → 1», удаление глифа возвращает зелёный. Остальные 20 сайтов — в строке «P3-4 (хвост)» выше. | Гейт: «47 crate(s), 1322 raw control(s) (baseline 1322), 21 hand-drawn <svg> (baseline 21), 43 design-system import site(s) (baseline 43)» |
| **P3-5** | ⬜ открыт | — | Локальные подкомпоненты поверх ДС |
| **P3-6** | ⬜ открыт | — | Playwright-smoke для Leptos-админки (`/design-system` + одна таблица) |
| **P3-7** | ✅ закрыт | Гейт требует решения для **каждого** `.ts`-файла пакета: `MODULES` для сравниваемых, баррель `index.ts` без своих определений, или запись в `TS_ONLY_FILES` с причиной и точным списком экспортов (`order.ts` — `Ord` вместо функции, `url.ts` — двойник `apply_ui_query_pairs` плюс boundary-only `queryParam`). | Гейт: «10 module twins, 212 Rust items, 211 TypeScript exports, 146 reviewed pairs, 32 justified one-sided symbols»; три негативных прогона (новый `.ts`, новый экспорт, определение в барреле) |

#### Расхождения адаптеров `rustok-grid` (P3-3)

Обе `CRATE_API.md` обещают «identical component APIs». Проверка нашла семь компонентов с общим набором
и пять реальных расхождений; каждое записано исключением в гейте `verify-ui-grid-adapter-parity` с причиной.

| Расхождение | Сторона | Что это на самом деле |
| :--- | :--- | :--- |
| Клиентская сортировка (`DataGrid::sort_cell`, `GridHeader::sort_enabled`) | только Leptos | **Открытая дыра в Dioxus.** Leptos-сетка сортирует свои строки через `SortState::sort_items` и честно гасит `sortable` в шапке, когда сортировать нечем. Dioxus-сетка `data` не переставляет никогда: без `on_sort_change` её кнопки сортировки не делают ничего. |
| Режим публикации фильтров (`DataGrid::filter_commit`, `GridHeader::filter_commit`, `GridFilterCell::commit_mode`) | только Dioxus | **Открытая дыра в Leptos.** У Dioxus вызывающий выбирает `OnCommit`/`Live`; Leptos публикует через фиксированный дебаунс `INPUT_DEBOUNCE_MS` (250 мс) и ручки не имеет. |
| Доступ к ключу строки (`key_fn` ↔ `row_key`) | обе | Одно и то же в идиоме каждого фреймворка: `<For key=…>` требует `Fn(&T) -> K`, Dioxus берёт `Callback<T, String>`. Переименование 40+ сайтов дало бы общее имя при разных типах — не стоит. |
| Ширины колонок в строке (`GridRow::column_widths`) | только Dioxus | Разный механизм, одинаковая геометрия: Leptos задаёт ширину через `<colgroup><col width>` под `table-fixed` (инлайновый `style` режет CSP `style-src-attr 'none'`), Dioxus пишет ширину в `<td style>`. |
| Sticky-шапка | была только в Dioxus | **Паритет разметки, не исправление поведения**: `<thead>` Leptos-адаптера получил `sticky top-0 z-10`, как у двойника, но липкой шапка не была ни там, ни там. Причины: ближайший scrollport — обёртка `overflow-x-auto` без ограничения высоты (вертикально не скроллит), над ней `overflow-hidden`, а sticky-`<thead>` в таблице с `border-collapse: collapse` в Chromium ненадёжен. Реальная липкость требует решения по layout и браузера — остаётся открытой в P3-3. |

### Глубокое ревью волны 2 и новых гейтов

Отдельный проход по уже сделанному (не по плану), с перечитыванием диффа против `HEAD` и `git show HEAD:…`:

| Найдено | Чем плохо | Что сделано |
| :--- | :--- | :--- |
| `channel_card.rs`: `make_default` (замыкание `move`) забирает `token_for_default` / `tenant_for_default` / `channel_id_for_default`, а блок lifecycle (добавленный раньше в этой же ветке) клонирует их **после** — девять строк вида `let lifecycle_token_toggle = token_for_default.clone();` | Это `E0382: borrow of moved value` ×3 переменные: крейт не собирается. Ошибку некому было поймать: `cargo` в песочнице нет, а job `Cargo Check` в `ci.yml` красный ещё на `main` | Девять строк переведены на клонирование из базовых `token` / `tenant` / `channel_id` — те в замыкания не попадают, приём тот же, что во всём остальном файле (по одной паре клонов на замыкание) |
| `verify-ui-ds-adoption.mjs`: `RAW_SVG = /<svg\b/` | Dioxus рисует глифы как `svg { … }` внутри `rsx!`, то есть ratchet «рисованной геометрии» слеп ко всем Dioxus-хостам и показывает ноль там, где их девять | Шаблон расширен до `/<svg\b\|(?<![\w])svg\s*\{/`; всплыл невидимый сайт в `page-builder`, baseline `svg` 20 → 21; негативный прогон подтверждает, что RSX-глиф теперь ловится |
| `verify-ui-grid-adapter-parity.mjs`: `parameters()` | `>` в `-> ` считался закрытием generic-скобки, поэтому `Option<Box<dyn Fn() -> T>>` (ровно форма `Button::on_click`) ломал разбор списка параметров — сравнение адаптеров молча съезжало со следующего параметра | `>` после `-` больше не считается закрытием; проверено на синтетической сигнатуре с `Fn() -> T` в середине списка |
| `GridHeader` doc-комментарий | Утверждал, что шапка «липнет» — неправда (см. строку Sticky-шапка выше) | Переписан: parity-класс, поведение не работает, вопрос открыт в P3-3 |
| Проверки, которые никто не запускал | `verify-channel-lifecycle-invariants` (+ его 20 фикстур) жил только в `package.json`; SQLite-набор инвариантов жизненного цикла исполнялся только косвенно, внутри job'а `coverage`; PostgreSQL-набор с `#[ignore]` не запускался нигде — job `migration-smoke`, который даёт нужную переменную, висит на `needs: [check, coverage]`, а `Cargo Check` красный на `main` | Гейт и обе фикстуры подключены в CI: `.github/workflows/cache-hardening.yml` — Node-гейт первым шагом job'а `compiled-contract` (до `cargo`, потому что workspace-`check` там красный на `main`) и SQLite/PostgreSQL-наборы в job'е `postgres-channel` с живым PostgreSQL. `hardening-gates.yml` не годится: он в `PROTECTED_PATHS` гейта `verify-migration-infrastructure-approval`. Записано в `channel-audit-report-2026-11-26.md` §10.19 |
| Фильтры `paths` в `ui-i18n-verify.yml` | Workflow перечислял только i18n-файлы, а из UI — `crates/ui/fly/src/runtime_locale.rs`. Правка `crates/ui/rustok-ui/leptos/src/button.rs`, `crates/ui/rustok-grid/**`, `UI/tokens/**`, `apps/admin/src/**`, CSS/конфигов Tailwind или самого гейта **не запускала ничего** — то есть восемь новых гейтов не срабатывали ровно на тех изменениях, которые обязаны ловить | Оба блока (`push` и `pull_request`) дополнены: `crates/ui/rustok-ui/**`, `crates/ui/rustok-grid/**`, `crates/ui/fly/**`, `UI/tokens/**`, `apps/admin/src/**`, `apps/admin/input.css`, `apps/admin/tailwind.config.js`, `apps/storefront/src/**`, `apps/storefront/assets/**`, `apps/storefront/tailwind.config.cjs`, `scripts/verify/verify-ui-*.mjs`, `scripts/verify/verify-csp-inline-style-exceptions.mjs`, `scripts/verify/ui-ds-adoption-baseline.json` |

Проверки, которые при этом не нашли проблем и потому тоже стоят фиксации:

- **Ни одна ветка логики не потерялась** при переводе на DS-кнопки: 9 длинных тел `on:click` в `channel_card.rs` (сравнение байт-в-байт с `HEAD`, с точностью до нормализации пробелов), 7 в `policy_set_card.rs`; единственное отличие — убран `.clone()` у `channel_id`, который больше не переезжает в замыкание.
- **Ни один `disabled`-гард не потерян**: 16/16 в `channel_card.rs` (в том числе 5 новых `busy`-гардов на кнопках привязки модуля/OAuth), 7/7 в `policy_set_card.rs`, 1/1 в `policy_workbench.rs`.
- **Имена пропсов и импортов** сверены с реальными сигнатурами компонентов (`/tmp/ds_sigs.json`): 26 использований, 0 несуществующих пропсов, 0 неиспользуемых импортов.
- **6 новых FTL-ключей** используются ровно по одному разу и есть в обоих каталогах (`en`/`ru`).
- Ручной скан изменённых `.rs` на `use-after-move` (замыкание `move` + повторное использование переменной) больше ничего не даёт: остальные совпадения — затенение имён внутри блока, а не перемещение.

### Новые гейты этого прохода

| Гейт | Команда | Что запрещает |
| :--- | :--- | :--- |
| Tailwind source coverage | `npm run verify:ui:tailwind-sources` (+ `test:verify:ui:tailwind-sources`) | класс, который не виден ни одному `@source`/`content` (компонент рендерится без стилей) |
| Grid chrome localization | `npm run verify:ui:grid-messages` | таблицу без локали (нет ни явного `messages`, ни локального контекста от хоста) и несколько таблиц на странице без собственных имён |
| CSP inline-style register | `npm run verify:csp:inline-style-exceptions` | любой `style=` в UI-хостах и `crates/ui` вне реестра; заодно ловит «мёртвый гейт» (реестр с нулём проверок) |
| Admin child pages | `npm run verify:ui:child-pages` | объявленный `child_pages`, который модуль не читает из маршрута, или объявленный подпуть, которого нет в коде модуля |
| Dark-theme pairing | `npm run verify:ui:dark-variants` | палитровый класс без `dark:`-пары — в том числе в строках, которые возвращает функция, а не в `class="…"` |
| Design-system adoption ratchet | `npm run verify:ui:ds-adoption` | рост числа сырых `<button>`/`<input>`/`<select>`/`<textarea>`, новый рисованный `<svg>` или падение числа импортов ДС |
| Design-token parity | `npm run verify:ui:token-parity` | расхождение `tokens.json` → `base.css` → Tailwind-конфиги хостов → сгенерированный Rust/Dart, приватный `--radius` в хосте |
| Grid adapter parity | `npm run verify:ui:grid-adapter-parity` | односторонний пропс или компонент в Leptos/Dioxus-адаптерах `rustok-grid` без ревьюированного исключения; устаревшее исключение |

Все восемь включены в CI (`.github/workflows/ui-i18n-verify.yml`), а не только в `package.json`:
гейт, который никто не запускает, — это не защита от регресса.

### Оговорки этого прохода (важно)

- **Rust не компилировался и `cargo test` не запускался**: в песочнице нет `cargo`/`rustc`. Все Rust-правки проверены tree-sitter-разбором (синтаксис) и чтением типов по месту; утверждать «собирается и тесты зелёные» нельзя. Это не оговорка для галочки: глубокое ревью именно так нашло `E0382` в `channel_card.rs` (три `String`, перемещённые в замыкание `make_default` и клонируемые после него) — ошибку внёс более ранний проход этой же ветки, а поймать её было некому, потому что job `Cargo Check` в CI красный ещё на `main`. После правки ручной скан изменённых `.rs` на «замыкание `move` + повторное использование переменной» чист; окончательным судьёй остаётся `cargo check`.
- Гейты и их тесты (Node) запускались реально; приложенные числа — их вывод.
- `verify-i18n-contract` и `verify-fly-multilingual` красные **до** этого прохода и на базе `29f184e` тоже (`/tmp/base_fails.txt`): их причины не связаны с UI-обзором и здесь не трогались.
