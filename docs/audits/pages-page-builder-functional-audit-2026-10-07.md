# Инженерный аудит функциональности: Pages + Page Builder (Fly)

- **Дата:** 2026-10-07
- **Ревизия:** `a7b309d` (ветка `main`)
- **Что проверялось:**
  - `crates/modules/rustok-pages`: domain, GraphQL/REST, admin, storefront;
  - `crates/modules/rustok-page-builder` и его admin-редактор;
  - `crates/modules/rustok-page-builder-storefront`;
  - `crates/ui/fly` (`fly`, `fly-ui`, `fly-web`, `fly-leptos`, `fly-browser`);
  - точки встраивания в `apps/admin`, `apps/server`, `apps/storefront`, `apps/next-*`.
- **Связанный документ:** [`fly-builder-engineering-audit-2026-10-02.md`](./fly-builder-engineering-audit-2026-10-02.md). Он разбирает ядро Fly с точки зрения безопасности, целостности и производительности: FNV→SHA-256, XSS через `component.id`, 4 клона на команду и т.д. Здесь эти темы не повторяются. Этот аудит отвечает на другой вопрос: **что продукт умеет, чего не умеет и как он выглядит рядом с рынком.**

> **Ограничение методики.**
> - Аудит статический: в песочнице нет Rust-тулчейна (`cargo` и `rustc` отсутствуют, crates.io недоступен), поэтому код не собирался и тесты не запускались.
> - Все выводы получены чтением исходников, они даны со ссылками `файл:строка`.
> - Состояние CI взято из GitHub Actions через `gh run list` / `gh run view` (раздел 3.8).
> - Пункты с пометкой «высокая вероятность» нужно подтвердить ручным прогоном.

---

## 0. Резюме

### 0.1 Вердикт одним абзацем

В RusTok построена **инфраструктура публикации уровня enterprise**:
- неизменяемые артефакты с SHA-256-целостностью;
- fail-closed санитизация статического HTML;
- аудит, пересборка и активация артефактов, откат на предыдущую версию;
- история маршрутов с алиасами и tombstone-записями;
- инвалидация кэша через outbox;
- HMAC-гранты на inline-редактирование;
- rollout-профили на уровне capability;
- около 100 evidence-пакетов и около 70 верификаторов.

Над ней стоит **продукт уровня раннего прототипа**. Контент-менеджер:
- не увидит 21-ю страницу;
- не создаст страницу с кириллическим заголовком;
- не опубликует новую страницу без ручной правки шаблонного текста;
- не исправит опечатку на опубликованной странице, не сняв её с сайта;
- не вставит видео YouTube, не загрузит картинку, не сделает жирный шрифт;
- не получит заявку из контактной формы;
- не откроет страницу по чистому URL `/about`.

По соотношению «инфраструктура / пользовательская ценность» модуль сейчас перевёрнут: много защиты вокруг функциональности, которой почти нет.

### 0.2 Оценка зрелости по осям (0–5)

| Ось | Оценка | Комментарий |
|---|:-:|---|
| Модель данных и целостность публикации | **4.5** | Артефакты, дайджесты, ревизии, optimistic concurrency, outbox |
| Безопасность рендера | **4** | Fail-closed политика, CSP `default-src 'none'`, sandbox-iframe; ценой функциональности |
| Жизненный цикл контента (draft → review → publish) | **1.5** | Нет черновика поверх опубликованной версии, расписания, истории правок, превью-ссылок, корзины |
| Маршрутизация и SEO-URL | **1.5** | Алиасы и история есть, но публичный URL `/{locale}/modules/pages?slug=`; нет иерархии и homepage |
| Визуальный редактор (UX) | **1.5** | Около 27 панелей «для разработчика», нет inline-текста, rich text, загрузки медиа, поиска в палитре |
| Библиотека блоков и выразительность | **2** | 26 примитивов, 5 шаблонов; нет table/code/embed/video-embed/tabs/accordion/slider |
| Динамические данные | **2.5** | Bindings, conditions и repeaters в ядре есть; источников данных и UI для маркетолога нет |
| Формы и интеракции | **0.5** | Примитивы форм есть, но обработчика на сервере нет, CSP блокирует внешние action |
| Локализация | **2** | Метаданные переводятся, тело страницы — отдельный документ на каждую локаль; две параллельные модели i18n |
| Масштабирование админки | **0.5** | Список ограничен 20 записями, нет поиска, фильтров и пагинации |
| Совместная работа | **1** | Только optimistic concurrency; нет блокировок, presence, комментариев, review-флоу в UI |
| Тестируемость / CI | **1.5** | Core-тесты Fly и unit-тесты Pages зелёные; workflow `Fly Page Builder` **красный на `main`** |

### 0.3 Топ-12 находок

| # | Severity | Находка | Где |
|---|:-:|---|---|
| B-1 | **P0** | Админ-список страниц жёстко ограничен `page=1, per_page=20`: страницы начиная с 21-й **недоступны** из UI | `rustok-pages/admin/src/transport/graphql_adapter.rs:234` |
| B-2 | **P0** | `slugify` в админке только ASCII: заголовок на кириллице даёт пустой slug и ошибку `Required page field is missing: Slug` | `rustok-pages/admin/src/core.rs:6` (сервер `helpers.rs:74` Unicode допускает) |
| B-3 | **P0** | Новая страница **непубликуема «из коробки»**: шаблон кладёт `"<h1>{title}</h1>"` в текстовое поле, Fly экранирует его (теги видны буквально), а publish-политика отклоняет документ с `landing_content_markup_forbidden` | `admin/src/core.rs:~120`, `fly/src/render.rs:321`, `page-builder/src/static_publish_policy.rs:429` |
| B-4 | **P0** | Роль `manager` («автор без права публикации») **не может сохранить черновик**: кнопка Save в редакторе завязана на `capabilities.publish`, а builder-capability «Publish» означает сохранение | `page-builder/admin/src/editor/toolbar.rs:100`, `rustok-pages/admin/src/builder.rs:401`, `access.rs` |
| B-5 | **P0** | Кнопка Publish публикует **серверную** ревизию: несохранённые правки молча не попадают на сайт | `composition.rs` publish handler, `graphql_adapter.rs:~379` |
| B-6 | **P1** | Publish из админки требует **promoted scenario baseline**, даже для статической страницы без динамики. Без захода в технический «scenario panel» опубликовать нельзя | `graphql_adapter.rs:379` |
| B-7 | **P1** | Опубликованный документ неизменяем (`PAGE_PUBLISHED_DOCUMENT_IMMUTABLE`). Исправление опечатки = unpublish → **страница 404 на сайте** → правка → publish | `services/page/document.rs:154` |
| B-8 | **P1** | Нет autosave, `beforeunload`-guard и подтверждений для Delete/Unpublish | `composition.rs:~456–500` |
| B-9 | **P1** | Черновики браузерного редактора хранятся **в памяти процесса** (TTL 8 ч, до 10 000 записей): теряются при рестарте, не шарятся между репликами; худший случай — 10 000 × 16 MiB | `browser_intent.rs:38`, `draft_session.rs:70` |
| B-10 | **P1** | Endpoint `/api/admin/pages/{id}/builder/intents` смонтирован только в `apps/admin` (standalone SSR), в `apps/server` его нет | `apps/admin/src/main.rs:171` |
| B-11 | **P1** | Контактная форма из шаблона нефункциональна: серверного обработчика submit нет, а CSP артефакта `form-action 'self'` блокирует внешние endpoint'ы | `controllers/mod.rs` (CSP), `fly/src/action/model.rs` |
| B-12 | **P1** | Публичный URL страницы — `/{locale}/modules/pages?slug=…`; на главной Pages рендерятся только в слоте `home_before_footer` внутри демо-карточки, анонимным посетителям — через `<iframe>` без автовысоты | `services/page/route.rs:570`, `apps/storefront`, `storefront/src` |

---

## 1. Архитектура (как реально устроено)

```
┌────────────────────────── apps/admin (Leptos SSR+hydrate) ───────────────────────────┐
│  rustok-pages-admin                                                                  │
│   ├─ PageWorkspace (composition.rs): навигатор → create form → editor/locked view    │
│   ├─ PagesFlyBuilder ──► rustok-page-builder-admin (editor/*: ~27 панелей)           │
│   │                        └─ fly-ui state machine + fly-leptos iframe canvas        │
│   │                           └─ canvas_runtime.js (select/hover/drag, postMessage)  │
│   └─ transport/graphql_adapter.rs ──► GraphQL apps/server                            │
│  /api/admin/pages/{id}/builder/intents  (SSR no-JS intents, in-memory draft store)   │
└──────────────────────────────────────────────────────────────────────────────────────┘
                 │ GraphQL / REST
┌────────────────▼──────────────── apps/server ────────────────────────────────────────┐
│  rustok-pages (domain)                                                               │
│   ├─ services/page/{create, metadata, document, lifecycle, reviewed_publish,         │
│   │                  rollback, artifact_*, route*, inline_edit}                      │
│   ├─ page_builder_artifact.rs ──► rustok-page-builder (capability + static policy)   │
│   │                                  └─ fly (document model, validation, render)     │
│   ├─ cache_invalidation.rs (outbox generations), seo_targets.rs, translation_target  │
│   └─ controllers: /api/pages/{id}/artifact (CSP, ETag, max-age=60 swr=300)           │
└──────────────────────────────────────────────────────────────────────────────────────┘
                 │ native server fn / GraphQL
┌────────────────▼──────────── apps/storefront (Leptos) ───────────────────────────────┐
│  rustok-pages-storefront: /{locale}/modules/pages?slug=…, слот home_before_footer    │
│   ├─ аноним: <iframe sandbox src=/api/pages/{id}/artifact>                           │
│   └─ inline edit (HMAC grant) ──► rustok-page-builder-storefront (real-DOM adapter)  │
└──────────────────────────────────────────────────────────────────────────────────────┘
apps/next-admin, apps/next-frontend: страниц Pages нет (только playwright-конфиги)
```

**Ключевое архитектурное решение.** Тело страницы — это **Fly-проект**: GrapesJS-совместимый JSON с несколькими «Fly-страницами», стилями, ассетами, биндингами и переводами. Он хранится в `page_bodies` (TEXT + format) **отдельно для каждой локали**. Публикация компилирует проект в **статический HTML-артефакт**. Артефакт проходит строгую политику санитизации и хранится неизменяемым вместе с дайджестом.

---

## 2. Инвентарь функциональности: Pages (`rustok-pages`, ≈47 тыс. строк Rust)

### 2.1 Модель данных

| Таблица | Назначение |
|---|---|
| `pages` | Корень: tenant, status (draft/published/archived), template, metadata JSON, published_at, версия |
| `page_translations` | locale, title, slug, meta_title, meta_description |
| `page_bodies` | locale, content (TEXT), format (Fly/GrapesJS JSON), updated_at, используется как ревизия |
| `page_channel_visibility` | Видимость по каналам (`channel_slugs`) |
| scenario baselines | «Эталонные» runtime-сценарии для проверки динамики перед публикацией |
| landing artifacts / publish ops / rollback ops / rebuild ops / binding replacement ops | Журнал и артефакты публикаций |
| route aliases / publications / history | Алиасы, tombstone, импорт истории маршрутов |
| translation_change | Изменения для модуля переводов |

Замечания по миграциям:
- `m20260721_000004` необратима, `down` отсутствует.
- Префикс `m20260806_000014` встречается **дважды**. Порядок применения зависит от сортировки имени целиком; при переименовании возможен дрейф.
- FK дочерних таблиц используют `ON DELETE CASCADE`, поэтому удаление страницы уносит всю историю публикаций безвозвратно.

### 2.2 Жизненный цикл

```
           create                 save_document (только если не published)
  (none) ─────────► DRAFT ◄──────────────────────────────┐
                      │ publish (reviewed, baseline,     │ unpublish
                      │  static policy, artifact)        │
                      ▼                                  │
                  PUBLISHED ─────────────────────────────┘
                      │  rollback → предыдущий артефакт (только на 1 шаг)
                      │  archive  → только сервисный слой, нет в API/UI
  delete: hard delete, запрещён для published
```

- **Published = immutable** (`document.rs:154`). Механизма «черновик поверх опубликованной версии» нет. Ревизия — это строка `updated_at`, а не версионированная запись.
- **Reviewed publish:** публикация проверяет, что публикуется ровно та ревизия, которую видел автор. Это идемпотентно и защищено от гонок.
- **Rollback** только к *предыдущему* артефакту; выбрать произвольную версию нельзя.
- **Archive** реализован в сервисе, но не выведен в GraphQL, REST или UI.

### 2.3 API

**GraphQL — запросы:** `page`, `page_by_slug`, `pages(filter)`, artifact audit, rollout status, scenario baseline/status.

**GraphQL — мутации:** `create_page`, `patch_page_metadata`, `save_page_document`, `publish_page`, `rollback_page` (предыдущая версия), `unpublish_page`, `delete_page`, `rebuild_page_artifact` / `activate_page_artifact`, операции над baseline.

**REST:** CRUD плюс publish и `/api/pages/{id}/artifact`. **Unpublish в REST нет**, паритета с GraphQL нет.

**`ListPagesFilter`:** только `status`, `template`, `locale` и пагинация. Нет:
- полнотекстового поиска или поиска по title/slug;
- сортировки по updated_at, title или published_at;
- фильтров по автору, каналу и дате.

### 2.4 Маршрутизация и URL

- Сервер нормализует slug с сохранением Unicode (`helpers.rs:74`).
- Есть алиасы, tombstone-записи (старый URL помнится), импорт истории маршрутов. Это сильнее, чем у большинства headless-CMS.
- **Канонический публичный URL:** `/{locale}/modules/pages?slug={slug}` (`route.rs:570`). Это query-параметр под техническим префиксом модуля, а не `/about` или `/ru/o-nas`.
- **Иерархии нет:** нет parent/child, nested-путей, breadcrumbs, `/` в slug вырезается админским `slugify`.
- **Homepage нет:** назначить страницу главной нельзя. Главная storefront — захардкоженный commerce-лендинг.
- Зарезервированных slug (`admin`, `api`, `cart`…) нет.

### 2.5 Локализация

- Метаданные (title, slug, meta) переводятся через `page_translations` и интегрированы с модулем переводов (`translation_target.rs`).
- Тело хранится как **отдельный Fly-документ на каждую локаль** в `page_bodies`. Внутри Fly параллельно есть **своя** модель i18n:
  - `flyTranslations`;
  - `flyLocales`;
  - `ProjectLocalePolicy` с default, supported, required и fallback;
  - `TranslationCommand`.
- **Две модели локализации тела** — источник путаницы: копия документа на локаль против локализованных значений внутри одного документа. Канонической из них ни одна не объявлена.
- В админ-UI **нет переключателя локали**: редактор открывает одну локаль. Нет действия «создать перевод страницы / скопировать тело из локали X». Нет индикации, какие локали заполнены.
- Модуль переводов тело страницы не покрывает: «It does not include Fly/GrapesJS body content».

### 2.6 SEO

- Поля `meta_title` и `meta_description` есть.
- В метаданных Fly-страницы: canonical, robots (noindex), og-теги; при рендере артефакта они попадают в `<head>`.
- Есть интеграция с `rustok-seo` (`seo_targets.rs`).
- Пробелы:
  - структурированные данные (JSON-LD) не генерируются;
  - нет sitemap-флага на странице, preview сниппета и SEO-скоринга;
  - анонимный посетитель получает контент в `<iframe>`, поэтому основная индексация зависит от того, как краулер обработает iframe. Это **критично для SEO**.

### 2.7 Кэш и доставка

- `/api/pages/{id}/artifact`:
  - CSP `default-src 'none'`, хэш для стилей, `form-action 'self'`, `frame-ancestors 'self'`;
  - кэширование `max-age=60, stale-while-revalidate=300`, ETag.
- Outbox-поколения кэша (`cache_invalidation.rs`) — корректный паттерн для мульти-инстанса.
- На storefront аноним получает `<iframe sandbox="allow-forms allow-same-origin" src=…>`. Высота не подстраивается под контент: нет auto-height bridge, страница либо обрезана, либо с двойным скроллом.

### 2.8 Безопасность и доступ

- RBAC capability-модель `pages:read`, `update`, `publish` на уровне page-builder сервиса.
- В админке роли захардкожены строками (`access.rs`):

  | Роль | Доступ |
  |---|---|
  | `super_admin`, `admin` | full |
  | `manager` | всё, кроме publish |
  | все остальные, включая кастомные | read-only |

  Кастомные роли RBAC-модуля не учитываются: fail-closed безопасно, но негибко.
- Inline-редактирование на storefront:
  - HMAC-грант (constant-time сравнение исправлено в предыдущем аудите);
  - только статические листовые тексты;
  - только неопубликованные страницы. Отсюда парадокс: inline-редактировать «живую» страницу нельзя.

### 2.9 Админ-UI Pages

| Функция | Состояние |
|---|---|
| Навигатор страниц | Список **первых 20**; нет поиска, фильтров, сортировки, пагинации, row-actions, статуса публикации колонкой |
| Создание | Форма: title, slug (автогенерация ASCII-only), locale — **свободный текст**, channels — **CSV-строка**, template фиксирован `"default"` |
| Метаданные | Панель contributions: title, slug, meta_title, meta_description, template (text), channel_slugs (list) |
| Редактор тела | `PagesFlyBuilder` (см. §3) |
| Publish / Unpublish / Delete | Кнопки без подтверждения; Publish требует baseline; Delete только для неопубликованных |
| Rollback | Отдельный контрол, только на предыдущую версию |
| Заблокированный вид | Для published вместо редактора `PublishedDocumentLocked`, редактор размонтирован |
| Загрузка | 4 последовательных запроса при открытии (waterfall) |
| i18n UI | Часть строк захардкожена на английском |

---

## 3. Инвентарь функциональности: Page Builder + Fly

### 3.1 Слои и зрелость (по README/коду)

| Слой | Назначение | Зрелость |
|---|---|---|
| `fly` (core) | Модель документа, команды, валидация, рендер HTML, bindings, dynamic, actions, i18n, snapshot/bundle | Stable, около 300 тестов |
| `fly-ui` | Headless state machine редактора, capability policy, viewport, стили | Stable |
| `fly-web` | Браузерный runtime, origin-проверки, real-DOM inline | Beta |
| `fly-browser` (JS) | `fly-browser.js`, `canvas_runtime.js` | Beta |
| `fly-leptos` | iframe-канвас и подписки | Foundation |
| `fly-dioxus` | Обёртки | Foundation (по сути заглушка) |
| `rustok-page-builder` | `FlyAdapterBackedPageBuilderService`, capability-запросы, rollout-профили, static publish policy | Сервис зрелый, интеграционных тестов **3** |
| `rustok-page-builder-admin` | Leptos-редактор: около 27 панелей | Работающий, но «developer-grade» |
| `rustok-page-builder-storefront` | Real-DOM inline-адаптер | Beta |

### 3.2 Компоненты (26 типов)

| Группа | Типы |
|---|---|
| Layout | `wrapper`, `section`, `container`, `row`, `column`, `grid` |
| Content | `text`, `heading`, `list`, `link`, `image`, `video`, `media`, `button`, `divider`, `spacer` |
| Forms | `form`, `input`, `textarea`, `select`, `checkbox`, `submit` |
| Escape hatch | `raw_html` |

**Шаблоны секций (5):** `hero`, `two_columns`, `feature_grid`, `cta`, `contact_form`.

**Чего нет**, хотя это стандарт рынка:
- table, code/pre, blockquote в rich text;
- embed (YouTube, Vimeo, Maps, соцсети), иконка, галерея/слайдер/карусель;
- tabs, accordion/FAQ, countdown, map, social links, navbar/menu, footer;
- product card / product grid (притом что платформа — e-commerce), blog-post list, форма подписки.

### 3.3 Редакторские операции

- **Команды** (`EditorCommand`): Select, Insert, Remove, Move, Patch, Asset, StyleRule, Page, Dynamic, Binding, Context, Translation, Batch.
- **Undo/redo**: история с `with_history_limit`. По предыдущему аудиту, 2 полных документа на запись, без бюджета памяти.
- **Clipboard**: copy, cut, paste фрагментов в пределах сессии. Межстраничного или системного буфера нет.
- **Drag & drop**: перемещение выделенного через «move mode» и drop-зоны в канвасе. Перетаскивания из палитры на канвас **нет**, вставка идёт кнопкой.
- Горячие клавиши для undo/redo, удаления, перемещения вверх/вниз и отмены.
- **Viewport-пресеты:** Desktop 1440/1280/1024, Tablet 768, Mobile 430/390.

### 3.4 Стили

- Каталог свойств `builtin_style_properties()`: display, flex-*, grid-template-columns, gap, width/height/min/max, margin, padding, color, background, typography и т.д. Для части свойств есть enum-значения.
- **Responsive-стили:** `StyleRuleCatalog` с брейкпоинтами (`mobile-down` и др.) и media-query.
- UI сделан как **«выбери свойство из списка → впиши значение текстом»**. Нет:
  - визуальных контролов: color picker, box-model для отступов, слайдеров, typography-панели;
  - классов/токенов: всё задаётся инлайн или правилами на конкретный компонент;
  - **глобальных стилей и дизайн-токенов**: палитры, шрифтов, spacing scale. В Fly нет понятия theme/tokens/CSS-variables;
  - hover/focus/active-состояний и анимаций/transitions/scroll-эффектов.

### 3.5 Ассеты

- Панель Assets: **ручной ввод `id` + URL** (`asset_section.rs`).
- **Загрузки нет**: ни file input, ни drag-drop файла, ни интеграции с `rustok-media`. Ни `rustok-pages`, ни `rustok-page-builder` не зависят от `rustok-media`.
- Нет кропа, focal point, alt-текста как обязательного поля в UI и автогенерации `srcset`. `srcset` к тому же **запрещён** publish-политикой.

### 3.6 Текст

- Текстовые узлы содержат **только plain text**. `RichTextPayload` в модели непрозрачен, `@rustok/richtext` (ADR есть) не интегрирован.
- **Inline-редактирования текста в админ-канвасе нет.** `canvas_runtime.js` обрабатывает только select, hover, drag и geometry. Текст правится в поле панели свойств.
- Publish-политика запрещает `b`, `i`, `table`, `pre`, `code`, а любая разметка внутри текста даёт `landing_content_markup_forbidden`. Жирного, курсива, ссылки внутри абзаца, списков внутри текста нет.

### 3.7 Динамические данные

- Bindings на attribute, field и style с путями контекста.
- Conditions: `exists`, `equals`, `not_equals`, `truthy`, `falsy`, `contains`.
- Repeaters: лимит по умолчанию 100, максимум 1000.
- Context schema, scenario baselines (эталонные контексты для проверки перед публикацией), provider health binding.
- **Пробел:** нет каталога источников данных для маркетолога («товары коллекции X», «последние 3 поста блога»). Биндинги задаются путями вручную. Статический артефакт публикуется **один раз**, поэтому динамика «замораживается» на момент публикации либо требует отдельного runtime-пути.

### 3.8 Действия и формы

- Actions: `NavigatePage`, `NavigateUrl`, `SubmitForm`, `EmitEvent`, `ProviderAction`.
- **Серверного обработчика форм нет:** нет хранения заявок, email-уведомлений, webhook, anti-spam (honeypot/captcha), согласия на обработку ПД.
- CSP артефакта `form-action 'self'` не даёт отправить форму во внешний сервис. Шаблон `contact_form` — **витрина без функции**.

### 3.9 Публикация (static publish policy)

- **Fail-closed:** всё, что не разрешено явно, запрещено.
- **Лимиты:** 16 MiB, 128 Fly-страниц, 50 000 узлов, глубина 128.
- **Запрещено:** `iframe`/`embed`/`object` (а значит YouTube и карты), `srcset`, `table`, `pre`, `code`, `b`, `i`, inline-разметка в тексте, `url()` в стилях (см. M-3 предыдущего аудита).
- Политика правильная для безопасности, но сейчас **отрезает базовую выразительность**. Нужен allow-list «безопасного форматирования» (`strong`, `em`, `a`, `ul/ol/li`, `table`, `code`) и провайдерный embed-механизм с allow-list доменов (youtube-nocookie, vimeo, maps).

### 3.10 UI редактора (admin)

- Фиксированная трёхколоночная сетка без адаптива.
- **Около 27 панелей стеком** без табов и сворачивания: palette, layers, properties, styles, responsive styles, assets, pages, dynamic, bindings, context, translations, locale policy, actions, links, snapshot, diagnostics, scenario, provider health, capability, rollout, server preview и т.д.
- Палитра без поиска и превью-миниатюр.
- **Layers — плоский список**: нет drag в дереве, rename, hide, lock, collapse.
- В заголовке редактора показан **UUID страницы** вместо названия.
- Iframe-канвас: `srcdoc` пересчитывается `Memo` от всего документа (`isolated_canvas.rs:40`). Каждая мутация документа означает полную перезагрузку iframe: мигание, сброс скролла, повторный handshake. Это накладывается на C-3 (4 клона на команду), так что на больших документах редактор будет заметно тормозить.
- `canvas_runtime.js` использует `postMessage(…, '*')` с проверкой origin на принимающей стороне. Отправку с `'*'` лучше заменить на конкретный origin.
- README admin-крейта устарел относительно кода.

### 3.11 Каноничность «страниц»

Fly-проект поддерживает **несколько страниц внутри одного документа** (PageManager в редакторе). Storefront при этом рендерит только первую (`PageSelection::First`). У пользователя оказываются две сущности с именем «страница»: Pages-страница и Fly-страница внутри неё. Вторая на сайте фактически не видна.

### 3.12 Качество и CI (факты из GitHub Actions)

Workflow **`Fly Page Builder` красный на `main`** в 5 из 5 последних прогонов, последний — run `37622730793`, 7 часов назад. Внутри этого прогона:

| Шаг | Статус |
|---|---|
| Fly core tests, Fly UI/web runtime tests | ✅ |
| Page Builder admin unit, Pages domain unit, Pages integration unit | ✅ |
| Check Pages storefront / Page Builder storefront / admin SSR endpoint | ✅ |
| Около 20 verify-скриптов (SSR assets, links, forms, multilingual…) | ✅ |
| **Fly browser contract tests** | ❌ (exit 101) |
| **Check Fly adapter feature combinations** | ❌ (exit 101) |
| **Lint Fly browser and Page Builder integrations** | ❌ |
| **Verify editor capability policy** | ❌ |
| Focused formatting | ❌ |
| Dependency advisories (advisory only) | ❌ |

Состояние остальных workflow:
- `Pages Artifact Repair PostgreSQL Evidence` — ❌ на `main`.
- `Pages Artifact Rollback PostgreSQL Evidence` — ❌ в последнем прогоне на `main`, предыдущие ✅.
- `Page Builder Static Sanitization Evidence` — ❌ на `main`.
- `Pages Page Builder Provider Health` — ❌ на `main`.
- `Pages Page Builder Parity` — ✅.

**Вывод:**
- Ядро и unit-уровень в порядке; браузерный контракт, lint и capability-policy сломаны.
- Около 100 evidence-JSON и около 70 верификаторов в основном проверяют **наличие текста в исходниках**, а не поведение. Это зафиксировано и предыдущим аудитом, H-7.
- **E2E-теста «создать страницу → наполнить → опубликовать → открыть на сайте» нет.** Именно такой тест поймал бы B-1…B-5.

---

## 4. Дефекты (подробно)

### B-1 (P0). Админка видит только 20 страниц
- **Где:** `rustok-pages/admin/src/transport/graphql_adapter.rs:234`, `fetch_pages` с захардкоженными `page: 1, per_page: 20`.
- **Эффект:** 21-я и последующие страницы не отображаются в навигаторе. Открыть их можно только прямой ссылкой с UUID.
- **Исправление:**
  1. Пробросить `page`/`per_page` и `total` в UI.
  2. Добавить пагинацию или бесконечный скролл.
  3. Добавить серверный поиск: `ListPagesFilter.search` (ILIKE по title/slug) и `sort`.
- **Тест:** создать 25 страниц и убедиться, что все достижимы.

### B-2 (P0). Кириллический заголовок ломает создание
- **Где:** `admin/src/core.rs:6`, `slugify`, который отбрасывает всё не-ASCII. Сервер (`services/page/helpers.rs:74`) Unicode допускает.
- **Эффект:** «О компании» даёт `""` и ошибку валидации. Для русскоязычного продукта это блокер.
- **Исправление:** один источник правды. Вариант А — Unicode-slug как на сервере. Вариант Б — транслитерация (ru/uk/kk и др.) с опцией tenant-уровня. Вынести в общий крейт, использовать на клиенте и сервере, покрыть таблицей тест-кейсов.

### B-3 (P0). Новая страница не публикуется
- **Где:** `admin/src/core.rs` `default_project_data` создаёт text-компонент с content `"<h1>{title}</h1>"`. `fly/src/render.rs:321` экранирует его, а `static_publish_policy.rs:429` отклоняет `landing_content_markup_forbidden`.
- **Эффект:** автор видит на канвасе буквальные `<h1>…</h1>`, а Publish падает с непонятной ошибкой.
- **Исправление:** стартовый документ должен состоять из `heading` (level 1) с текстом `title`. Добавить тест «default project проходит static publish policy».

### B-4 (P0). Автор (manager) не может сохранить работу
- **Где:**
  - `page-builder/admin/src/editor/toolbar.rs:100` блокирует Save, если `!capabilities.publish`;
  - `rustok-pages/admin/src/builder.rs:401` (`page_builder_permissions_for_role`) выдаёт только `Read` и, если роль может публиковать, `Publish`, но **никогда** `Update`;
  - `access.rs`: у `manager` `publish: false`.
- **Причина:** builder-capability `Publish` означает «сохранить документ в store» (idempotency key `page-builder-save:{page}:{rev}`), а Pages-уровень трактует publish как «вывести на сайт».
- **Эффект (высокая вероятность):** manager редактирует на канвасе, но Save неактивна, а SSR-intent `save` будет отклонён capability-проверкой.
- **Исправление:** разделить понятия. Builder `Save` (сохранение черновика) должен требовать `pages:update`; `Publish` — `pages:publish` и только на Pages-уровне. Переименовать `PageBuilderCapabilityRequest::Publish` → `PersistDraft`/`Save`. Добавить тест матрицы ролей.

### B-5 (P0). Publish игнорирует несохранённые правки
- **Где:** обработчик publish в `composition.rs` берёт серверную ревизию; dirty-состояние редактора не проверяется.
- **Эффект:** автор правит, жмёт Publish — на сайт уходит предыдущая сохранённая версия, без предупреждения.
- **Исправление:** Publish выполняется как save-then-publish атомарно по ревизии. Если сохранение невозможно, publish блокируется с понятным сообщением.

### B-6 (P1). Publish требует promoted baseline
- **Где:** `graphql_adapter.rs:379`: «Publish requires a promoted Page Builder runtime scenario baseline».
- **Эффект:** для статической страницы без биндингов baseline бессмыслен, но обязателен. Маркетолог должен понять, что такое scenario panel.
- **Исправление:** требовать baseline только если документ содержит bindings, dynamic или provider actions. Иначе создавать baseline автоматически при публикации.

### B-7 (P1). Правка опубликованной страницы требует снятия с публикации
- **Где:** `services/page/document.rs:154`.
- **Эффект:** любая правка «живой» страницы даёт простой: 404 или пустой слот на сайте плюс сброс кэша и SEO-сигналов.
- **Исправление (архитектурное):** модель **draft-over-published**, как у WordPress, Strapi 5, Payload и Sanity:
  - `page_bodies` хранит рабочую (draft) версию;
  - опубликованная версия — это активный артефакт, он уже неизменяем и хранится отдельно;
  - Save больше не блокируется статусом, Publish = «скомпилировать текущий draft в новый артефакт и активировать».
  - Индикатор «есть неопубликованные изменения», diff и «отменить изменения» (reset draft to published).

  Инфраструктура артефактов уже это поддерживает, менять нужно в основном guard и UI.

### B-8 (P1). Нет защиты от потери данных
- Нет autosave (ни локального в IndexedDB/localStorage, ни серверного).
- Нет `beforeunload` при dirty-состоянии.
- Delete и Unpublish срабатывают в один клик (`composition.rs:~456–500`).
- **Исправление:** autosave черновика с debounce 2–5 с, guard навигации, confirm-диалоги; для Delete — ввод названия или soft delete.

### B-9 (P1). In-memory draft store
- **Где:** `browser_intent.rs:38` (`pages_browser_draft_store`), `draft_session.rs:70` (TTL 8 ч, 10 000 записей).
- **Эффект:** при рестарте или деплое no-JS черновики теряются. За load balancer'ом запрос на другую реплику не видит черновик. Ограничение по количеству есть, по байтам нет: 10 000 × 16 MiB в худшем случае.
- **Исправление:** хранить в Redis или БД (таблица `page_builder_draft_sessions` с TTL), добавить лимит по байтам на tenant/пользователя.

### B-10 (P1). Intents endpoint вне основного сервера
- **Где:** `apps/admin/src/main.rs:171`. В `apps/server` маршрута нет.
- **Эффект:** no-JS/SSR-режим редактора работает только при standalone-деплое `apps/admin`. Встроенная в сервер админка его не получит.
- **Исправление:** зарегистрировать роут через модульный реестр маршрутов `rustok-pages`, а не в приложении. Добавить smoke-тест на обоих хостах.

### B-11 (P1). Формы не работают
См. §3.8. **Исправление:**
- модуль `form submissions`: хранение заявок, уведомления, webhooks, rate limit, honeypot/captcha, экспорт CSV;
- endpoint `POST /api/pages/{id}/forms/{form_id}` под `form-action 'self'`;
- серверная валидация по схеме полей из Fly-документа.

### B-12 (P1). URL и встраивание на storefront
- Публичный URL с query-параметром под `/modules/pages`.
- На главной — только слот `home_before_footer` в демо-обёртке.
- Аноним видит iframe без auto-height.
- **Исправление:**
  1. Catch-all маршрут `/{locale}/{*path}` с резолвом через route publications/aliases (инфраструктура уже есть) и 301 со старых URL.
  2. Настройка «Homepage = page X» на уровне tenant/канала.
  3. Рендер артефакта **инлайн** (SSR-вставка санитизированного HTML со scoped CSS) вместо iframe. Либо, как минимум, auto-height через `postMessage` с проверкой origin.
  4. Убрать демо-обёртку.

### Прочие дефекты (P2)

| ID | Дефект | Где |
|---|---|---|
| B-13 | Дублирующийся префикс миграции `m20260806_000014` | `migrations/` |
| B-14 | Необратимая миграция без явной документации отката | `m20260721_000004` |
| B-15 | REST без unpublish, archive не выведен в API | `controllers/mod.rs`, `lifecycle.rs` |
| B-16 | Storefront рендерит только первую Fly-страницу (`PageSelection::First`), а редактор позволяет создавать несколько | storefront, editor pages panel |
| B-17 | Locale в форме — свободный текст (опечатка `ru_RU` против `ru` создаёт «новую» локаль) | create form |
| B-18 | Channels вводятся CSV-строкой, без выбора из существующих | create form |
| B-19 | 4 последовательных запроса при открытии страницы | `composition.rs` |
| B-20 | Захардкоженные английские строки в Pages admin | `composition.rs`, create form |
| B-21 | Заголовок редактора показывает UUID | editor header |
| B-22 | Полная перезагрузка iframe-канваса на каждое изменение документа | `isolated_canvas.rs:40` |
| B-23 | `postMessage(..., '*')` в canvas runtime | `canvas_runtime.js` |
| B-24 | Hard delete с `ON DELETE CASCADE` уничтожает журнал публикаций и артефакты (аудит-след) | `lifecycle.rs`, миграции |
| B-25 | Роли захардкожены строками; кастомные RBAC-роли получают read-only | `admin/src/access.rs` |
| B-26 | Inline-edit на storefront разрешён только для неопубликованных страниц, а это противоречит смыслу inline-edit | `services/page/inline_edit.rs` |

---

## 5. Функциональные пробелы (gap-лист по областям)

### 5.1 Управление контентом

| Возможность | Есть? | Комментарий |
|---|:-:|---|
| Список с поиском, фильтрами, сортировкой, пагинацией | ❌ | B-1 |
| Bulk-операции (publish, unpublish, delete, смена канала) | ❌ | |
| Дублирование страницы | ❌ | Базовая операция всех CMS |
| Иерархия (parent), вложенные URL, breadcrumbs | ❌ | |
| Homepage / 404-страница / служебные страницы | ❌ | |
| Шаблоны страниц (page templates) с выбором при создании | ⚠️ | `template` — строка `"default"`, без реестра |
| Сохранённые блоки / паттерны / глобальные секции (header, footer) | ❌ | Нет symbols/reusable/synced |
| Корзина и восстановление | ❌ | Hard delete |
| Автор, last editor, «кто сейчас редактирует» | ❌ | |

### 5.2 Жизненный цикл и governance

| Возможность | Есть? | Комментарий |
|---|:-:|---|
| Draft поверх published | ❌ | B-7 |
| История ревизий черновика + diff + restore | ❌ | Ревизия = updated_at; хранится только последняя |
| Rollback к произвольной опубликованной версии | ⚠️ | Только на один шаг назад |
| Отложенная публикация / снятие по расписанию | ❌ | |
| Review/approval workflow (submit for review → approve) | ⚠️ | «Reviewed publish» — это защита ревизии, а не воркфлоу согласования |
| Preview-ссылка для внешнего согласования (signed URL) | ❌ | |
| Комментарии на канвасе | ❌ | |
| Блокировка или presence при совместном редактировании | ❌ | Только конфликт ревизий |
| Аудит-лог действий | ⚠️ | Publish/rollback ops есть; удаляются каскадом |

### 5.3 Редактор

| Возможность | Есть? | Комментарий |
|---|:-:|---|
| Drag из палитры на канвас | ❌ | Вставка кнопкой |
| Inline-редактирование текста на канвасе | ❌ | |
| Rich text (bold, italic, link, list) | ❌ | Запрещено политикой |
| Дерево слоёв с DnD, rename, hide, lock | ❌ | Плоский список |
| Поиск в палитре, миниатюры блоков | ❌ | |
| Визуальные стилевые контролы | ❌ | Свойство + текстовое значение |
| Глобальные стили / дизайн-токены / тема | ❌ | |
| Hover/focus-состояния, анимации | ❌ | |
| Загрузка медиа / медиатека | ❌ | Только URL |
| Embed (YouTube, Maps) | ❌ | Запрещено политикой |
| Responsive-стили по брейкпоинтам | ✅ | Есть, UI сырой |
| Undo/redo, clipboard, горячие клавиши | ✅ | |
| Предпросмотр по устройствам | ✅ | 6 пресетов |
| Server preview | ✅ | |
| Режимы «Простой» / «Эксперт» (скрытие технических панелей) | ❌ | 27 панелей видны всем |
| Code view / import HTML | ❌ | |
| AI-ассистент (генерация секций и текстов) | ❌ | Рынок 2025–26 это ожидает |

### 5.4 Данные, формы, коммерция

| Возможность | Есть? | Комментарий |
|---|:-:|---|
| Bindings, conditions, repeaters (ядро) | ✅ | |
| Каталог источников данных для маркетолога | ❌ | |
| Блоки коммерции (товар, коллекция, корзина, цена) | ❌ | При том что RusTok — commerce-платформа |
| Формы с обработкой, хранением и уведомлениями | ❌ | B-11 |
| Персонализация / A/B-тесты | ❌ | |
| Аналитика по странице | ❌ | |

### 5.5 Доставка

| Возможность | Есть? | Комментарий |
|---|:-:|---|
| Неизменяемые артефакты, дайджесты, ETag, CSP | ✅ | Сильная сторона |
| Аудит, пересборка, активация артефактов | ✅ | Сильная сторона |
| Алиасы, tombstone, история URL | ✅ | Сильная сторона |
| Чистые URL | ❌ | B-12 |
| SSR-инлайн рендер без iframe | ❌ | B-12 |
| Headless-доставка JSON/HTML для Next-фронта | ⚠️ | API есть, `apps/next-frontend` страниц не рендерит |
| `srcset` / адаптивные изображения / lazy-loading | ❌ | `srcset` запрещён |
| JSON-LD, sitemap на уровне страницы | ❌ | |

---

## 6. Сравнение с платформами

Легенда: ✅ есть, зрелое · ⚠️ частично/ограниченно · ❌ нет · — неприменимо.
Оценки конкурентов — по публично известным возможностям продуктов на 2025–2026 гг. Платные редакции отмечены (Pro/EE).

### 6.1 Сводная матрица

| Возможность | **RusTok Pages+Fly** | WP Gutenberg | Elementor | Webflow | Builder.io | Strapi / Contentful / Sanity | Shopify OS 2.0 | Framer | Wix (Studio) | Payload + Puck | GrapesJS |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| Визуальный DnD-канвас | ⚠️ | ✅ | ✅ | ✅ | ✅ | ⚠️ (Contentful Studio, Sanity Presentation) | ⚠️ секции | ✅ | ✅ | ✅ (Puck) | ✅ |
| Inline-текст на канвасе | ❌ | ✅ | ✅ | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ | ✅ |
| Rich text | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ (Portable Text, Rich Text) | ✅ | ✅ | ✅ | ✅ (Lexical) | ✅ RTE |
| Библиотека блоков (кол-во) | 26 + 5 шаблонов | 90+ core + паттерны | 100+ (Pro) | обширная | свои компоненты | — модель | секции темы + app blocks | обширная + маркет | обширная | свои компоненты | базовые + плагины |
| Регистрация собственных компонентов разработчиком | ⚠️ registry в коде | ✅ блоки | ✅ widgets | ⚠️ DevLink/Code components | ✅ ключевая фича | ✅ компоненты/блоки | ✅ секции | ✅ code components | ⚠️ Velo | ✅ ключевая фича | ✅ |
| Reusable / synced блоки, глобальный header/footer | ❌ | ✅ synced patterns, template parts | ✅ global widgets, Theme Builder | ✅ Components + props | ✅ Symbols | ✅ references | ✅ section groups | ✅ components/variants | ✅ | ⚠️ | ✅ symbols |
| Глобальные стили / токены | ❌ | ✅ theme.json | ✅ global colors/fonts | ✅ Variables, classes | ⚠️ | — | ✅ theme settings | ✅ | ✅ | ⚠️ | ⚠️ |
| Responsive по брейкпоинтам | ✅ (сырой UI) | ⚠️ | ✅ | ✅ | ✅ | — | ⚠️ | ✅ | ✅ | ⚠️ | ✅ |
| Медиатека / загрузка | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ asset manager |
| Embed (видео, карты) | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Формы с обработкой | ❌ | ⚠️ плагины | ✅ (Pro) | ✅ | ⚠️ | ❌ | ✅ (contact) | ✅ | ✅ | ✅ form builder plugin | ⚠️ плагин |
| Динамика (bindings, loops) | ⚠️ ядро | ✅ Query Loop, Block Bindings | ✅ dynamic tags, Loop Grid (Pro) | ✅ CMS collections | ✅ data bindings | ✅ (модель) | ✅ dynamic sources/metafields | ✅ CMS | ✅ datasets | ✅ | ⚠️ |
| Draft поверх published | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ (Strapi 5 D&P) | ✅ (unpublished theme) | ✅ | ✅ | ✅ versions/drafts | — |
| Autosave | ❌ | ✅ | ✅ | ✅ | ✅ | ⚠️/✅ | ✅ | ✅ | ✅ | ✅ | ✅ storage manager |
| История + restore | ⚠️ rollback на 1 шаг | ✅ revisions + compare | ✅ | ✅ backups | ✅ | ✅ (Sanity, Contentful; Strapi EE) | ⚠️ темы | ✅ | ✅ | ✅ | ⚠️ undo |
| Отложенная публикация | ❌ | ✅ | ✅ | ✅ (CMS items) | ✅ | ✅ Releases | ⚠️ apps | ⚠️ | ⚠️ | ✅ | — |
| Review/approval | ⚠️ ревизия-гард | ⚠️ плагины | ❌ | ⚠️ Enterprise | ✅ | ✅ (EE / workflows) | ❌ | ❌ | ⚠️ | ✅ access-hooks | — |
| Preview-ссылка | ❌ | ✅ | ✅ | ✅ staging | ✅ | ✅ live preview | ✅ preview theme | ✅ | ✅ | ✅ live preview | — |
| Совместное редактирование в реальном времени | ❌ | ⚠️ (в разработке) | ❌ | ⚠️ page branching | ⚠️ | ✅ Sanity | ❌ | ✅ | ⚠️ | ❌ | ❌ |
| Иерархия страниц, чистые URL | ❌ | ✅ | ✅ | ✅ folders | ✅ | ⚠️ (фронтом) | ⚠️ /pages/x | ✅ | ✅ | ✅ (nested-docs) | — |
| Redirects / история URL | ✅ сильная | ⚠️ плагины | ⚠️ Pro | ✅ | ✅ | ⚠️ | ✅ | ✅ | ✅ | ✅ плагин | — |
| Локализация тела страницы | ⚠️ (две модели, без UI) | ⚠️ WPML/Polylang | ⚠️ WPML | ✅ Localization | ✅ | ✅ | ✅ Markets + Translate&Adapt | ✅ | ✅ | ✅ field-level | ⚠️ |
| SEO (meta, canonical, robots, og) | ✅ | ✅ (+Yoast) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ SEO plugin | ⚠️ |
| Неизменяемые артефакты с дайджестом | ✅ уникально | ❌ | ❌ | ⚠️ (immutable deploys) | ⚠️ | ❌ | ⚠️ versions | ⚠️ | ❌ | ❌ | ❌ |
| Fail-closed санитизация HTML | ✅ сильная | ⚠️ kses | ⚠️ | — closed-platform | ⚠️ | ✅ (структурный контент) | ✅ Liquid | — | — | ✅ (структура) | ❌ (raw HTML) |
| SSR/no-JS режим редактора | ✅ уникально | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Rollout-фича-флаги на capability | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Коммерческие блоки | ❌ | ⚠️ WooCommerce blocks | ✅ WooCommerce widgets | ✅ Ecommerce | ✅ | — | ✅ нативно | ⚠️ | ✅ | ⚠️ | — |
| A/B, персонализация | ❌ | ❌ | ❌ | ✅ (Optimize) | ✅ сильная | ⚠️ (Ninetailed) | ⚠️ apps | ⚠️ | ⚠️ | ❌ | ❌ |
| AI-генерация | ❌ | ⚠️ | ✅ Elementor AI | ✅ | ✅ Visual Copilot | ✅ (AI Assist) | ✅ Sidekick | ✅ | ✅ | ⚠️ | ❌ |
| Self-hosted / open source | ✅ | ✅ | ✅ | ❌ | ⚠️ SDK open | Strapi ✅ / ❌ / Studio ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |

### 6.2 Что взять у каждого (конкретно для RusTok)

**WordPress Gutenberg**
- *Synced patterns + template parts*: глобальные header и footer, переиспользуемые секции, «отсоединить копию».
- *Revisions с compare*: diff двух ревизий бок о бок; restore одной кнопкой.
- *Block Bindings API*: блок связывается с полем источника декларативно, как Fly bindings, только с UI выбора источника.
- *Статус «Scheduled»* и предпросмотр без публикации.
- `theme.json` как модель дизайн-токенов (палитра, типографика, spacing): почти один в один ложится на Fly style catalog.

**Elementor**
- *Theme Builder с display conditions*: шаблон для «всех страниц X» / «товаров категории Y».
- *Глобальные цвета и шрифты* в пикере каждого стилевого контрола.
- *Form widget с actions after submit*: email, webhook, CRM, redirect. Эталон для B-11.
- *Navigator* (дерево слоёв) с DnD, rename, hide.

**Webflow**
- *Class-based styling + Variables*: стили как переиспользуемые классы, а не инлайн на каждом узле. Fly `StyleRuleCatalog` к этому близок, не хватает UI классов.
- *Components с props*: параметризуемые символы.
- *CMS Collection templates*: одна страница-шаблон на тип записи. Для RusTok это шаблон для блога, товара, категории.
- *Staging-домен + publish*, page branching.
- *Localization*: дерево локалей, «перевести эту страницу», статус перевода по локалям, локализованные slug.

**Builder.io**
- *Регистрация собственных компонентов* с типизированными inputs. Fly registry уже ближе всего именно к этой модели, нужен публичный контракт для модулей: `rustok-commerce` регистрирует `ProductCard` с полями.
- *Targeting и A/B на уровне контента* без деплоя.
- *Content API / SDK* для Next: прямой аналог недостающего рендера в `apps/next-frontend`.

**Strapi 5 / Contentful / Sanity**
- *Strapi 5 Draft & Publish*: отдельные draft- и published-версии документа (ответ на B-7). *Releases* для групповой публикации по расписанию. *Review workflows* (EE).
- *Contentful Environments + Releases + Scheduled actions*; *Live Preview*.
- *Sanity*: real-time collaboration, история каждого изменения, Presentation tool (click-to-edit из превью), *Content Releases*. Portable Text — эталон хранения rich text как структуры, без HTML. Он идеально совместим с fail-closed политикой RusTok и даёт bold/italic/links без HTML в тексте.

**Shopify OS 2.0**
- *JSON-шаблоны + sections everywhere + blocks*, схема настроек секции объявляется разработчиком (`{% schema %}`): маркетолог видит только **осмысленные поля** («Заголовок», «Кнопка», «Картинка»), а не CSS-свойства. Для commerce-платформы это самая близкая и правильная модель.
- *Section groups* для header и footer.
- *Theme versions*: работа в непубликованной копии с превью-ссылкой и публикацией одной кнопкой.
- *App blocks*: сторонние модули добавляют блоки в тему. Аналог: модули RusTok регистрируют Fly-компоненты.

**Framer**
- Плавный канвас (без перезагрузки iframe), breakpoints с наследованием, variants у компонентов.
- Встроенные формы, CMS, локализация с AI-переводом, real-time collaboration.
- Публикация в staging, откат к любой версии.

**Wix / Wix Studio**
- Site History с restore любой версии сайта.
- Мастер SEO-настроек и онбординг «с нуля до опубликованной страницы».

**Payload CMS + Puck**
- *Payload*: `versions: { drafts: { autosave: true } }`, scheduled publish, live preview, field-level localization, access-control функции. По архитектуре ближе всего к RusTok (код-first, self-hosted).
- *Puck*: открытый React-редактор, конфиг компонентов `fields` + `render`, JSON-документ, плагины. Минимальный UX-эталон, который стоит догнать: палитра → DnD → поля справа → outline-дерево.

**GrapesJS** (из его модели Fly и вырос)
- Готовые менеджеры: *Asset Manager* с upload, *Layer Manager* с DnD/visibility, *Style Manager* с секторами и визуальными контролами, *Trait Manager*, встроенный *RTE*, *Storage Manager* с autosave, *Pages*, *Symbols*.
- RusTok унаследовал формат проекта, но **не унаследовал UX-менеджеры**. Это главный провал паритета с собственным предком.

### 6.3 Позиционирование

- **В чём RusTok впереди всех:**
  - доказуемая целостность публикации (дайджест-артефакты, аудит и пересборка);
  - fail-closed санитизация;
  - SSR/no-JS-редактор;
  - capability rollout-профили;
  - история URL с tombstone «из коробки»;
  - мультитенантность.

  Это востребовано в регулируемых и enterprise-сценариях: банки, госсектор, маркетплейсы с UGC.
- **В чём отстаёт от всех:** авторский опыт (rich text, inline, медиа, embed, формы, глобальные стили, reusable-блоки) и контентный жизненный цикл (draft-over-published, autosave, история, расписание, превью-ссылки).
- **Рекомендуемая целевая модель:** *«Shopify OS 2.0 sections + Payload drafts/versions + Sanity Portable Text, поверх существующего артефактного конвейера»*. Маркетолог собирает страницу из схемных секций, которые объявляют модули (hero, товарная сетка, FAQ, форма), и правит смысловые поля. Технические панели (bindings, scenario, rollout, provider health) уходят в «режим эксперта».

---

## 7. Сильные стороны, которые нужно сохранить

1. **Артефактный конвейер:** неизменяемость, SHA-256, аудит → rebuild → activate, reviewed publish с idempotency.
2. **Fail-closed static publish policy:** расширять allow-list, но не ослаблять принцип.
3. **Route history:** алиасы, tombstone, импорт истории.
4. **Outbox-инвалидация кэша поколениями.**
5. **Optimistic concurrency** по ревизии с понятным кодом `REVISION_CONFLICT`.
6. **Capability-модель редактора** с деградацией по здоровью провайдеров и rollout-профилями.
7. **HMAC-гранты inline-редактирования** с constant-time проверкой.
8. **Lossless GrapesJS-совместимость** документа: миграционный путь с GrapesJS.
9. **Лимиты документа** (16 MiB / 50k узлов / глубина 128) как защита от DoS.

---

## 8. План работ (P0–P3)

### P0 — блокеры использования (1–2 недели)

| # | Задача | Связь | Оценка |
|---|---|---|---|
| P0-1 | Пагинация и серверный поиск в списке страниц (`ListPagesFilter.search/sort`, UI) | B-1 | 2–3 д |
| P0-2 | Единый slugify (Unicode или транслитерация) для клиента и сервера + тест-таблица | B-2 | 1 д |
| P0-3 | Стартовый документ из `heading` + тест «default project проходит publish policy» | B-3 | 0.5 д |
| P0-4 | Разделить capability Save (`pages:update`) и Publish (`pages:publish`); переименовать `PageBuilderCapabilityRequest::Publish`; тест матрицы ролей | B-4 | 2 д |
| P0-5 | Publish = save-then-publish по ревизии; блок при dirty | B-5 | 1 д |
| P0-6 | Починить CI `Fly Page Builder` на `main`: browser contract tests, feature combos, lint, capability policy, fmt | §3.12 | 2–3 д |
| P0-7 | **Один E2E-тест** (Playwright, конфиги уже есть): создать страницу с кириллическим заголовком → добавить hero → сохранить под manager → опубликовать под admin → открыть на storefront → проверить контент | — | 2 д |

### P1 — базовый продуктовый минимум (3–6 недель)

| # | Задача | Связь |
|---|---|---|
| P1-1 | **Draft-over-published**: снять immutable-guard с `page_bodies`, опубликованная версия = активный артефакт; индикатор «есть неопубликованные изменения», «сбросить к опубликованному» | B-7 |
| P1-2 | Baseline только для документов с динамикой; иначе автосоздание | B-6 |
| P1-3 | Autosave (debounce) + `beforeunload` + confirm-диалоги; soft delete с корзиной | B-8, B-24 |
| P1-4 | Draft store в БД/Redis с лимитом по байтам | B-9 |
| P1-5 | Регистрация intents-роута через модульный реестр (работает в `apps/server`) | B-10 |
| P1-6 | **Чистые URL** `/{locale}/{*path}` через route publications + 301 со старых; назначение homepage | B-12 |
| P1-7 | SSR-инлайн рендер артефакта (scoped CSS) вместо iframe для анонимов; либо auto-height | B-12 |
| P1-8 | **Rich text** на базе структурного формата (Portable-Text-подобный, `@rustok/richtext`) с allow-list `strong/em/a/ul/ol/li/br`; политика разрешает их как структуру, не как HTML | §3.6 |
| P1-9 | **Медиа:** интеграция с `rustok-media` (upload, медиатека, alt обязательный, автогенерация `srcset` на сервере, разрешение `srcset` в политике для своих URL) | §3.5 |
| P1-10 | **Формы:** submissions-хранилище, email/webhook, honeypot + rate-limit, экспорт | B-11 |
| P1-11 | Inline-редактирование текста на канвасе (contenteditable для text/heading/button через `canvas_runtime.js`) | §3.6 |
| P1-12 | Инкрементальное обновление канваса (патчи DOM по `postMessage`) вместо перезагрузки `srcdoc`; закрыть C-3 из предыдущего аудита | B-22 |
| P1-13 | Переключатель локали в редакторе; «создать перевод из локали X»; объявить **одну** каноническую модель i18n тела (рекомендация: документ на локаль + `flyTranslations` только для общих строк) | §2.5 |

### P2 — паритет с рынком (1–3 месяца)

| # | Задача |
|---|---|
| P2-1 | **Режимы редактора «Контент» / «Дизайн» / «Эксперт»**: по умолчанию 5–6 панелей (палитра, слои, свойства, стили, страница, SEO), остальные в «Эксперт»; табы и сворачивание |
| P2-2 | Дерево слоёв: DnD, rename, hide, lock, collapse; DnD из палитры на канвас; поиск и миниатюры в палитре |
| P2-3 | Визуальные стилевые контролы: color picker, box-model, typography, слайдеры; hover/focus-состояния |
| P2-4 | **Дизайн-токены / глобальные стили** (палитра, шрифты, spacing) на уровне tenant-темы → CSS variables в артефакте |
| P2-5 | **Reusable-секции / symbols**, глобальные header и footer (synced) |
| P2-6 | **Схемные секции от модулей** (модель Shopify): модуль регистрирует компонент с типизированными полями; маркетолог правит поля, а не CSS. Первые: `ProductCard`, `ProductGrid`, `CollectionList`, `BlogPostList`, `FAQ`, `Newsletter` |
| P2-7 | Embed-провайдеры с allow-list доменов (YouTube-nocookie, Vimeo, Maps) через отдельный sandbox-iframe и CSP `frame-src` |
| P2-8 | Расширение библиотеки: table, blockquote, code, tabs, accordion, gallery/slider, icon, social links, navbar |
| P2-9 | История ревизий черновика (таблица `page_body_revisions`), diff, restore; rollback к любой опубликованной версии |
| P2-10 | Отложенная публикация и снятие (scheduler в существующем outbox/worker) |
| P2-11 | Preview-ссылки (signed, с TTL) для согласования |
| P2-12 | Дублирование страницы, bulk-операции, иерархия (parent_id) + breadcrumbs |
| P2-13 | Pages в `apps/next-frontend`: SSR-рендер артефакта/JSON через публичный API (паритет хостов) |
| P2-14 | JSON-LD (WebPage, BreadcrumbList, FAQPage), флаг sitemap, SEO-превью сниппета |
| P2-15 | Роли из RBAC-модуля вместо захардкоженных строк |
| P2-16 | Почистить миграции (дубликат префикса), документировать необратимую |

### P3 — дифференциация (квартал+)

| # | Задача |
|---|---|
| P3-1 | Review/approval workflow (submit → approve → publish) поверх reviewed publish; комментарии на канвасе |
| P3-2 | Presence и мягкие блокировки; затем real-time collaboration (CRDT/OT поверх `EditorCommand`, команды уже сериализуемы) |
| P3-3 | A/B-варианты секций и таргетинг по сегменту/каналу (channel visibility — уже основа) |
| P3-4 | AI-ассистент: генерация секции по промпту в рамках схем компонентов (политика гарантирует безопасность вывода), AI-перевод тела страницы |
| P3-5 | Аналитика по странице (просмотры, конверсия форм) |
| P3-6 | Шаблоны страниц-коллекций (шаблон товара, категории, поста), аналог Webflow CMS templates |
| P3-7 | Решение по `fly-dioxus`: довести или убрать из workspace (см. H-1 предыдущего аудита) |
| P3-8 | Пересмотр процесса evidence/verify: оставить поведенческие тесты, сократить текстовые grep-верификаторы (около 100 JSON + около 70 скриптов) |

### Последовательность

```
Нед. 1–2   P0-1..P0-7   (CI зелёный, E2E-тест, базовые баги)        ← без этого всё остальное непроверяемо
Нед. 3–8   P1-1, P1-3, P1-6, P1-8, P1-9, P1-10  (жизненный цикл, URL, rich text, медиа, формы)
           P1-2, P1-4, P1-5, P1-7, P1-11..13   (параллельно)
Мес. 3–5   P2 (UX-режимы → токены → секции модулей → история/расписание)
Квартал+   P3
```

---

## 9. Что обязательно подтвердить прогоном

1. **B-4:** войти под `manager`, открыть редактор и проверить, что Save неактивна; отправить SSR-intent `save` и проверить, что получен отказ capability.
2. **B-3:** создать страницу, открыть канвас (буквальные `<h1>`?) и нажать Publish (`landing_content_markup_forbidden`?).
3. **B-6:** опубликовать статическую страницу без baseline.
4. **B-1:** 25 страниц в tenant'е, видимость в навигаторе.
5. **B-22:** профиль перерисовки канваса при вводе в панели свойств на документе около 2 000 узлов.
6. Причины падения шагов CI `Fly browser contract tests`, `Check Fly adapter feature combinations`, `Verify editor capability policy`. Логи job'ов из песочницы недоступны: blob-хранилище Actions заблокировано.
7. Поведение поисковых краулеров на странице, отданной через iframe (Search Console, URL Inspection).
