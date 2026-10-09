# Страницы и Билдер — глубокий инженерно-функциональный аудит

**Дата:** 2026-10-09
**Ветка:** `arena/b20c5c92-rustok` (база `2573727` / `main`)
**Область аудита:** `crates/ui/fly/**` (движок билдера), `crates/modules/rustok-page-builder` (+ `admin`, `storefront`), `crates/modules/rustok-pages` (+ `admin`, `storefront`), а также смежные модули: `rustok-seo`/`rustok-seo-targets`, `rustok-translation-targets`, `rustok-channel`, `rustok-cache`, `rustok-search`, `rustok-navigation`, `rustok-media`, `rustok-outbox`, `rustok-rbac`, `rustok-tenant`, `apps/admin`, `apps/storefront`, `apps/next-admin`.
**Компаньон:** `docs/audits/page-subsystem-engineering-audit-2026-10-08.md` (инженерный/CI-аудит), `docs/audits/fly-builder-engineering-audit-2026-10-02.md` (fly-аудит). Настоящий документ — функциональный аудит: «что система умеет, чего не умеет, как это выглядит на фоне рынка». Инженерные дефекты CI/инструментов здесь не пересказываются.

**Ограничения среды:** в окружении нет Rust toolchain (cargo/rustc отсутствуют), поэтому выводы сделаны статическим чтением исходников; компиляция и запуск тестов не подтверждаются. Все суждения ниже опираются на фактический код, а не на заявления README.

**Статус (2026-10-09):** Волна 1 реализована в ветке `arena/b20c5c92-rustok`: G-1 (черновики при живой опубликованной странице), G-2 (append-only история версий тела с restore), G-4 (дублирование страниц). Решения и границы — `DECISIONS/2026-10-09-page-body-draft-and-revision-history.md`; итоговое поведение — README `rustok-pages` (Known Limitations). Волна 2: G-3 (расписание публикации) реализован — `DECISIONS/2026-10-09-scheduled-page-publishing.md`; G-7 (бэкенд форм) реализован — модуль `rustok-forms`, `DECISIONS/2026-10-09-form-submissions-module.md`; G-6 (медиатека/`AssetProvider`) в процессе. Текст аудита ниже зафиксирован на момент проведения и не переписывается.

---

## 1. Резюме для руководства

Подсистема «Страницы + Билдер» — это **зрелое ядро нетипичной архитектурной силы** с заметными **продуктовыми пробелами типичного «второго CMS-поколения»**.

Сильная сторона — то, чего нет почти ни у одного конкурента в таком объёме:
- **неизменяемые опубликованные артефакты** с SHA-256 провенансом, идемпотентные чеки операций, откат к предыдущему набору артефактов, аудит целостности и ремонт связок;
- **fail-closed санитизация** публикуемой разметки (allowlist тегов, запрет `on*`/`script`/`iframe`, политика URL, контроль CSS `url()`), точный CSP с hash-для CSS;
- **runtime-сценарии** с release-baseline (SHA-256 CAS), снапшоты рендера, режимы `BlockBroken`/`RequireStable` — фактически «детерминированный runtime-контракт публикации»;
- **типизированные контракты контекста**, data-binding, условия и репитеры с preflight-политиками;
- многоарендность, каналы (web/mobile), RBAC `pages:*`, deadline/idempotency политики портов.

Слабая сторона — продуктовый слой, где пользователь сравнивает с Webflow/Wix/Tilda:
- **нельзя редактировать черновик при живой версии** (опубликованный документ иммутабелен, нужен unpublish);
- **нет истории версий документа** (одна строка `page_bodies` перезаписывается);
- **нет расписания публикации, дублирования страниц, глобальных символов, медиатеки, бэкенда форм, встраивания видео/iframe, анимаций, шаблонов/мастер-страниц, дизайн-токенов, совместного редактирования**;
- страницы **не попадают в собственный поиск** (`rustok-search` проецирует forum/blog/product, но не pages) и **не связаны с навигацией**;
- Next.js-админка не имеет раздела Pages/Builder (только Leptos-админка).

Итог: как **платформенный движок публикации** RusTok опережает рынок по строгости и целостности; как **редактор для маркетолога** он отстаёт по функциям «дня» (черновики, версии, медиа, символы, формы, расписание). Ниже — полный инвентарь и матрица сравнения.

---

## 2. Архитектура и связи модулей (как это устроено)

```text
                    ┌───────────────────────────────────────────────┐
                    │  crates/ui/fly  (37k LOC)                      │
                    │  AST: ProjectDocument/GrapesProject/Component │
                    │  Lossless GrapesJS codec, commands, history   │
                    │  registries, style rules, snapshots (sha256)  │
                    │  dynamic: conditions/repeaters/bindings       │
                    │  actions/forms, translations, locale policy   │
                    │  runtime scenarios (baseline + render snaps)  │
                    │  SSR render, validation, landing readiness    │
                    └──────────────┬────────────────────────────────┘
                                   │
        ┌──────────────────────────┼───────────────────────────────┐
        │                          │                               │
┌───────▼──────────────┐  ┌────────▼─────────────────┐  ┌──────────▼──────────┐
│ fly-ui / fly-web /   │  │ rustok-page-builder      │  │ rustok-page-builder │
│ fly-browser          │  │ (28k LOC)                │  │ -storefront         │
│ state machine,       │  │ capability service:      │  │ inline edit (WASM)  │
│ capabilities,        │  │ Preview/Tree/Properties/ │  │ localized routes    │
│ keymaps, geometry,   │  │ Publish, rollout guards, │  └─────────────────────┘
│ iframe bridge,       │  │ authz, port policies,    │
│ JS bridge            │  │ static publish policy    │
└──────────────────────┘  │ (sanitizer, limits),     │
                          │ materialization, health  │
                          └────────┬─────────────────┘
                                   │ capability envelopes / ports
        ┌──────────────────────────┼──────────────────────────────┐
        │                          │                              │
┌───────▼─────────────┐   ┌────────▼────────────────┐   ┌─────────▼────────────┐
│ rustok-page-builder │   │ rustok-pages (47k LOC)  │   │ rustok-pages-admin   │
│ -admin (18k LOC)    │   │ pages, translations,    │   │ (Leptos): builder,   │
│ Leptos редактор:    │   │ bodies, channel visib., │   │ metadata, rollback,  │
│ canvas, palette,    │   │ route aliases/redirects,│   │ inline edit launch   │
│ layers, props,      │   │ scenario baselines +    │   └──────────────────────┘
│ styles, responsive, │   │ revision journal,       │
│ assets, pages,      │   │ immutable artifacts,    │
│ translations,       │   │ publish/rollback/rebuild│
│ inspector, publish  │   │ receipts, SEO target,   │
│ scenarios, a11y     │   │ cache invalidation      │
└─────────────────────┘   └───────┬─────────────────┘
                                  │
   ┌──────────┬───────────┬───────┼──────────┬─────────────┬──────────────┐
   │          │           │       │          │             │              │
┌──▼───┐ ┌────▼────┐ ┌────▼───┐ ┌─▼──────┐ ┌─▼──────────┐ ┌▼───────────┐ ┌▼──────────┐
│seo/  │ │translat.│ │channel │ │cache   │ │outbox      │ │search      │ │navigation │
│seo-  │ │targets  │ │        │ │(gen-   │ │(события +  │ │(❌ нет     │ │(❌ нет    │
│targets│ │(только │ │(гейт   │ │based   │ │идемпотентн.)│ │проектора   │ │ссылок на  │
│(sitemap│ │метадан.)│ │модулей)│ │инвалид.)│ │           │ │pages)      │ │pages)     │
└──────┘ └─────────┘ └────────┘ └────────┘ └───────────┘ └────────────┘ └───────────┘
```

**Ключевое правило ownership** (подтверждено кодом и README модулей):
- `fly` владеет моделью документа и рендером; GrapesJS — только формат импорта/экспорта (никогда не вторая доменная модель);
- `rustok-page-builder` владеет capability-контрактом (v1.1), rollout, авторизацией, санитизацией и материализацией;
- `rustok-pages` владеет персистентностью, жизненным циклом и публикацией (consumer-документы: Pages, а также Blog/Forum используют билдер как capability);
- UI-адаптеры (Leptos SSR + WASM-острова) только отображают одни и те же контракты.

### 2.1 Поверхности API
- **REST** (`rustok-pages/controllers`): `GET /api/pages`, `GET /api/pages/{id}/artifact`, `POST /api/admin/pages`, `DELETE /api/admin/pages/{id}`, `PATCH .../metadata`, `PUT .../document`.
- **GraphQL**: `page`, `pageBySlug`, `pages`, `createPage`, `patchPageMetadata`, `savePageDocument`, `publishPage`, `rollbackPage`, `unpublishPage`, `deletePage`, + `pageBuilderScenarioBaseline`, `pageBuilderScenarioBaselineHistory`, `pageBuilderScenarioReleaseStatus`, `save/deletePageBuilderScenarioBaseline`, `pageBuilderRolloutSnapshot`, `pageBuilderCapabilityPreflight`, а также `artifactIntegrityAudit`, `artifactRepair`.
- **Leptos server-functions** (storefront/admin): route-decision, inline-edit bootstrap/commit, capability-эндпоинты билдера.

⚠️ Несогласованность: publish/rollback/unpublish/rebuild существуют **только в GraphQL**, REST-контур ограничен CRUD метаданных/документа. Для внешних интеграций (CI/CD, маркетинг без GraphQL) это дыра.

### 2.2 Схема данных (ключевые таблицы `rustok-pages`)
`pages`, `page_translations`, `page_bodies` (одна строка на page+locale, канонический формат `grapesjs`), `page_channel_visibility`, `page_route_alias` + `page_route_publication` (история canonical/redirect/gone), `page_builder_scenario_baseline` (+ `_revision` журнал), `page_static_landing_artifact`, `page_published_landing_artifact` (binding), `page_publish_operation` (+ `_artifact`, `_rebuild_source`), `page_rollback_operation`, `page_artifact_rebuild_operation`, `page_artifact_binding_replacement_operation`, `page_route_history_import`, `translation_change`.

⚠️ Несоответствие: платформенный README декларирует «composite PK `(tenant_id, id)`», но в миграциях pages (`m20260328_000001`) PK — одиночный `id` (UUID), изоляция обеспечивается **фильтрами `tenant_id` в каждом запросе**. Прошлый аудит (2026-10-08) уже находил реальные пропуски именно из-за этого. Для домена «разделы» это самая рискованная граница.

---

## 3. Полный функциональный инвентарь

### 3.1 Модель документа (fly-core)

| Сущность | Что есть | Детали |
|---|---|---|
| `ProjectDocument` / `GrapesProject` | ✅ | assets, styles, pages + flatten-extensions; lossless round-trip GrapesJS |
| `ComponentNode/Object` | ✅ | id/type/tag/provider/attributes/style/traits/children + extensions; `Opaque`-узлы сохраняются нетронутыми |
| Мультистраничность внутри проекта | ✅ | `PageLocator`, `PageCommand` (Add/Remove/Move/Patch), last-page removal запрещён, дубли id страниц запрещены |
| Стабильные id компонентов | ✅ | `ensure_stable_ids` самоизлечивает битые/дублированные id (валидация идентификаторов, чтобы не ломать CSS-селекторы) |
| `StyleRuleCatalog` | ✅ | scopes `Base` и `Media{query}`; правила привязаны к `flyComponentId`; при удалении страницы — чистка правил |
| Регистры | ✅ | components, blocks, traits, styles, selectors, asset_providers, commands, plugins (+ валидация зависимостей и циклов плагинов) |
| Встроенный каталог | ✅ | ~30 типов компонентов (wrapper/section/row/column/grid/text/heading/link/image/video/media/list/select/form/inputs/button/quote/...), блоки + landing-шаблоны (hero, features, ...); категория «advanced» содержит `raw_html` |
| Команды/история | ✅ | `EditorCommand` (Insert/Remove/Move/Patch/Asset/StyleRule/Page/Dynamic/Binding/Context/Translation/RestoreSnapshot/Batch); undo/redo deque с бюджетом байт (по умолчанию 100 записей) |
| Проект-снапшоты | ✅ | `SnapshotCatalog`, `restore_verified`, `compare_projects` (diff), `ContentDigest` (SHA-256) |
| Фрагменты | ✅ | `ProjectFragment` — copy/paste поддерева со стилями, ассетами и `provider_requirements` (перенос требований провайдеров) |

### 3.2 Динамический runtime (data-binding)

| Возможность | Статус | Детали |
|---|---|---|
| Биндинги `flyRuntimeBindings` | ✅ | targets: Attribute/Field/Style; трансформы: string/number/boolean/upper/lower/trim/json; fallback-значения |
| Условия `flyRuntimeConditions` | ✅ | operators exists/equals/not_equals/truthy/falsy/contains + invert |
| Репитеры `flyRuntimeRepeaters` | ✅ | item/index aliases, limit (default 100 / max 1000), `empty_behavior` hide/keep |
| Контекст-контракты | ✅ | `RuntimeContextContract` (JSON-schema поля), preflight-политики, зависимости контекста (`context_dependency`), гейты в редакторе (панели ContextSchema/ContextDependency) |
| Сценарии runtime | ✅ | `scenario_id` (≤128 байт), context ≤256 KiB, `RuntimeScenarioRenderSnapshot` на страницу, release baseline (SHA-256 CAS `expected_baseline_hash`), режимы Disabled/BlockBroken/RequireStable |
| Материализация | ✅ | `materialize_runtime` (условия → скрытия, репитеры → размножение, биндинги → значения), не-детерминизм исключён |

Это один из самых сильных слоёв: типизированный data-binding уровня Builder.io/Plasmic, но с контрактами и провенансом.

### 3.3 Действия и формы

| Возможность | Статус | Детали |
|---|---|---|
| `ComponentAction` | ✅ | NavigatePage (page_id/base/query/fragment/fallback_href), NavigateUrl, SubmitForm, EmitEvent, ProviderAction |
| Нативные формы | ✅ | `ComponentForm`: method get/post/dialog, enctype, action_url/provider/action/input; materialize генерирует `<form>` атрибуты |
| Browser bridge (fly-browser.js) | ✅ | submit → payload (checkbox/number нормализация), интенты, abort/timeout классификация, pending intents, response ordering, проблем-статусы |
| Хранилище отправок форм | ❌ | Нет модуля form-submissions/leads; `ProviderAction` не имеет серверного потребителя |
| Уведомления о заявках | ❌ | — |
| Антиспам (captcha/honeypot) | ❌ | `novalidate` есть, валидации/капчи нет |

### 3.4 Редактор (admin: Leptos SSR + WASM)

`rustok-page-builder/admin` — фактически весь UX авторинга (~18k LOC):

- **AdminShell / AdminCanvas** (modular_canvas, isolated_canvas): canvas в изоляции (iframe/srcdoc), resize handles, hit-testing, drag&drop (ssr_drop), выделение/копирование/буфер, shortcut dispatch, selection commands;
- **Palette/Layers** (`palette_layers`), **свойства** (`properties_section`, `property_helpers`, `trait_panel`), **стили** (`style_section`, `responsive_styles` — 3 брейкпоинта: laptop/tablet/mobile с media-queries и пресетами вьюпортов);
- **Страницы** (`page_manager`): список, создание, переименование, метаданные, порядок;
- **Локализация**: `ssr_locale`, `ssr_locale_policy`, `ssr_locale_coverage`, `ssr_translations`, `ssr_localized_metadata`, внутренние ссылки (`ssr_internal_link`);
- **Ассеты** (`asset_section`): ручной ввод id+URL, upsert/remove/select; `AssetCatalog` с дедупликацией id;
- **Actions/Forms** (`ssr_actions_forms`, `ssr_forms` — полный набор форм редактора: компонент, форма, translation, asset и т.д.);
- **Inspector/diagnostics/audit** (`ssr_inspector`, `diagnostics_section`, `audit_panel`);
- **Runtime-панели**: `dynamic_runtime` (условия/репитеры), `binding_panel`, `context_schema_panel`, `context_dependency_panel`, `context_contract_tools`, `runtime_scenarios` + `runtime_scenario_matrix`/`regression`, `publish_scenario_selector`, `runtime_publish_gate`;
- **Contribution-модель** (consumer-properties): Pages/Blog/Forum подключают свои свойства через `ConsumerPropertyEditorSchema/Port` — сохранение остаётся за consumer-модулем, схемы сверяются байт-в-байт;
- **Server preview** (`server_preview`): рендер через канонический серверный пайплайн, гейтится provider-профилем;
- **Provider status / capability controls**: rollout-профиль (preview/tree/properties/publish) с политикой fallback, health snapshot (`unobserved` ≠ healthy), accessibility-evidence тесты.

i18n редактора: Fluent-словари `en.ftl`/`ru.ftl`.

### 3.5 Публикация (самый сильный контур)

Цепочка `publish_reviewed` (одна транзакция, receipt):
1. локи страницы/тел, RBAC (`pages:publish` + owned-scope);
2. идемпотентность (`idempotency_key` ≤191 байт, replay возвращает сохранённый чек без пересборки);
3. проверка `expected_version` + `expected_body_revisions` (CAS по `updated_at` тел);
4. гейты билдера (rollout-флаги preview/properties/publish) и scenario-release гейт;
5. **санитизация** `sanitize_static_landing_project`: allowlist тегов, запрет опасных типов (`script/style/iframe/object/embed/...`), запрет `on*`-атрибутов, `style`/`srcset`/`srcdoc`/`srcset`-атрибутов, политика URL по ролям (nav/resource/image/form/canonical), контроль CSS `url()`/`@import`/`expression`, лимиты ресурсов (16 MiB, 128 pages, 50k нод, глубина 128, 4096 ассетов, 20k правил);
6. **материализация** `compile_materialized_static_landing` — runtime-снапшот на страницу + повторная полная проверка политики (runtime не может обойти санитайзер);
7. immutable-артефакты (HTML документ, HTML тела, CSS) с хэшами (sha256 для контента, компактный `ProjectHash` внутри снапшотов), staging → **switch binding** (указатель на активный набор), запись событий в outbox (`NodePublished/Updated/Deleted`);
8. durable receipt `PublishPageResult` (operation_id, review_hash, sanitized_set_hash, artifact_set_hash, replayed).

Рядом: `rollback_to_previous` (откат на предыдущий distinct набор артефактов), `artifact_rebuild` (append-only пересборка с CAS по provenance-хэшу), `artifact_integrity_audit` + `artifact_repair`, `artifact_binding_replacement`, `route_history_import` (импорт публичной истории маршрутов с fail-closed проверкой владельца).

Статика: `StaticLandingCompiler` (inspect/compile_publish), лимиты `PageBuilderStaticPublishResourceLimits`, политика `page_builder_static_publish_policy_v1` с форматом-хэшом в конверте `v2` (policy_hash + exact project).

### 3.6 Публичная доставка (storefront)

- `resolve_storefront_page_route`: канонический slug по локали с fallback, disposition `canonical/redirect/gone/not_found/conflict`, гейт модуля `pages` в канале запроса, tenant из `TenantContext` с fail-closed сверкой slug;
- алиасы/редиректы: смена slug опубликованной страницы создаёт redirect-алиас; удаление — `gone`; история иммутабельна;
- **отдача артефакта** `GET /api/pages/{id}/artifact`: ETag/If-None-Match, `Vary` по tenant/channel-контексту, **CSP с `style-src 'sha256-...'`** (точный хэш CSS, без unsafe-inline), media/img ограничены self/https/data;
- кэш: `PagesCacheReadRuntime/Invalidation` — генерации route/page/artifact, событийная инвалидация (`page.updated/published/unpublished/deleted`), TTL 60s, лимит 10k ключей, ключ ≤512 байт, значение ≤2 MiB;
- **inline edit**: WASM (`rustok_storefront`), HMAC-grant с TTL и key-id, bootstrap/module/wasm ассеты с `must-revalidate`, доменные маркеры запрещены в DOM/URL (токены не утекают в разметку), commit через `/api/fn/pages/inline-edit/commit`;
- SEO: `PagesSeoTargetProvider` — load/resolve_route/sitemap_candidates/bulk/alternate routes/OG + structured data `web_page` через `rustok-seo`; metadata-поля переводимы через Translation-target (CAS, receipts, content-free change cursor).

### 3.7 Безопасность (фактически в коде)

- RBAC `pages:create/read/update/delete/list/publish/manage` + `enforce_owned_scope` (author-owned vs tenant-wide);
- транспортные контракты: deadline обязательны для read, deadline+idempotency для write (`PortCallPolicy`); авторизация capability по 4 ролям (Preview/Tree/Properties/Publish);
- GraphQL-чтения: `query_tenant_id` отклоняет чужой tenant и расхождение `AuthContext.tenant_id` (исправлено 2026-10-08);
- inline-edit grants: HMAC, TTL, same-origin launch;
- санитизация publish: fail-closed, повторная проверка после materialization; opaque-контент с разметкой отвергается;
- `fly`/`fly-browser` — `forbid(unsafe_code)`; детерминированный рендер экранирует весь текстовый контент (`push_escaped_html`).

---

## 4. Пробелы в реализации

### 4.1 Продуктовые пробелы (P0 — блокируют полноценное CMS-использование)

| # | Пробел | Факт в коде | Последствие |
|---|---|---|---|
| G-1 | **Нет чернового редактирования живой страницы** | `ensure_document_is_mutable` → `PAGE_PUBLISHED_DOCUMENT_IMMUTABLE`: «Published page documents are immutable without a separate draft revision. Unpublish this page before editing» | Нельзя править контент при живой версии — нет редакторского цикла «черновик → публикация». У Webflow/WP/Wix/Strapi это базовый сценарий. Отдельной таблицы черновиков нет |
| G-2 | **Нет истории версий документа** | `page_bodies` — одна строка на (page, locale), `save_document` перезаписывает `content`; журнал есть только для scenario-baseline (не для контента) | Потеря правок одним сохранением не восстановима. Undo — только в памяти клиента (100 записей, не переживает reload). Нет «restore version» |
| G-3 | **Нет расписания публикации** | `grep schedul/publish_at` — отсутствует; publish только немедленный | Нельзя запланировать выход материала. Рынок (WP plugins, Webflow, Tilda, Strapi) — стандарт |
| G-4 | **Нет дублирования страниц** | Нет `duplicate/clone_page` в сервисах/GraphQL | Каждый лендинг создаётся с нуля |
| G-5 | **Нет глобальных символов/символьных компонентов** | `ProjectFragment` — только clipboard-перенос; нет инстансов, обновляемых из одного источника («изменить везде») | Header/CTA-блок копируются физически; ребрендинг = ручная правка всех страниц. Webflow Symbols, WP reusable blocks, Tilda «глобальные блоки» — есть |
| G-6 | **Нет медиатеки/загрузки файлов** | `AssetSection` — ручные поля id+URL; `AssetProviderDefinition` — декларация без реализации; интеграции с `rustok-media` **нет** | Нет загрузки, кадрирования, библиотеки, выбора из галереи. `srcset` и `picture`-оптимизация запрещены политикой атрибутов → нет адаптивных изображений |
| G-7 | **Нет бэкенда форм** | Формы рендерятся и отправляются (browser bridge), но `ProviderAction` не имеет серверного получателя; модуля submissions/leads нет | Форма «контакт» на лендинге никуда не попадает. Для лидогенерации критично |
| G-8 | **Нет встраивания (embeds)** | Static publish: `iframe/embed/object/script` в `DANGEROUS_COMPONENT_TYPES`; `raw_html` экранируется как текст при рендере | Нельзя вставить YouTube/Vimeo/Google Maps/календарь/форму стороннего сервиса. У всех конкурентов — есть |

### 4.2 Функциональные пробелы (P1)

| # | Пробел | Факт | Комментарий |
|---|---|---|---|
| G-9 | **Страницы не индексируются в поиск** | `rustok-search` содержит projectors только для forum/blog/product; pages-проектора нет | Собственный поиск платформы не находит опубликованные страницы |
| G-10 | **Навигация не связана со страницами** | `rustok-navigation` не ссылается на pages (нет page_id/slug-связей) | Меню собираются вручную; смена slug не обновляет меню |
| G-11 | **Нет шаблонов/мастер-страниц** | `pages.template` — свободная строка-метка; нет layout-наследования, секций-шаблонов, `page templates` в билдере (только landing-блоки) | Header/footer — статические виджеты `apps/storefront`, не CMS-контент |
| G-12 | **Нет дизайн-токенов/глобальных стилей** | Стили — per-component правила + 3 фикс-брейкпоинта; нет typography presets, цветовых палитр, class-системы как в Webflow | Ребрендинг = перебор стилей |
| G-13 | **Нет совместного редактирования** | Оптимистичный CAS (`expected_revision`) → при конфликте отказ, без merge/присутствия/комментариев | Два редактора не работают одновременно; нет review-комментариев |
| G-14 | **Publish — «всё или ничего» по локалям** | `publish_reviewed` проверяет `expected_body_revisions` по всем локализациям и собирает полный набор | Нельзя опубликовать только EN, отложив RU. Нейтральный пробел, но у WP-multilingual/Strapi есть per-locale publish |
| G-15 | **Перевод тел статей не покрыт Translation-модулем** | `PagesMetadataTranslationTargetProvider` — только метаданные (title/slug/meta_title/meta_description: 2+2 поля) | Тела переводятся вручную (per-locale документы) или через `flyTranslations` внутри документа; AI-перевод тела не подключён |
| G-16 | **Нет анимаций/интеракций** | grep animation/scroll/parallax в fly — пусто | Нет аналога Webflow Interactions / Tilda-анимаций |
| G-17 | **Нет A/B-тестов и персонализации** | Runtime-сценарии есть (инфраструктура готова), но нет экспериментального движка, таргетинга, сплит-трафика | Ближайший задел: scenario_id + baseline + materialization |
| G-18 | **Превью не публикуемое** | Server preview — только внутри админской сессии; нет share-link со сроком действия | У конкурентов «показать клиенту по ссылке» — стандарт |

### 4.3 Инженерные/эксплуатационные пробелы (P2)

| # | Пробел | Факт |
|---|---|---|
| G-19 | Одиночный PK вместо заявленного composite `(tenant_id, id)` | Изоляция держится на дисциплине фильтров (прошлый аудит находил пропуски) |
| G-20 | REST не покрывает publish/rollback/rebuild | Только GraphQL; асимметрия контуров |
| G-21 | Next.js-админка без раздела Pages/Builder | Playwright-тесты есть, UI-раздела нет; редактор только в Leptos-админке → два admin-стека без паритета (верификаторы `verify-page-builder-next-admin-parity.mjs` следят за контрактом, но не за функциональностью) |
| G-22 | `fly-leptos`/`fly-dioxus` — оболочки без холста | Мульти-фреймворк FFA заявлена, работает только Leptos+SSR-путь |
| G-23 | Журнал scenario-baseline: без retention и без UI | append-only рост; чтение через GraphQL/SQL (известное ограничение README) |
| G-24 | Кэш: TTL 60s фиксирован | Нет stale-while-revalidate/ISR-подобного поведения на артефактах (хотя артефакты иммутабельны и могли бы кэшироваться вечно по хэшу) |
| G-25 | Нет rate-limit на публичные артефакты в контроллере | Есть CSP/ETag, нет троттлинга (может покрываться платформенным слоем) |
| G-26 | Undo не переживает перезагрузку | History только в `FlyEditor` в памяти; снапшоты есть, но каталог клиентский |
| G-27 | Нет экспорта сайта/страницы (HTML zip, архив) | `ProjectFragment` — только перенос в буфере; есть `snapshot`/codec, но нет продуктового export-флоу |
| G-28 | `raw_html` в каталоге обещает HTML, рендерится как текст | Несоответствие ожиданий: тип есть, семантики «сырого HTML» нет (безопасно, но вводит в заблуждение палитра) |

### 4.4 Что проверено и работает без замечаний (чтобы не перепроверять)

- Идемпотентная публикация с replay-чеками и request-hash CAS; событийный outbox не дублируется при replay;
- Откат (`rollback_to_previous`) и append-only rebuild с provenance-хэшами; аудит/ремонт целостности артефактов;
- Санитайзер и его повторное применение после materialization (runtime-обход невозможен) — сверено по коду: validators реально вызываются (`artifact_rebuild.rs`, `publish_manifest.rs`, `reviewed_publish.rs`);
- CSP с hash-для CSS; ETag/Vary по tenant+channel;
- Route-алиасы/redirect/gone история;
- Tenant-guard на GraphQL-чтениях; inline-edit HMAC-grants без утечки токенов в DOM/URL;
- Мультитенантность storefront-чтений с fail-closed сверкой slug;
- Событийная инвалидация кэша по scope route/page/artifact;
- Contribution-модель consumer-свойств (Pages/Blog/Forum) с байтовой верификацией схем.

---

## 5. Сравнение с популярными платформами

Условные обозначения: ✅ — есть и работает, 🟡 — частично/задел, ❌ — отсутствует.

| Функция | RusTok Pages/Builder | Webflow | WordPress + Elementor/Gutenberg | Wix / Squarespace | Tilda / Carrd | Builder.io / Plasmic | Strapi / Payload (headless) |
|---|---|---|---|---|---|---|---|
| Визуальный редактор (canvas DnD, слои, дерево) | ✅ (canvas в iframe, слои, resize, shortcut'ы) | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 (blocks-редакторы) |
| Инспектор свойств/стилей | ✅ (traits + style rules + 3 брейкпоинта) | ✅ (классы, любые брейкпоинты) | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Data-binding / динамический контент | ✅ (биндинги, условия, репитеры, типизированный контекст) | 🟡 (CMS collections) | 🟡 (plugins) | 🟡 | ❌ | ✅ (bindings/API) | ✅ (API-first) |
| Черновик при живой версии | ❌ (unpublish-first) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| История версий / restore | ❌ (только журнал baseline) | ✅ | ✅ (revisions) | ✅ | ✅ | ✅ | ✅ |
| Расписание публикации | ❌ | ❌ | ✅ (plugins/core) | ❌ | ✅ | ❌ | ✅ |
| Глобальные символы/символьные секции | ❌ (fragment-copy) | ✅ Symbols | ✅ reusable blocks | ✅ | ✅ | ✅ | 🟡 (components/dynamic zones) |
| Медиатека, загрузка, оптимизация | ❌ (ручной URL) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Адаптивные изображения (srcset) | ❌ (srcset запрещён) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Формы + хранилище заявок + уведомления | ❌ (только рендер/submit) | ✅ | ✅ | ✅ | ✅ | 🟡 (webhooks) | 🟡 |
| Embeds (YouTube, карты, календари) | ❌ (iframe запрещён) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (richtext) |
| Анимации/интеракции | ❌ | ✅ | 🟡 | 🟡 | ✅ | 🟡 | ❌ |
| Шаблоны/мастер-страницы | ❌ (template = строка) | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Дизайн-токены/глобальные стили | ❌ | ✅ (variables) | 🟡 | ✅ | ✅ | ✅ | ❌ |
| Мультиязычность тела контента | ✅ (per-locale документы + flyTranslations; AI/Translation — только метаданные) | ✅ (localize) | ✅ (плагины) | ✅ | 🟡 | 🟡 | ✅ (i18n plugin) |
| SEO: мета, OG, canonical, sitemap, redirects, structured data | ✅ (seo-targets, sitemap, alias-redirect/gone, web_page schema) | ✅ | ✅ | ✅ | ✅ | 🟡 | 🟡 |
| Расписанный/пошаговый review-процесс | 🟡 (reviewed publish + review_hash, нет workflow/одобрений) | 🟡 | 🟡 (плагины) | ❌ | ❌ | 🟡 | ✅ (draft/publish review в части) |
| Откат публикации | ✅ (артефакт-сеты, immutable, integrity audit) | ✅ | ✅ | ✅ | 🟡 | ✅ | 🟡 |
| Целостность публикаций (hash, provenance, receipt) | ✅✅ (SHA-256 CAS, идемпотентные чеки — редкость на рынке) | ❌ | ❌ | ❌ | ❌ | 🟡 | ❌ |
| Совместное редактирование/комментарии | ❌ (CAS-конфликты) | ✅ | 🟡 | ✅ | ❌ | ✅ | 🟡 |
| A/B-тесты/персонализация | 🟡 (runtime-сценарии без экспериментов) | ❌ | 🟡 | 🟡 | ❌ | ✅ | ❌ |
| Мульти-тенант / каналы (web+mobile) | ✅✅ (tenant + channel visibility + модульные гейты) | ❌ | ❌ | ❌ | ❌ | ✅ | 🟡 |
| Публичный поиск по контенту | ❌ (pages не проецируются) | ✅ | ✅ | ✅ | 🟡 | 🟡 | ✅ |
| Меню/навигация из CMS | ❌ (модуль не связан) | ✅ | ✅ | ✅ | ✅ | 🟡 | 🟡 |
| Расширяемость компонентов | ✅ (регистры, plugins, contributions, provider-контракты) | 🟡 | ✅ (плагины) | ❌ | ❌ | ✅ | ✅ |
| Inline-редактирование на storefront | ✅ (WASM + HMAC-grants) | ❌ | ✅ (фронт-редактор) | ✅ | ❌ | ✅ | ❌ |
| Безопасность вывода (санитайзер, CSP) | ✅✅ (fail-closed allowlist + hash-CSP) | 🟡 | 🟡 (зависит от плагинов) | 🟡 | 🟡 | 🟡 | ✅ |
| AI-генерация контента | 🟡 (AI-модули платформы есть, адаптера «pages» нет) | ✅ | ✅ (плагины) | ✅ | ✅ | ✅ | 🟡 |
| Экспорт/импорт сайта | 🟡 (lossless codec есть, UX-флоу нет) | 🟡 | ✅ | ❌ | ❌ | ✅ | ✅ |
| Мобильное приложение автора | 🟡 (rustok_mobile, Flutter wave-handoff контракты) | ❌ | 🟡 | 🟡 | ❌ | ❌ | ❌ |

### 5.1 Где RusTok объективно впереди рынка

1. **Дисциплина публикации.** Ни Webflow, ни Wix, ни WP не дают такого контура: иммутабельные артефакты, SHA-256 провенанс, идемпотентные операции с чеками, откат на точный предыдущий набор, аудит целостности с ремонтом связок, CAS (`expected_baseline_hash`, `expected_provenance_hash`). Это уровень «финтех-публикация контента».
2. **Fail-closed безопасность вывода.** Allowlist + двойная проверка (до и после runtime-материализации), запрет `on*`/`srcdoc`/инлайн-стилей, контролируемые `url()` в CSS, точный CSP-hash. Рынок (особенно WP с плагинами) здесь слаб.
3. **Типизированный runtime-контент.** Биндинги + условия + репитеры + контракты контекста (JSON-schema + preflight) + сценарии с release-gate — компактный аналог Builder.io/Plasmic с гарантиями.
4. **Мультиарендность и каналы.** Tenant + channel + module gating из коробки; у SaaS-конструкторов этого нет вовсе.
5. **Inline-edit на storefront с HMAC-grants** и отсутствием токенов в DOM/URL — аккуратнее, чем у большинства CMS.

### 5.2 Где рынок объективно впереди

См. таблицу выше; критичнее всего для продукта: черновики при живой версии (G-1), история версий (G-2), медиатека (G-6), формы-бэкенд (G-7), символы (G-5), embeds (G-8), расписание (G-3).

---

## 6. Приоритетные рекомендации (roadmap-порядок)

**Волна 1 — «редакторский цикл» (закрывает главный разрыв с рынком):**
1. **Draft-версия тела документа** (отдельная строка `page_bodies` с `state=draft` или таблица `page_body_drafts` + preview по гранту). Опубликованная версия остаётся иммутабельной — модель артефактов не ломается, публикация становится «promote draft».
2. **История версий тела** (append-only, как журнал baseline; UI — список версий + restore). Практически готовый образец уже есть в `page_builder_scenario_baseline_revision`.
3. **Дублирование страницы** (copy: metadata + bodies + channel visibility; слаги с суффиксом).

**Волна 2 — «контент-операции»:**
4. Медиа-интеграция: `AssetProvider` поверх `rustok-media` (upload, галерея, выбор), плюс разрешение `srcset`-безопасного пути для адаптивных изображений.
5. Form-submissions модуль (таблица отправок + события в outbox + email-уведомления через `rustok-email` + простой inbox в админке); honeypot/rate-limit.
6. Планировщик публикации (job по `publish_at`, идемпотентный `publish_reviewed` уже готов к вызову из шедулера).

**Волна 3 — «структура сайта»:**
7. Символы/секции (ссылочный тип компонента на «определение», переиздание всех вхождений при изменении).
8. Шаблоны страниц (layout-обёртки из секций) и связка `navigation` ↔ pages (меню, авто-обновление при смене slug).
9. Pages-проектор в `rustok-search` (переиспользовать модель blog/forum-проекторов).

**Волна 4 — «конкурентоспособность UX»:**
10. Embeds-политика: allowlisted-iframe (YouTube/Vimeo/Maps отдельным компонентом с фиксированными host allowlist + `sandbox`/`referrerpolicy`), не снятие общего запрета.
11. A/B на базе runtime-сценариев (variant selector + cookie bucket + тот же baseline-контур).
12. Share-превью по подписанной ссылке с TTL (механика грантов inline-edit уже есть).
13. Per-locale publish (гранулярность `expected_body_revisions` уже по-локальная — осталось разрешить частичный набор).

**Быстрые победы (дни):** REST-эндпоинты publish/rollback поверх существующих сервисов; retention-политика журнала baseline; экспорт страницы в HTML; размещение версий baseline в UI (read уже есть).

---

## 7. Методология и границы выводов

- Метод: статический аудит исходников (cargo/rustc в среде отсутствуют — компиляция/тесты не подтверждаются), чтение моделей, сервисов, миграций, GraphQL/REST-контрактов, редактора (Leptos), storefront-слоя и смежных модулей; сверка заявленного (README/ADR) с фактическим кодом.
- Все утверждения о наличии/отсутствии функций сопровождаются путями в репозитории; «отсутствует» означает «не найдено в workspace после полного поиска по связанным модулям».
- Продуктовое сравнение с платформами — по публично известным возможностям продуктов (Webflow, WordPress, Wix/Squarespace, Tilda/Carrd, Builder.io/Plasmic, Strapi/Payload) на дату аудита.
- Инженерные дефекты CI/toolchain (F-1…F-9 из аудита 2026-10-08) не пересматривались; часть из них уже закрыта. Открытые на текущий момент: F-3 (WebKitGTK-зависимости workflow), F-4/F-5 (нужен toolchain) — см. компаньон.
