# Модуль product — инженерный аудит

**Дата:** 2026-10-07
**Область:** `crates/modules/rustok-product` (владелец каталога: продукт, варианты, оси вариантов, изображения, категории, EAV-атрибуты и схемы категорий, переводы, порты), пакеты `rustok-product-admin`, `rustok-product-storefront`, FFA-пакет `@rustok/product-admin` (`apps/next-admin/packages/rustok-product`), storefront-пакет `@rustok/product` (`apps/next-frontend/packages/rustok-product`), commerce-транспорт продукта (`crates/modules/rustok-commerce/src/graphql/*`, REST-контроллеры), смежные владельцы (`rustok-pricing`, `rustok-inventory`, `rustok-tax`, `rustok-media`, `rustok-product-relations`, `rustok-product-bundles`, `rustok-product-transport`, `rustok-product-catalog-service`), верификаторы `scripts/verify/verify-product-*` и `verify-*-product-*`.
**Метод:** статическая трассировка исходников (точки входа → транспорт → сервис → SQL/миграции → UI), инвентаризация миграций/портов/мутаций/UI-вызовов, прогон source-level верификаторов; сопоставление с публичной документацией Medusa v2, Saleor, Shopify, WooCommerce, BigCommerce.

**Ограничения окружения (важно для оценки выводов).** В песочнице нет `cargo` и каталога `target/` — модуль невозможно скомпилировать, нельзя запустить clippy и Rust-тесты. Доступен только `node` v22.22.3. Все выводы ниже — результат статического анализа исходников и прогона JS-верификаторов; утверждения о рантайм-поведении помечены как выведенные из кода. Прогон верификаторов:

```text
node scripts/verify/verify-product-admin-boundary.mjs          → passed
node scripts/verify/verify-product-storefront-boundary.mjs     → passed
node scripts/verify/verify-product-catalog-schema.mjs          → passed (source-locked)
node scripts/verify/verify-product-lifecycle-collaboration.mjs → passed (maintainer execution evidence remains open)
node scripts/verify/verify-product-runtime-fallback-smoke.mjs  → passed (no-compile smoke)
```

---

## Резюме для руководителя

Владение каталогом в `rustok-product` спроектировано сильно и нестандартно богато: это не «products + variants», а полноценный EAV-движок с определениями атрибутов по типам значений, схемами, привязанными к категориям (с режимами `inherit / use_schema / clone_from_category / custom`), переопределениями видимости (filterable / searchable / sortable / comparable / storefront / admin_grid), типизированными значениями, publish-требованиями владельца, «detached values» для осиротевших значений, атомарным `ProductWriteTransaction` с outbox-событиями до commit, PostgreSQL-инвариантами (tenant-уникальность handle/SKU, deferred closure-дерево категорий, триггеры `index_revision`, GIN-индекс по `metadata`, триггер нормализации видимости канала), обязательной идемпотентностью команд, публичными read-портами (`product.catalog_read.v1`) и gRPC-транспортом порта.

При этом **модуль не доведён до продуктовой целостности**: часть функциональности существует только на уровне схем БД, сервисов или транспорта и не имеет ни одного потребителя в UI, а обе витрины не могут показать покупателю ни одного изображения товара и ни одной цены в листинге. Наиболее серьёзные подтверждённые пробелы:

1. **Изображения товара не отображаются нигде в витрине** (`PROD-MEDIA-001`, P1). Ни Leptos-витрина, ни Next-витрина не запрашивают изображения и не рендерят их: в Next-карточке и на детальной странице нарисованы заглушки-иконки «Thumbnail Placeholder» / «Media Gallery». Модуль `rustok-media` существует, но `rustok-product` не имеет с ним ни одной зависимости, а `product_images.media_id` — невалидируемый UUID.
2. **Сохранение значений атрибутов из Next-админки сломано** (`PROD-FFA-ATTR-001`, P1): FFA-мутация `SAVE_ATTRIBUTE_VALUES_MUTATION` не передаёт обязательный серверный аргумент `$idempotencyKey: String!`, который владелец требует (`mutations/catalog.rs:1408-1411`). Вызов гарантированно отклоняется валидатором GraphQL, а поскольку сохранение продукта и сохранение атрибутов идут в одном server action, пользователь получает частичную запись.
3. **Листинг витрины не показывает цену** (`PROD-LIST-001`, P1): `StorefrontProductListItem` и `GqlProductListItem` не содержат цены; обе витрины рендерят сетку карточек без цены, хотя у владельца есть price lists, scoped prices, каналы, скидки и тиры.
4. **Принятая ADR-архитектура осей вариантов недостижима из UI** (`PROD-AXES-001`, P2): команда `set_variant_axes` (и GraphQL-мутация) не вызывается ни из Leptos-, ни из Next-админки; в Leptos-редакторе есть текстовое поле «Size, Color», которое только отображается, а в Next — мутации осей нет вовсе.
5. **Авторинг схем мёртв в обоих админ-интерфейсах** (`PROD-SCHEMA-UI-001`, P2): `set_category_schema_mode`, `create_product_attribute_schema_group`, `create_category_attribute_group`, `bind_schema_attribute`, `bind_category_attribute` реализованы в транспорте (native + GraphQL), но не вызываются ни одной UI-веткой; в Next-пакете то же самое с `bindSchemaAttribute`.
6. **Декларативная валидация атрибутов не применяется** (`PROD-VALID-001`, P2): колонки `validation JSONB` и `validation_overrides JSONB` существуют, но `validate_product_value_patch` их не читает — типы, опции и bounded-JSON проверяются, а заявленные ограничения нет.
7. **Виртуальные категории и channel-settings атрибутов существуют только как схема** (`PROD-VCAT-001`, `PROD-GROUP-001`, P2): правила виртуальных категорий умеют только валидироваться, таблица `virtual_category_product_assignments` не имеет ни писателей, ни читателей; `product_attribute_channel_settings` / `product_attribute_group_channel_settings` не читаются рантаймом, из-за чего per-channel видимость атрибутов не работает.
8. **Две админ-поверхности сильно разошлись по возможностям** (`PROD-PARITY-001`, P2): Next-админка умеет price list, канал, `minQuantity/maxQuantity`, превью и применение скидок (через pricing-владельца), relations, bundle-редактор и values-редактор атрибутов; Leptos-админка цену пишет только как часть payload варианта, а тиры/скидки/price list не умеет вообще. При этом внутри Leptos-пакета живут три параллельные реализации админки, из которых смонтирована одна (`ui/root.rs` → grid/editor), а две другие (включая `ui/leptos.rs` на 2570 строк) не смонтированы.
9. **Управление жизненным циклом неполно на уровне контракта** (`PROD-EVENT-001`, `PROD-PORT-001`, P2): снятие с публикации не имеет собственного события (`ProductUpdated` вместо `ProductUnpublished`), `ProductStatus::Archived` не выставляется ни одной командой владельца (только через generic `updateProduct(status)`), а `ProductCatalogCommandPort` (13 операций) вообще не покрывает смену статуса и операции EAV/категорий.
10. **Нет конкурентного контроля редактора** (`PROD-CONC-001`, P2): отсутствует business revision/CAS-токен, а `UpdateProductInput` использует `Option<T>`, не различая `Keep / Set / Clear` — параллельное редактирование теряет изменения (это же зафиксировано во внутреннем `docs/reference-hardening-current.md`).
11. **Отсутствуют массовые операции** (`PROD-BULK-001`, P2): нет bulk-редактора, нет CSV-импорта/экспорта, нет batch-операций — при этом уникальность `handle`/`SKU` строгая, что делает заведение больших каталогов ручным.
12. **Нет цифрового fulfillment, нет владельца доставки, нет tax_class** (`PROD-DIGITAL-001`, `PROD-SHIP-001`, `PROD-TAX-001`, P2/P3): `shipping_profile_slug` — свободная строка без валидации, модуля shipping в репозитории нет; налог включается только по `product_type`; цифровые товары допускаются, но типизированного контракта на них нет (зафиксировано как accepted cutover в `docs/reference-hardening-current.md`).

**Где модуль сильнее рынка.** Typed EAV со схемами категорий, режимами наследования и override-флагами видимости; publish-requirements владельца, блокирующие переход в `Active` без заполнения обязательных атрибутов и явной переводовой строки; «detached values» с отдельным списком и очисткой; tenant-инварианты и closure-дерево категорий на уровне БД; обязательная идемпотентность и deadline-политика команд; переводовые targets для продукта/варианта/изображения; публичный FBA-порт с gRPC-адаптером и fail-closed выбором провайдера. По этим осям RusTok ближе к Saleor, чем к Shopify/WooCommerce, и заметно богаче Medusa v2 «из коробки».

---

## Сводка находок

| ID | Severity | Область | Находка |
|---|---|---|---|
| PROD-MEDIA-001 | **P1** | Витрина / media | Изображения товара не запрашиваются и не рендерятся ни в одной витрине; `media_id` не валидируется владельцем |
| PROD-FFA-ATTR-001 | **P1** | Next-админка | `SAVE_ATTRIBUTE_VALUES_MUTATION` не передаёт обязательный `idempotencyKey` → сохранение значений атрибутов падает, продукт при этом уже обновлён |
| PROD-LIST-001 | **P1** | Витрина / цены | Листинг каталога не содержит цены (модели list-item без цены; карточки без цены) |
| PROD-AXES-001 | P2 | Варианты / UI | Команда осей вариантов (`set_variant_axes`, `setProductVariantAxes`) недостижима из UI обеих админок |
| PROD-SCHEMA-UI-001 | P2 | EAV / UI | Авторинг схем (режим, группы, привязки атрибутов) реализован в транспорте, но не вызывается из UI |
| PROD-VALID-001 | P2 | EAV | `validation` / `validation_overrides` JSON не применяются при записи значений |
| PROD-VCAT-001 | P2 | Категории | Виртуальные категории: только валидация правил, материализации и чтения нет |
| PROD-GROUP-001 | P2 | EAV / каналы | `product_attribute_channel_settings`, `product_attribute_group_channel_settings` не читаются; сущности групп без сервисов |
| PROD-PARITY-001 | P2 | Админки | Next- и Leptos-админки разошлись по ценам, скидкам, relations, bundles и группам атрибутов |
| PROD-DUPUI-001 | P2 | Админка | Три параллельные Leptos-реализации; в смонтированной ветке результаты 8 операций отбрасываются (`let _ =`), включая удаление продукта |
| PROD-IDEM-001 | P2 | Commerce GraphQL | Relations и bundles конструируют сервисы напрямую: нет idempotency key, receipt и deadline-политики |
| PROD-EVENT-001 | P2 | Жизненный цикл | Нет `ProductUnpublished`; `Archived` не выставляется командами владельца |
| PROD-PORT-001 | P2 | Порты | `ProductCatalogCommandPort` не покрывает статус/архив/EAV/категории |
| PROD-CONC-001 | P2 | Редактор | Нет business revision/CAS; `Option<T>`-патчи не различают `Keep / Set / Clear` |
| PROD-BULK-001 | P2 | Операции | Нет bulk-редактора, CSV-импорта/экспорта и batch-операций |
| PROD-STORE-001 | P2 | Витрина / фасеты | Фильтры по атрибутам — сырая строка `code=value;…` (Leptos) либо отсутствуют (Next) |
| PROD-ATTR-STORE-001 | P2 | Витрина / EAV | Значения атрибутов не отдаются в витрину: у детальной страницы нет спецификаций |
| PROD-SHIP-001 | P2 | Доставка | `shipping_profile_slug` не валидируется, владельца shipping-профилей нет |
| PROD-TAX-001 | P3 | Налоги | Налоговая логика привязана только к `product_type`, нет `tax_class` |
| PROD-DIGITAL-001 | P2 | Fulfillment | Нет типизированного digital/virtual-контракта (downloadables, исключение из shipping) |
| PROD-TESTS-001 | P2 | Тесты | В модуле нет поведенческих тестов сервисов: только миграционные/катовер-фикстуры |
| PROD-LOCALE-001 | P3 | Next-админка | Чтение effective form и значений атрибутов жёстко с локалью `en`, запись — с активной локалью |
| PROD-IMG-001 | P3 | Next-админка | `UPDATE_PRODUCT_IMAGE_MUTATION` объявлена, но функции-обёртки нет: alt/position из Next не редактируются |
| PROD-META-001 | P3 | Метаданные | Тени в `metadata` (tags, shipping profile, custom_fields) вместо типизированного состояния |
| PROD-RICHTEXT-001 | P3 | Контент | Тип `richtext` фактически скалярный текст; richtext-контракт отложен по решению владельца |
| PROD-ENTITIES-001 | P3 | Публичный API | `pub mod entities` — персистентность торчит в публичный API модуля |
| PROD-SEO-001 | P3 | SEO | Категории каталога не входят в SEO-регистр (референс на аудит SEO) |

---

## Часть I. Инвентаризация функциональности

### 1.1 Владение, границы, манифест

- `rustok-product` — модуль-владелец каталога, `ui_classification = "dual_surface"` (админка + витрина), зависимости `{taxonomy}`, co-requisites `{inventory, pricing}`; права `PRODUCTS_{CREATE,READ,UPDATE,DELETE,LIST,MANAGE}`.
- Манифест `crates/modules/rustok-product/rustok-module.toml`: admin UI (`leptos_crate = "rustok-product-admin"`, `route_segment = "product"`, child-pages `products / new / categories / attributes`, локали `en, ru`), storefront UI (`leptos_crate = "rustok-product-storefront"`, `route_segment = "products"`, `slot = "home_after_catalog"`), FBA-провайдер (`contracts/product-fba-registry.json`, `product.catalog_read.v1`, статус `boundary_ready`).
- Монтирование UI — кодогенерация хостов: `apps/admin/build.rs` генерирует `module_registry_codegen.rs` и рендерит для модуля компонент `{Slug}Admin` → `rustok_product_admin::ProductAdmin` (реэкспорт `ui::root::ProductAdmin`); `apps/storefront/build.rs` рендерит `{Slug}View` → `rustok_product_storefront::ProductView` (`ui/leptos.rs`). Это важно для оценки «мёртвого кода»: смонтированы именно `ui/root.rs` (админка) и `ui/leptos.rs` (витрина), а не одноимённые компоненты из других модулей пакета.
- Размер: 233 файла, 69 011 строк `*.rs`. Крупнейшие файлы: `admin/src/core.rs` (3926), `admin/src/ui/leptos.rs` (2570), `src/services/catalog/commands.rs` (2470), `.../catalog_schema_service/attributes/translation.rs` (2048), `.../schemas/translation.rs` (1995), `.../categories/translation.rs` (1796), `admin/src/transport/native_server_adapter.rs` (1432), `src/services/catalog_schema_service.rs` (1437).
- Смежные владельцы: `rustok-product-relations` (2470 строк), `rustok-product-bundles` (4276), `rustok-product-transport` (1700, tonic gRPC), `rustok-product-catalog-service` (508, отдельный процесс), `rustok-ai-product` (307), `rustok-pricing(+persistence)`, `rustok-inventory`, `rustok-tax`, `rustok-media`, `rustok-taxonomy`.

### 1.2 Персистентность: 35 миграций, ~33 таблицы

Каталог ведётся исключительно в PostgreSQL (`migrations` требуют `DatabaseBackend::Postgres`). Ключевые миграции:

- `m20250130_000012/13/14` — базовый продукт/вариант/изображения; `m20250130_000013` удалил legacy `product_options*`;
- `m20260301_000001` — поля варианта + частичные уникальные индексы `uq_product_variants_combination (product_id, combination_identity)` и `uq_product_variants_default` (дефолтный вариант);
- `m20260405_000007` — расширение локалей;
- `m20260701_000001` — большой блок EAV: `product_attributes` (+ переводы, опции, типы значений, `validation JSONB`, `validation_overrides JSONB`), `product_attribute_schemas`, `category_attributes`, `category_attribute_schema_assignments`, `product_attribute_values` (+ переводы, опции значений), `catalog_categories` (+ переводы, SEO, closure), `virtual_category_product_assignments` (строка 404);
- `m20260701_000002` — tenant-консистентность (составные FK `(tenant_id, id)` для всех дочерних таблиц);
- `m20260711_000001..000004` — enum статуса продукта, tenant-integrity уникальные индексы, инварианты типов значений, триггер нормализации `metadata.channel_visibility.allowed_channel_slugs` + GIN-индекс `products USING GIN (metadata jsonb_path_ops)`;
- `m20260725_000002/000003` — инварианты дерева категорий (deferred closure), удаление переходных колонок и ужесточение констрейнтов изображений (`media_id NOT NULL`);
- `m20260730_000001`, `m20260731_000003` — триггеры/индексы `index_revision`;
- `m20260918_000033` — «unified variant axis»: `product_variant_axes`, `product_variant_axis_values`, `product_attribute_groups` (+ translations, + attributes, + channel settings), политики `variant_axis_policy` / `default_variant_axis`, SQL-функции `rustok_product_compute_combination_identity` и constraint-триггер `trg_maintain_combination_identity`;
- `m20260927_000034` — последняя в цепочке.

Инварианты в БД: tenant-уникальность handle/SKU, уникальность `(tenant_id, product_id, attribute_id)` для значений (`uq_product_attribute_values`), `idx_product_attribute_values_lookup` для фильтрации, уникальные оси/значения осей по позиции, FK на `tenants(id)`, deferred-проверка дерева категорий.

### 1.3 Агрегат «продукт»

Поля: `id, tenant_id, status (draft|active|archived), handle, seller_id, vendor, product_type, shipping_profile_slug, primary_category_id, metadata (JSONB), published_at, created_at, updated_at` + переводы (`product_translations`: локаль, заголовок, handle, описание, meta title/description) + теги (`product_tags`).

Сервис (`src/services/catalog/commands.rs`, 2470 строк): `create_product:62`, `update_product:393`, `publish_product:720`, `unpublish_product:763`, `delete_product:795`, `create_variant:904`, `update_variant:1060`, `delete_variant:1226`, изображения ~1287–1610, `set_variant_axes:1622`. Все записи идут через `ProductWriteTransaction` (сущность + outbox в одной транзакции, `SELECT … FOR UPDATE`, события публикуются до commit), валидация входа — `input.validate()`, уникальность маппится `map_product_unique_violation`, удаление последнего варианта запрещено (`CannotDeleteOnlyVariant`), удаление опубликованного — `CannotDeletePublished`.

Особенности, подтверждённые чтением кода:

- `update_product` **исполняет** `validate_product_publish_requirements_in` при переходе в `Active` (строки ~500–660) — то есть обойти publish-валидацию через generic update нельзя (проверено специально, поскольку это был подозреваемый дефект);
- `publish_product` валидирует требования и выставляет `published_at`, публикует `ProductPublished`;
- `unpublish_product` выставляет `Draft`, очищает `published_at` и публикует **`ProductUpdated`** (а не отдельное событие снятия с публикации);
- канальная видимость: `metadata.channel_visibility.allowed_channel_slugs`, нормализуется триггером; чтение витрины фильтрует по каналу;
- цена и остаток варианта пишутся в рамках продуктовых команд: цены — через `rustok-pricing-persistence` (delete+recreate набора цен варианта), остаток — через inventory bootstrap/availability.

### 1.4 Варианты и оси вариантов

- `product_variants`: SKU, barcode, title, `shipping_profile_slug`, `combination_identity`, вес/единица, `inventory_policy`, `index_revision`; уникальность SKU в тенанте; дефолтный вариант — частичный уникальный индекс.
- Единый ADR «unified variant axis architecture» (`DECISIONS/2026-09-18-unified-variant-axis-architecture.md`, статус cutover — «Not started»): оси — это атрибуты с `variant_axis_policy ∈ {forbidden, allowed, required}` и `default_variant_axis`; значения оси — опции атрибута; `combination_identity` вычисляется SQL-функцией и поддерживается constraint-триггером (гарантирует, что генерация комбинаций и запись вариантов согласованы на уровне БД).
- Команда владельца `set_variant_axes` (`commands.rs:1622`), GraphQL-мутация `setProductVariantAxes` (`mutations/catalog.rs:309`), порт имеет операцию `set_variant_axes`.
- **Пробел UI:** ни одна админка не вызывает эту команду (см. `PROD-AXES-001`).

### 1.5 Изображения

- Таблицы `product_images` (+ `product_image_translations`) с `media_id` (NOT NULL после `m20260725_000003`), `url`, `alt_text`, `position`; отдельные translation-target и progress-target для изображений, события изменения изображений (`image_translation_changes.rs`), участие в индексации.
- Команды: добавление/обновление/удаление/переупорядочивание изображений; GraphQL-мутации на 524/558/593/626; Leptos-админка: добавление/удаление/порядок (после рефакторинга — включая `update_product_image`), FFA: add/delete/reorder + неиспользуемая `UPDATE_PRODUCT_IMAGE_MUTATION`.
- **Пробел:** в витрине изображения отсутствуют полностью (см. `PROD-MEDIA-001`); `media_id` не проверяется против `rustok-media` (зависимости нет).

### 1.6 Мультиязычность и переводовые targets

- `ProductTranslation`, `VariantTranslation`, `ProductImageTranslation` + «change»-сервисы (`translation_changes.rs`, `variant_translation_changes.rs`, `image_translation_changes.rs`) и lifecycle-маппинг статуса продукта в `TranslationResourceLifecycle` (в том числе `Archived`).
- Владельческие translation targets: продукт (owner slug `product`), вариант, изображение, варианты/изображения — progress-target’ы; в модуле есть `seo_targets.rs`.
- Запись переводов локализована: переводы пишутся для явной локали хоста, sibling-локали не перезаписываются; «detached»-значения атрибутов хранятся отдельно и отображаются в админке отдельным блоком со своей очисткой (`clear_detached_product_attribute_values`).

### 1.7 Категории и дерево

- `catalog_categories` (kind: обычные/виртуальные), переводы, SEO-переводы, closure-таблица с deferred-проверкой целостности дерева (запрет циклов, дублей root-slug, drift closure), `primary_category_id` у продукта (каноническая первичная категория).
- Сервисы категорий: CRUD/чтение, `set_category_schema_mode`, привязки атрибутов к категории; переводы и SEO (1768 строк в `categories/seo_translation.rs`).
- Перенос/катовер taxonomy: фикстуры `category_taxonomy_same_id_cutover.rs`, `product_category_closure_storage_retirement.rs` (313 строк) — исторически категории переезжали из taxonomy в product.

### 1.8 EAV: определения, опции, схемы, режимы, группы

- `AttributeValueType`: `text, textarea, richtext, integer, decimal, boolean, date, datetime, select, multiselect, json`.
- `CategorySchemaMode`: `inherit, use_schema, clone_from_category, custom`; привязки с режимом `use_schema` (наследование схемы категории) и `inherit/snapshot`.
- `AttributeBindingKind`: `addition, override, removal` — переопределение набора атрибутов категории относительно схемы.
- `AttributeVisibilityOverrides`: `filterable, searchable, sortable, comparable, storefront, admin_grid`.
- Модель групп (миграция `m20260918_000033`): `product_attribute_groups` (+ translations, + attributes с позициями, + `product_attribute_group_channel_settings`); на уровне сервисов есть создание schema-групп и category-групп, а также `group_code`-привязки (зафиксировано в реестре модулей).
- «Effective form» (`effective_forms.rs`): собирает категорию/схему/группы, подгружает локализованные словари опций и `group_label`.

### 1.9 Значения атрибутов, publish-требования, фильтры

- Запись: `write_product_value_patch` + `validate_product_value_patch` (`catalog_schema_service.rs:1125`): проверяет `scope ∈ {product, both}`, соответствие типа значения типу атрибута, bounded JSON, принадлежность опций атрибуту и уникальность опций; `Clear` и пустой multiselect удаляют значение.
- Publish-требования (`values.rs:120–250`): обязательные (required, не disabled) атрибуты категории должны иметь непустое значение, опцию или непустую локализованную строку; проверки — SQL на `is_filled / has_option / has_localized_text`; отказ блокирует `publish` и переход в `Active` через `update_product`; create-with-publish для категорий с обязательными атрибутами запрещён.
- «Detached values»: значения, осиротевшие после смены схемы, сохраняются, показываются в админке и удаляются только явной командой; удаление значений, входящих в действующую схему, запрещено.
- Чтение/фильтрация: `catalog/attribute_filters.rs` (354 строки) — полный набор EXISTS-условий по типам (текст с fallback-локалью, числа, даты, boolean, select/multiselect по `option_id` или `code`), `catalog_attribute_terms.rs` — разбор типизированных значений и термов; тип `json` для фильтрации запрещён.
- Проекции и листинги: `catalog/queries.rs` (админские выборки), `catalog/projection.rs` (`get_product:190`), `catalog/tags.rs`; storefront-листинг: `per_page 1..=48`, админ — ≤100, невалидная пагинация — ошибка; сортировки/фильтры витрины описаны единым owner-контрактом `StorefrontProductListQuery`.

### 1.10 Виртуальные категории

`services/catalog_schema.rs` описывает `VirtualCategoryRuleV1` (условия `eq` / `range`, наборы атрибутов) и парсер с валидацией (непустые значения, корректные диапазоны); `catalog_schema_service/virtual_categories.rs` (97 строк) — только валидация правила. Таблица `virtual_category_product_assignments` создана в `m20260701_000001:404` и имеет tenant-FK, но **не имеет ни одного писателя и ни одного читателя** в рантайме. Материализация членства удалена вместе с прежним Index-проекционным путём (см. `docs/modules/registry.md`, запись Product от 2026-07-02).

### 1.11 Транспорты

- **Native (`#[server]`)**: `rustok-product/admin/src/transport/native_server_adapter.rs` (1432 строки) — 17 серверных функций под `/api/fn/product/admin/…`; storefront — аналогичный набор публичных функций (листинг/деталь/опции поиска) с fallible-резолвом `HostRuntimeContext` и статическими публичными ошибками.
- **GraphQL**: владелец — `rustok-commerce/src/graphql/mutations/catalog.rs` (1580 строк, 36 мутаций): продукт `209/247`, оси `309`, публикация `352`, удаление `382`, варианты `412/453/492`, изображения `524/558/593/626`, relations `659/687/704`, brands `732–853`, bundles `876–1012`, атрибуты `1035/1091`, категории `1131/1252`, схемы `1175/1212/1292/1327/1367`, значения `1408`, detached `1448`. Ключевые продуктовые мутации требуют `$idempotencyKey: String!` (SHA-256-scoped ключ, ≤191 байт, deadline 2 s).
- **REST (commerce)**: storefront-маршруты `/products`, `/products/{id}`, `/regions`, `/shipping-options`; admin-маршруты продукта, включая unpublish (единственный публичный путь снятия с публикации).
- **gRPC**: `rustok-product-transport` — tonic-адаптер трёх операций `ProductCatalogReadPort`, bearer-аутентификация, доверенный authority, loopback-конформность; отдельный процесс `rustok-product-catalog-service` с fail-closed проверкой схемы.
- **FFA (Next)**: `@rustok/product-admin` (9 744 строки TS/TSX) — `api/{products,attributes,bundles,categories,pricing,relations,types}.ts`, `components/products/**`, `pages/{product-editor(682), attributes, bundles, categories}`, маршруты `apps/next-admin/src/app/dashboard/product/{page, actions(434), [productId], attributes, categories, bundles}`.

### 1.12 Порты и FBA

- `ProductCatalogReadPort` (`read_product_projection`, `read_variant_product_projection`, `list_published_products`) — `contract_version = product.catalog_read.v1`, статус `boundary_ready`, консьюмеры: commerce (checkout projection), marketplace-listing, ai-product (деградации `generate_from_prompt_only`, `skip_catalog_enrichment`, `require_operator_review`).
- `ProductCatalogCommandPort` — 13 операций (create/update/delete/publish/unpublish product, variants CRUD, `set_variant_axes`, изображения); `PortCallPolicy::write()` требует deadline и idempotency key (`crates/libs/rustok-api/src/ports.rs:134–172`). Операций смены статуса/архива, EAV и категорий в порту нет.
- `ProductStorefrontHttpReadPort` — read-only, «legacy list» на 100 строк; `ProductStorefrontTagReadPort`.

### 1.13 События, outbox, индексация

- Все продуктовые записи публикуют доменные события до commit: `ProductCreated`, `ProductUpdated`, `ProductPublished`, `ProductDeleted` (варианта/изображения — через `ProductUpdated`).
- `index_refresh.rs` (631 строка) и `index_refresh_publication.rs` мапят события в Index-цели обновления (`product_locale_refresh_target`), включая варианты и переводы; `index_channel_relation*.rs` — конвергенция связи Index↔канал.
- Отдельного `ProductUnpublished` нет (см. `PROD-EVENT-001`).

### 1.14 Админ-UI

Внутри `rustok-product-admin` сосуществуют:

| Компонент | Файл | LOC | Смонтирован? | Замечание |
|---|---|---|---|---|
| `ProductAdmin` (root) | `ui/root.rs` | небольшая | **да**, `/modules/product` | диспетчер: grid / editor / categories / attributes |
| `ProductGridPage` | `ui/product_grid.rs` | 685 | да (через root) | смена статуса: результат отбрасывается (`let _ =`) |
| `ProductEditorPage` | `ui/product_editor.rs` | 1362 | да (через root) | поле «variant axes» декоративно; результат `change_product_status` / `update_product_variant` отбрасывается |
| `AttributesPage`, `CategoriesPage` | `ui/attributes.rs` (810), `ui/categories.rs` | — | да (через root) | создание атрибутов/опций/схем/категорий; привязки — нет |
| `ui/leptos.rs::ProductAdmin` | 2570 | **нет** | полная обработка статуса (error/refresh), SEO-панель, media/variants-панели, detached values, pricing-превью |
| `ui/catalog_admin.rs::ProductAdmin` | — | **нет** | альтернативный список каталога |
| `@rustok/product-admin` (Next FFA) | `apps/next-admin/...` | 9 744 | да, `/dashboard/product**` | самый функциональный редактор |

Транспортный слой Leptos-админки (27 функций в `admin/src/transport.rs`): `fetch_bootstrap`, `fetch_products`, `fetch_product`, `fetch_product_pricing`, `fetch_shipping_profiles`, `fetch_product_attributes`, `fetch_catalog_categories`, `fetch_attribute_schemas`, `fetch_effective_product_form`, `fetch_product_attribute_values`, `create_product`, `create_product_attribute`, `create_product_attribute_option`, `create_catalog_category`, `create_attribute_schema`, `set_category_schema_mode`, `create_product_attribute_schema_group`, `create_category_attribute_group`, `bind_schema_attribute`, `bind_category_attribute`, `save_product_attribute_values`, `clear_detached_product_attribute_values`, `update_product`, `change_product_status`, `delete_product` (+ variant/image/relations/bundles в других файлах). Последние пять операций авторинга схем вызываются только транспортом и не имеют UI-потребителей.

### 1.15 Витрина (storefront)

- **Leptos** (`rustok-product-storefront`, `ui/leptos.rs` смонтирован как `ProductView`): единый owner-контракт листинга (поиск, категория, сортировка, `attribute_filters`, пагинация), карточка выбранного товара, детальная панель с ценами, `ProductPricingDetail` (эффективная цена, price list, канал, скидка), SEO/переводы, i18n `en/ru`.
  - Запросы (`transport/graphql_adapter.rs:13-16`): `StorefrontProductCatalog` (без цены и изображений), `StorefrontCommerceProduct` (переводы, варианты, остаток, `prices`), `StorefrontProductPricing` (эффективная цена/price list/канал), `StorefrontCatalogSearchOptions` (категории + код/метки атрибутов).
- **Next** (`apps/next-frontend/packages/rustok-product`, 1349 строк): `product-grid`, `product-card`, `product-detail-view`, `product-filters`, `catalog-section`; страницы `[locale]/products` и `[locale]/products/[slug]` с SEO-метаданными.
  - Карточка: иконка-заглушка вместо изображения, плашка «Bundle» по тегу, бейдж вендора; **цены нет**.
  - Детальная: медиа-блок — заглушка-иконка; эффективная цена варианта, compare-at, процент скидки, выбор варианта, блок «features» (статические подписи, не атрибуты).
  - Фильтры: поиск, категория, сортировка — фасетов по атрибутам нет.
- **Атрибуты в витрине:** Leptos принимает `attribute_filters` как человекочитаемую строку `code=value;code2=value2` (подсказка «Use filterable attribute codes as code=value, separated by semicolons»), Next — не принимает их вовсе; значения атрибутов товара нигде не отображаются (нет «характеристик» на детальной странице).

### 1.16 Интеграции

- **Pricing** (`rustok-pricing`): `set_price_tier:627`, `set_price_list_tier:758`, `set_scoped_price_tier:823`, `validate_price_tier_quantities:2268` — тиры/min-max количества; pricing-владелец имеет собственный admin-пакет, где тиры поддержаны (native `set_price_tier_with_channel`/`set_price_list_tier_with_channel`, тест `admin_pricing_native_update_variant_price_supports_quantity_tiers`), и GraphQL-мутации `updateAdminPricingVariantPrice` (с `minQuantity/maxQuantity`), `previewAdminPricingVariantDiscount`, `applyAdminPricingVariantDiscount`.
- **Inventory**: остаток и политика варианта; продажа/резерв — через inventory-владельца (`InventoryReservationPort` в commerce). В product-админке видно одно число остатка на вариант.
- **Tax**: `rustok-tax` подключается по `product_type`; поля `tax_class` нет.
- **Media**: `rustok-media` существует, но у `rustok-product` нет зависимости и нет валидации `media_id`.
- **Shipping**: модуля нет; `shipping_profile_slug` — свободная строка (`fetch_shipping_profiles` в Leptos-админке отдаёт список из bootstrap-провайдера хоста).
- **SEO**: продукт зарегистрирован как SEO-target; у категорий есть SEO-переводы, но SEO-регистр экспонирует только `product` (детали — в аудите SEO от 2026-09-30, находка SEO-TARGET-001).
- **Search**: product-owned опции (категории и filterable/sortable-атрибуты) отдаются поисковым UI через публичный DTO/GraphQL (`storefrontCatalogSearchOptions`, `fetch_catalog_search_options`); search-пакеты не импортируют внутренности продукта.

### 1.17 Верификаторы, контракты, тесты

- `package.json`: `verify:product:{admin-boundary, storefront-boundary, catalog-schema, lifecycle-collaboration, runtime-fallback-smoke, grpc-test-attestation}`; всего в `scripts/verify` — 1311 скриптов, включая commerce-product (command-port, retryability, schema-write cutover) и index-product (Postgres-harness).
- Контракты `crates/modules/rustok-product/contracts/` + `contracts/evidence/*.json` (FBA/error-safety), «boundary_ready» с открытыми пунктами: remote profile, host startup, preflight evidence, e2e, sanitized capture.
- Тесты модуля: `tests/` — миграционные/катовер-фикстуры (`module.rs` 24, `postgres_migrations.rs` 649, `category_taxonomy_same_id_cutover.rs` 347, `product_category_closure_storage_retirement.rs` 313). Поведенческие тесты каталога лежат в `rustok-commerce/tests/catalog_service_test.rs` (включая `test_unpublish_product`).

---

## Часть II. Пробелы реализации

### P1 — блокирующие функциональные дефекты

#### PROD-MEDIA-001 (P1). Изображения товара не отображаются нигде в витрине

**Доказательства**

- `grep -rin "media|image|thumbnail" crates/modules/rustok-product/storefront/src` → **0 совпадений**; в запросах витрины (`storefront/src/transport/graphql_adapter.rs:13-14`) поля `images` отсутствуют, хотя GraphQL-тип `GqlProduct` изображения отдаёт (`commerce/src/graphql/types.rs`).
- Next-витрина: `apps/next-frontend/packages/rustok-product/src/components/product-card.tsx:29-36` — «Top Media / Thumbnail Placeholder» с иконкой `Package`/`Boxes`; `product-detail-view.tsx:144-152` — «Media Gallery / Image Showcase» с иконкой-заглушкой; `grep -n "<img\|Image"` по компонентам пакета → ни одного рендера изображения.
- `product_images` (NOT NULL `media_id`, `url`, `alt`, `position`) и `product_image_translations` заполняются командами владельца; `rustok-product` не зависит от `rustok-media`, `media_id` — UUID без проверки существования и без FK.
- Админки сохраняют ассет: FFA `product-media-card.tsx:218` — «Enter a Media module asset ID or direct image URL», Leptos — `add_product_image` (URL/alt).

**Влияние.** Карточка товара в обеих витринах не может показать фотографию; конверсионный сценарий e-commerce не работает, при том что владелец хранит и переводит изображения и участвует ими в Index/Translation. Это главный функциональный разрыв модуля.

**Что нужно.** Ввести в storefront-контракт `images` (любое из: расширить `StorefrontProductListQuery`/`StorefrontProductDetail` или отдельный owner-endpoint), отдавать `url/media_id/alt/position` с учётом канала и локали; в Leptos/Next-витринах отрендерить галерею и thumbnails; задокументировать владельца media-id и добавить проверку существования ассета (или FK/порт к media).

#### PROD-FFA-ATTR-001 (P1). Next-админка сохраняет значения атрибутов без обязательного idempotency key

**Доказательства**

- `apps/next-admin/packages/rustok-product/src/api/products.ts:213` объявляет
  `mutation ProductAdminSaveAttributeValues($productId: UUID!, $locale: String!, $patches: […])` — без `$idempotencyKey`.
- Владелец требует ключ: `rustok-commerce/src/graphql/mutations/catalog.rs:1408` `async fn save_product_attribute_values`, `1411: idempotency_key: String,` → в SDL это `$idempotencyKey: String!`.
- Вызов: `apps/next-admin/src/app/dashboard/product/actions.ts:167-175` — внутри `saveProductAction` для существующего продукта: сначала `updateProductDetail(...)`, затем `saveProductAttributeValues(...)` без обёртки try/catch (в ветке создания — с try/catch и `console.error`).
- Остальные мутации Next-пакета для relations/bundles тоже без ключа — но там сервер ключа не требует (`mutations/catalog.rs:659 add_product_relation`, `876 create_bundle`, `978 add_bundle_item` — параметра нет), поэтому это не ошибка клиента, а иной дефект (см. `PROD-IDEM-001`).

**Влияние.** Любое сохранение карточки из Next-админки, где пользователь изменил значения атрибутов, приводит к ошибке валидации GraphQL на шаге атрибутов: продукт уже изменён (запись 1 прошла), атрибуты — нет, action падает исключением → пользователь видит сбой сохранения при фактически частично применённых изменениях. Это худший из возможных UX-исходов: «ложный отказ + частичная запись».

**Что нужно.** Добавить `$idempotencyKey: String!` в мутацию и генерировать ключ в server action (тот же механизм, что у `updateProductDetail`); обернуть сохранение атрибутов в обработку ошибок и возвращать понятный частичный результат; добавить верификатор, сверяющий «обязательные серверные аргументы vs клиентские мутации» для всего FFA-пакета.

#### PROD-LIST-001 (P1). Листинг каталога не показывает цену

**Доказательства**

- `StorefrontProductListItem` (`src/services/catalog/types.rs:298-309`) и storefront-модель `ProductListItem` (`storefront/src/model.rs`) содержат только `id, status, title, handle, seller_id, vendor, product_type, tags, created_at, published_at` — цены нет.
- Commerce GraphQL `GqlProductListItem` (`commerce/src/graphql/types.rs`) — тоже без цены.
- Запросы листинга (Leptos `graphql_adapter.rs:13`, Next `packages/rustok-product/src/api/products.ts:21-38`) не запрашивают цены.
- Рендер карточек: Next `product-card.tsx`/`product-grid.tsx` — `grep -n "price|Price"` → пусто; Leptos-листинг выводит заголовок/вендора/теги.

**Влияние.** Каталожная страница (главная коммерческая поверхность) не может показать цену ни целиком, ни «от X». Владелец при этом поддерживает price lists, каналы, scoped-цены, compare-at, скидки и тиры — то есть функциональность цен есть, но не доходит до покупателя в списке.

**Что нужно.** Добавить в owner-контракт листинга минимальную цену/цену-от и признак «есть скидка», с учётом канала/валюты/price list (батч-запрос, без N+1), отрендерить в обеих витринах; при необходимости — флаг «скрывать цену до выбора варианта».

### P2 — существенные функциональные пробелы

#### PROD-AXES-001 (P2). Оси вариантов недостижимы из UI

**Доказательства.** `grep -rn "set_variant_axes"` по `admin/src` → 0 вызовов; в `apps/next-admin/packages/rustok-product` нет мутации осей (`grep -n "VariantAxes"` → нет); Leptos-редактор держит `variant_axes_str` («Size, Color») и только отображает его (`ui/product_editor.rs:68, 781-782`), `ui/leptos.rs:1826` лишь читает `product.variant_axes`. Владельческие пути существуют: `commands.rs:1622`, GraphQL `:309`, порт.

**Влияние.** Принятая ADR-модель идентичности вариантов (ось = атрибут + опции, `combination_identity` из БД) не может быть настроена оператором: админка показывает оси, но не сохраняет их; заведение вариативного товара возможно только через прямой вызов GraphQL/REST.

**Что нужно.** Реализовать UI осей на обеих поверхностях (выбор атрибутов с `variant_axis_policy`, набор допустимых опций, порядок), привязать к `set_variant_axes`/`setProductVariantAxes`, покрыть верификатором «UI → команда осей» и тестом дефолтного варианта/комбинаций.

#### PROD-SCHEMA-UI-001 (P2). Авторинг схем не имеет UI

**Доказательства.** В `admin/src/transport.rs` реализованы `set_category_schema_mode:371`, `create_product_attribute_schema_group:388`, `create_category_attribute_group:418`, `bind_schema_attribute:447`, `bind_category_attribute:463` (native + GraphQL fallback); все grep-совпадения этих имён — только транспорт/адаптеры. В Next-пакете `bindSchemaAttribute` экспортирован (`api/attributes.ts:270`), но в маршруте `apps/next-admin/src/app/dashboard/product/attributes/actions.ts` импортируются только `createProductAttribute`, `createProductAttributeOption`, `createProductAttributeSchema`.

**Влияние.** Схема категории в UI не собирается: нельзя переключить режим схемы, создать группу атрибутов, привязать атрибут к схеме/категории. Фактически EAV-движок настраивается только вручную через GraphQL, а «Attributes» страница Next умеет лишь создать атрибут/опцию/схему «в вакууме».

**Что нужно.** Довести UI: список схем и привязок, drag-order, режимы, группы с переводами; добавить в Next-маршрут действия `bindSchemaAttribute` и `set_category_schema_mode`.

#### PROD-VALID-001 (P2). `validation` / `validation_overrides` не применяются

**Доказательства.** Колонки существуют: `product_attributes.validation JSONB` (`m20260701_000001:187`), `validation_overrides JSONB` для `category_attributes` и schema-атрибутов (строки 338, 387). `validate_product_value_patch` (`catalog_schema_service.rs:1125-1204`) читает scope, тип, bounded JSON, принадлежность и уникальность опций — обращений к `validation`/`validation_overrides` нет (проверено сплошным чтением функции).

**Влияние.** Декларативные ограничения атрибутов (min/max, шаблоны, границы для чисел/дат, ограничения по локалям) не защищают данные: значение, формально проходящее тип, может нарушать бизнес-правило; фильтры/сортировки на таких данных дают мусорные результаты.

**Что нужно.** Реализовать движок правил (схема JSON + применение по типу и override-приоритету схема/категория/атрибут), вернуть структурированную ошибку в публичном конверте, добавить верификатор «каждое объявленное правило проверяется в записи».

#### PROD-VCAT-001 (P2). Виртуальные категории: только валидация

**Доказательства.** `virtual_categories.rs` (97 строк) — исключительно валидация правила; `virtual_category_product_assignments` (`m20260701_000001:404`, tenant-FK в `…000002:234-243`) не читается и не пишется рантаймом; материализация удалена вместе с прежним Index-проекционным путём (запись Product в `docs/modules/registry.md`).

**Влияние.** Виртуальные категории нельзя использовать ни в админке (нет членства), ни в витрине (нет выдачи); правило можно создать и провалидировать, но эффекта нет.

**Что нужно.** Либо реализовать материализацию/вычисление членства и чтение (по правилу на лету с индексами либо materialized + пересчёт по событиям), либо объявить функциональность «не поставляется» и убрать мёртвые таблицы/правила из публичного контракта, чтобы не создавать ложных ожиданий.

#### PROD-GROUP-001 (P2). Channel-settings атрибутов и групп не читаются

**Доказательства.** Таблицы `product_attribute_channel_settings` и `product_attribute_group_channel_settings` созданы (`m20260701_000001`, `m20260918_000033`); `grep -rn "is_visible|channel_settings"` по `src` (вне миграций) → нет читателей; `product_attribute_groups` присутствует только как сущность `entities/product_attribute_group.rs` (в `entities/mod.rs:2,13`), сервисов чтения/записи групп в модуле не найдено (создание групп реализовано через schema/category-контракты).

**Влияние.** Per-channel видимость/позиция атрибутов и групп не работает: канал влияет только на видимость продукта (`metadata.channel_visibility`), но не на состав характеристик. Это скрытая потеря функциональности: схема заявляет channel-aware группы, рантайм их игнорирует.

**Что нужно.** Либо подключить channel-settings к effective form и storefront-выдаче (с тестом на два канала), либо задокументировать как «не реализовано» и снять таблицы из активной модели.

#### PROD-PARITY-001 (P2). Разрыв между админками по ключевым операциям

**Доказательства.**

- Цены: Next-пакет вызывает pricing-владельца (`UPDATE_VARIANT_PRICE_MUTATION` c `priceListId/channelId/minQuantity/maxQuantity`, `PREVIEW/APPLY_VARIANT_DISCOUNT`); в Leptos-транспорте **нет ни одной ценовой операции** — цена пишется только как поле payload варианта (`VariantPriceDraft` в `product_editor.rs:452`, `leptos.rs:1752/2046`), price list/канал/тиры недоступны.
- Relations/bundles: Next имеет add/remove/reorder relations, bundle-редактор с позициями; Leptos-транспорт этих операций не имеет (relations/bundles живут отдельными пакетами `rustok-product-relations-admin`).
- Группы атрибутов: Next умеет создавать schema-группы (`api/attributes.ts` через `createProductAttributeSchema`), Leptos — нет (см. `PROD-SCHEMA-UI-001`).
- Смена статуса: Leptos-транспорт `change_product_status` (через generic `updateProduct(status)`), Next — поле `status` в `UpdateProductInput`; оба не предлагают явных «Опубликовать/Снять с публикации/Архивировать» как операций с подтверждением и результатом.

**Влияние.** Один и тот же модуль даёт операторам разные возможности в зависимости от хоста; невозможен перенос регламентов между инсталляциями (Leptos-host vs Next-host), документация «что умеет админка продукта» неоднозначна.

**Что нужно.** Зафиксировать матрицу «операция × поверхность» как контракт, закрыть разрывы (как минимум цены и статус), добавить верификатор паритета на уровне набора мутаций/операций в обоих пакетах.

#### PROD-DUPUI-001 (P2). Три параллельные реализации админки; в смонтированной ветке теряются результаты операций

**Доказательства.** В `rustok-product-admin` три `ProductAdmin`: `ui/root.rs` (смонтирован хостовой кодогенерацией), `ui/leptos.rs` (2570 строк, не смонтирован), `ui/catalog_admin.rs` (не смонтирован) — плюс FFA-пакет Next. В смонтированной ветке результат операции отбрасывается в 8 местах: `ui/product_editor.rs` — `add_product_image:291`, `delete_product_image:327`, `change_product_status:426`, `update_product_variant:461`; `ui/product_grid.rs` — `change_product_status:108, 182`, `delete_product:148, 215` (все `let _ = transport::…`). В несмонтированном `ui/leptos.rs:1504-1540` результат `change_product_status` обрабатывается корректно (view-model, сообщение об ошибке, refresh).

**Влияние.** В живом интерфейсе ошибка смены статуса, обновления/удаления продукта, добавления/удаления изображения не показывается пользователю и не приводит к обновлению данных: оператор видит неизменённый UI и не понимает, что операция отклонена (например, publish-валидация обязательных атрибутов или `CannotDeletePublished` при удалении). Особенно опасно молчаливое удаление продукта: пользователь не может отличить успех от отказа. Дублирование ~3 реализаций увеличивает стоимость поддержки и рассогласование поведения.

**Что нужно.** Выбрать одну реализацию на поверхность, остальные удалить или превратить в тонкие обёртки; запретить отбрасывание результатов (`let _ =`) на уровне верификатора; закрыть закрываемый набор сценариев (статус, вариант, изображения) отображением ошибок и refresh.

#### PROD-IDEM-001 (P2). Relations и bundles обходят идемпотентность и политику вызовов

**Доказательства.** `commerce/src/graphql/mutations/catalog.rs`: `add_product_relation:659` — `rustok_product_relations::services::ProductRelationService::new(db.clone())`; `create_bundle:876` и `add_bundle_item:978` — `rustok_product_bundles::BundleService::new(db.clone())`; ни одна из этих мутаций не принимает `idempotencyKey` (сравните с продуктовыми мутациями, где ключ обязателен и scoped). Ошибки соседних владельцев описаны (`ProductRelationError{RelationAlreadyExists, RelationNotFound, InvalidInput, Outbox…}`, `BundleError{SlugAlreadyExists, ItemNotFound…}`), то есть уникальные конфликты ожидаются, но реиграбельного контракта нет.

**Влияние.** Повтор (retry) после таймаута/двойного клика создаёт конфликт уникальности вместо воспроизведения результата; в мультиагентной/оффлайн-среде нет receipts, по которым можно доказать факт применения. Это прямое нарушение собственного инварианта платформы (`PortCallPolicy::write()` — deadline + idempotency key).

**Что нужно.** Ввести idempotency key и receipt-семантику для relations/bundles (по образцу продуктовых команд), либо явно объявить эти операции неидемпотентными и запретить автоматические повторы на уровне транспорта.

#### PROD-EVENT-001 (P2). Нет события снятия с публикации; `Archived` недостижим командами

**Доказательства.** `src/services/catalog/commands.rs:784` — `unpublish_product` публикует `DomainEvent::ProductUpdated`; вариантов событий в коде — `ProductCreated/Updated/Published/Deleted` (нет `ProductUnpublished`, нет `ProductArchived`). `ProductStatus::Archived` используется как вход в lifecycle-переводах и как фильтр выборок (например, `Column::Status.ne(Archived)`), но выставляется только через generic `updateProduct(status)`: grep по командам владельца не находит ни одной операции архивации; `ports/` и `ProductCatalogCommandPort` архива не имеют.

**Влияние.** Потребители (Index/SEO/уведомления/аналитика) не могут отличить снятие с публикации от обычного обновления; семантика «архив» существует в модели и в фильтрах, но не в жизненном цикле — товар нельзя корректно вывести из оборота штатной операцией с подтверждением и событием.

**Что нужно.** Добавить `ProductUnpublished` и (при подтверждении требования) команду/событие архивации; распространить в порт и оба транспорта; покрыть верификатором событийный контракт.

#### PROD-PORT-001 (P2). Границы порта записи не покрывают жизненный цикл и EAV

**Доказательства.** `ProductCatalogCommandPort` — 13 операций (продукт CRUD + publish/unpublish, варианты, оси, изображения); операций статуса/архива, значений атрибутов, категорий и схем нет. Публично экспортирован только read-порт (FBA registry), запись идёт через owner-сервисы/GraphQL.

**Влияние.** Любая межмодульная запись (ассистенты, импортёры, маркетплейс) вынуждена либо использовать GraphQL/REST владельца, либо напрямую вызывать сервисы — что нарушает декларацию владения и не даёт политики deadline/idempotency по умолчанию.

**Что нужно.** Решить, какие записи вообще допустимы внешне, и расширить порт минимально необходимым (например, смена статуса, патч значений атрибутов), либо явно задокументировать «внешняя запись — только через transport-контракт».

#### PROD-CONC-001 (P2). Нет business revision/CAS, неоднозначные патчи

**Доказательства.** Зафиксировано самим проектом в `crates/modules/rustok-product/docs/reference-hardening-current.md`: (а) у агрегата нет business revision, `update_product` выполняет unconstrained ActiveModel update, а `index_revision` — водяной знак Index, а не редакторский CAS; (б) `UpdateProductInput` использует `Option<T>` для nullable-полей (seller/vendor/type/shipping profile/primary category), из-за чего ветки обновления не различают `Keep / Set / Clear`.

**Влияние.** Два оператора, редактирующие один товар, затирают изменения друг друга без диагностики; API не позволяет явно очистить поле (пустая строка вместо NULL), что ведёт к «полупустым» данным и неожиданным перезаписям при частичных апдейтах.

**Что нужно.** Ввести `version`/revision и predecessor-CAS через сервис, GraphQL/native-транспорты и состояние редактора; ввести явный `Patch<T>` (Keep/Set/Clear) и миграцию клиентов; покрыть тестом «параллельное редактирование → конфликт».

#### PROD-BULK-001 (P2). Нет массовых операций и импорта/экспорта

**Доказательства.** Ни в Leptos-транспорте, ни в FFA-пакете, ни в commerce GraphQL нет операций массового создания/обновления/публикации, CSV/JSON-импорта и экспорта (grep по `bulk|import|export|csv` в product-пакетах даёт только unrelated-совпадения); все операции — поштучные, уникальность handle/SKU строгая (инварианты БД).

**Влияние.** Заведение каталога на тысячи SKU и массовая правка цен/статусов невозможны из продукта; отсутствует и «bulk editor», который есть во всех сравниваемых платформах (Medusa, Shopify, Saleor, BigCommerce).

**Что нужно.** Спроектировать batch-контракт (лимиты, атомарность по строке, отчёт ошибок по строкам, идемпотентность на батч), добавить UI-мастер и верификатор на объём/лимиты.

#### PROD-STORE-001 (P2). Фасетные фильтры недоступны покупателю как UX

**Доказательства.** Leptos: `catalog_controls.rs` принимает `attribute_filters: Vec<String>` и подсказку «Use filterable attribute codes as code=value, separated by semicolons» — ввод вручную. Next: `product-filters.tsx` — только поиск, категория, сортировка. Владелец же умеет отдавать опции атрибутов (`storefrontCatalogSearchOptions`, `attributeOptions`) и фильтровать по `option_id`/`code`, включая локализованный fallback.

**Влияние.** Ключевое конкурентное преимущество (typed EAV + filterable-флаги) не конвертируется в продажи: покупатель не может выбрать «Размер M» или «Цвет: синий» кликом, а Next-витрина вообще не имеет фасетов.

**Что нужно.** Строить фасеты из effective form storefront-флага с counts (батч), рендерить в обеих витринах, заменить ручной ввод `code=value` на UI при сохранении совместимости параметра.

#### PROD-ATTR-STORE-001 (P2). Значения атрибутов не отдаются в витрину

**Доказательства.** В storefront-моделях и запросах нет блока атрибутов (`storefront/src/model.rs`, `graphql_adapter.rs:13-14`, Next `api/products.ts`); детальная страница Next выводит статические «features», Leptos — только переводы/цены. При этом значения хранятся типизированно (`product_attribute_values` + переводы) и участвуют в фильтрах.

**Влияние.** На детальной странице нет «характеристик» товара — покупатель не видит то, что каталогизатор заполняет; SEO/сравнение/спека-таблицы невозможны.

**Что нужно.** Отдавать storefront-safe набор значений (with `storefront` override, локализованные строки/опции) в detail-контракте и рендерить таблицу характеристик; исключить служебные/не-storefront-атрибуты.

#### PROD-SHIP-001 (P2). Доставка — свободная строка без владельца

**Доказательства.** `shipping_profile_slug` — текстовое поле продукта/варианта; `grep -ri "ship" crates/modules | grep -v rustok-product` не находит модуля shipping-профилей (кроме хост-списка в Leptos-админке `fetch_shipping_profiles`); валидации слага против владельца нет. Принятый cutover «fulfillment requirements» (см. `docs/reference-hardening-current.md`) ещё не реализован.

**Влияние.** Неверный профиль не диагностируется до расчёта доставки; цифровые товары не могут быть исключены из доставки типизированно.

**Что нужно.** Владелец профилей доставки (или явная валидация по реестру хоста) + типизированный `fulfillment_requirement` (physical/digital) на продукте и варианте, снапшот в Cart/Order.

#### PROD-DIGITAL-001 (P2). Нет цифровых товаров в контракте

**Доказательства.** Модель не содержит поля типа `digital`/`has_downloads`; `Fulfillment`/`fulfillment.rs` модуля касается выполнения, а не digital-раздачи; в `docs/reference-hardening-current.md` зафиксирована открытая задача «Product must own an explicit digital/physical fulfillment requirement … digital product must have no synthetic default shipping profile».

**Влияние.** Нельзя продавать лицензии/файлы/подписки корректно: они попадут в физические потоки доставки либо потребуют обходных соглашений.

**Что нужно.** Реализовать принятый контракт (тип требования, отсутствие синтетического shipping-профиля, покупаемость в digital-only композиции), покрыть матрицей.

#### PROD-TESTS-001 (P2). Поведенческие тесты владельца живут в другом модуле

**Доказательства.** `crates/modules/rustok-product/tests/` — миграционные/катовер-фикстуры (24/649/347/313 строк); поведенческие тесты каталога — `crates/modules/rustok-commerce/tests/catalog_service_test.rs` (в т.ч. `test_unpublish_product:880`), parity-тесты — в commerce; в песочнице нет `cargo`, поэтому реальные прогоны не выполнялись.

**Влияние.** Границы владения в тестах размыты: регрессии каталога обнаруживаются в чужом модуле, а у продукта как владельца нет собственного поведенческого контура; при выносе модуля в отдельный процесс/репозиторий тесты «останутся» в commerce.

**Что нужно.** Перенести/добавить поведенческие тесты в сам модуль (команды, publish-требования, EAV-инварианты, оси, идемпотентность), оставив в commerce только интеграционные сценарии.

### P3 — гигиена и мелкие дефекты

- **PROD-LOCALE-001.** `apps/next-admin/src/app/dashboard/product/[productId]/page.tsx` читает effective form и значения атрибутов с жёсткой локалью `'en'` (`getCategoryEffectiveForm(opts, …, 'en')`, `fetchProductAttributeValues(opts, productId, 'en')`), а запись в `actions.ts` использует `payload.activeLocale`. На русскоязычной инсталляции редактор показывает значения не той локали, что сохраняет.
- **PROD-IMG-001.** `UPDATE_PRODUCT_IMAGE_MUTATION` (`api/products.ts:192`) объявлена, но обёртки/экспорта нет: из Next нельзя править alt/position изображения (только добавить/удалить/переупорядочить).
- **PROD-META-001.** Часть состояния хранится «тенями» в `metadata` (tags, shipping profile, custom fields) вместо типизированных колонок; это порождает двойные источники правды (зафиксировано и во внутреннем hardening-документе).
- **PROD-RICHTEXT-001.** Значения типа `richtext` — скалярный текст; полноценный rich-text контракт отложен по решению владельца (`docs/implementation-plan.md`, «Open results»).
- **PROD-ENTITIES-001.** `pub mod entities` в `src/lib.rs` оставляет персистентность в публичном API модуля; `ProductStatus` приходит из entity-модуля в public DTO — целевой layout требует выноса.
- **PROD-SEO-001.** Категории каталога имеют SEO-переводы, но SEO-регистр экспонирует только target `product`; централизованные редиректы/карты сайта для категорий отсутствуют (см. аудит SEO от 2026-09-30, SEO-TARGET-001).
- **PROD-ERROR-001.** Адаптер Flex→Product сохраняет catch-all `other => CommerceError::Validation(...)`: инфраструктурные отказы могут классифицироваться как клиентская валидация (зафиксировано в hardening-документе как открытый пункт).

---

## Часть III. Сравнение с популярными платформами

Легенда: ✅ — есть в продукте «из коробки»; ◐ — частично/через расширение; ❌ — нет.

| Возможность | RusTok product | Medusa v2 | Saleor | Shopify | WooCommerce | BigCommerce |
|---|---|---|---|---|---|---|
| Продукт/варианты | ✅ (варианты + оси-атрибуты, `combination_identity` в БД) | ✅ (options/option values) | ✅ | ✅ (до 2048 вариантов с 2025-10-15, макс. 3 опции) | ✅ (одноуровневые вариации) | ✅ (batch-варианты, option rules) |
| Опции как атрибуты (typed) | ✅ (EAV, 11 типов значений, `variant_axis_policy`) | ◐ (options + metadata/PIM) | ✅ (12 типов, включая reference/numeric-with-unit) | ◐ (metafields) | ◐ (атрибуты + вариации) | ◐ (option rules, custom fields) |
| Схемы атрибутов по категориям | ✅ (режимы `inherit/use_schema/clone/custom`, override-флаги, группы) | ❌ (PIM/расширение) | ✅ (product types как наборы атрибутов) | ◐ (metafields + apps) | ◐ (шаблоны атрибутов) | ◐ |
| Обязательные атрибуты до публикации | ✅ (`validate_product_publish_requirements`, блокировка `Active`) | ❌ | ◐ (Required-атрибуты, валидация типа) | ❌ | ❌ | ◐ |
| Мультиканальность (channel-aware) | ◐ (видимость продукта ✅; атрибуты/группы по каналам — таблицы без чтения ❌) | ✅ (sales channels) | ✅ (channels) | ✅ (markets) | ◐ (плагины) | ✅ (channels) |
| Цены: price lists/scoped | ✅ (владелец pricing, price lists, каналы, скидки) | ✅ (price sets + price rules + tiered) | ✅ (price lists, каналы) | ✅ (markets, discounts) | ◐ (роли цен, плагины) | ✅ (price lists) |
| Ценовые тиры (quantity tiers) | ✅ в pricing (admin pricing + GraphQL), ❌ в продуктовой админке | ✅ (tiered pricing в price set) | ◐ (promotions) | ◐ (apps) | ◐ (плагины) | ✅ (tier prices) |
| Витрина: изображения товара | ❌ (не отдаются/не рендерятся) | ✅ | ✅ | ✅ (до 250 media) | ✅ | ✅ |
| Витрина: цена в листинге | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Фасетные фильтры по атрибутам | ◐ (движок фильтров ✅, UI-фасетов нет) | ✅ (extensions) | ✅ (filterable-атрибуты) | ✅ (фильтры/markets) | ◐ (плагины, варьируются) | ✅ |
| Массовые операции / CSV | ❌ | ✅ (bulk editor, CSV import/export) | ◐ (GraphQL bulk, apps) | ✅ (bulk editor, CSV ≤15 МБ) | ✅ (импорт/экспорт, лимит 50 вариаций за прогон) | ✅ (batch API) |
| Медиа-хранилище/интеграция | ❌ (модуль media есть, интеграции продукта нет) | ◐ (внешние URL/файлы) | ✅ (file/image/swatch-атрибуты) | ✅ | ✅ | ✅ |
| Склады/multi-location остаток | ❌ в продукте (владелец inventory) | ✅ (inventory kits/locations) | ✅ | ✅ | ◐ | ✅ |
| Комплекты/бандлы | ✅ (владелец bundles, Next-UI) | ✅ (inventory kits) | ◐ (gift cards/аппсы) | ◐ (apps) | ◐ (плагины) | ◐ |
| Связи товаров (cross/up-sell) | ✅ (владелец relations + UI в Next) | ◐ (related products через extensions) | ✅ (product relations) | ◐ (apps) | ◐ | ✅ |
| Цифровые товары | ❌ (принятый контракт не реализован) | ✅ (variants + digital delivery через плагин) | ✅ (product type + file) | ✅ (digital downloads) | ✅ | ✅ |
| Метаданные/метаполя | ◐ (`metadata` JSONB, тени) | ✅ (metadata) | ✅ (public/private metadata) | ✅ (metafields, не для variant в CSV) | ◐ | ✅ (custom fields) |
| Идемпотентность записи | ✅ для команд продукта; ❌ для relations/bundles/части Next-клиента | ◐ | ◐ | ◐ (API idempotency keys для части операций) | ❌ | ◐ |
| Publish-требования/консистентность статусов | ✅ (черновик/актив/архив, publish-валидация) | ◐ (status draft/published, без required-атрибутов) | ◐ (draft/published + required attrs) | ✅ (draft/active/archived) | ✅ (draft/published) | ✅ |
| Централизованные переводы полей | ✅ (translation targets + lifecycle + CAS-apply) | ◐ (translations модуль) | ✅ (translations + attribute translations) | ◐ (markets) | ◐ (WPML/плагины) | ◐ |
| Оптимистическая блокировка редактора | ❌ (нет revision/CAS) | ◐ | ◐ | ◐ | ❌ | ◐ |

**Выводы по рынку**

1. **Структурная сила RusTok** — владельческая модель EAV + категорийных схем + publish-requirements + tenant-инвариантов БД + переводовых targets. Это ближе к Saleor (product types/атрибуты) и уходит дальше Medusa v2 «из коробки», где атрибуты делаются PIM-расширением.
2. **Провал на последней миле.** Все сравниваемые платформы показывают покупателю изображение и цену в каталоге; RusTok — нет. Потеря происходит не в модели (данные есть), а в storefront-контракте и UI.
3. **Нет операционного масштаба.** Bulk/CSV и batch-операции есть у всех пяти конкурентов; без них каталог 10k+ SKU не администрируется, несмотря на бенчмарки пагинации на 1–3 млн строк.
4. **Атрибуты не доходят до покупателя: ни в фильтрах-фасетах, ни в характеристиках товара.** Тем самым самая сильная часть модели почти не монетизируется.
5. **Жизненный цикл неполон** относительно Shopify/BigCommerce: нет операции «снять с публикации/архивировать» как первоклассного действия с событием и UI-подтверждением (в RusTok это побочный эффект generic-update).
6. **Цифровые товары и доставка** — зияющие пробелы против Shopify/WooCommerce/BigCommerce/Medusa; частично признано принятым cutover-планом.
7. **Инженерная гигиена выше среднего** (идемпотентность ядра, outbox-до-commit, верификаторы границ, FBA/gRPC), но именно на периферии (relations/bundles/один FFA-клиент) она нарушена — а «периферия» здесь совпадает с деньгами (мерчандайзинг, значения атрибутов).

---

## Часть IV. Приоритизированный план

**Немедленно (P1, до следующего релиза витрины/админки)**

1. `PROD-FFA-ATTR-001` — вернуть `idempotencyKey` в мутацию значений атрибутов и обернуть частичное сохранение в обработку ошибок; верификатор «обязательные серверные аргументы ↔ клиентские мутации».
2. `PROD-MEDIA-001` — добавить изображения в storefront-контракт (листинг + деталь) и отрендерить галерею/миниатюры в Leptos- и Next-витринах; определить проверку `media_id` против media-владельца.
3. `PROD-LIST-001` — минимальная цена (и признак скидки) в owner-контракте листинга, рендер в карточках обеих витрин.

**Короткий цикл (P2, ядро продукта)**

4. `PROD-AXES-001` и `PROD-SCHEMA-UI-001` — UI осей вариантов и авторинга схем (режим, группы, привязки) на смонтированной Leptos-поверхности и в Next-маршрутах.
5. `PROD-VALID-001` — движок правил `validation`/`validation_overrides` + тесты.
6. `PROD-DUPUI-001` — консолидация админок и запрет отбрасывания результатов операций (верификатор `let _ =` на критичных вызовах).
7. `PROD-IDEM-001`, `PROD-EVENT-001`, `PROD-PORT-001` — идемпотентность relations/bundles, событие снятия с публикации/архива, расширение порта записи.
8. `PROD-CONC-001` — revision/CAS и явные `Keep/Set/Clear`-патчи.
9. `PROD-STORE-001`, `PROD-ATTR-STORE-001` — фасеты и характеристики товара в витринах.
10. `PROD-VCAT-001`, `PROD-GROUP-001` — либо реализовать материализацию виртуальных категорий и channel-settings, либо официально снять с модели.

**Средний цикл**

11. `PROD-BULK-001` — batch/CSV-контур с отчётом по строкам.
12. `PROD-DIGITAL-001`, `PROD-SHIP-001`, `PROD-TAX-001` — fulfillment-требование, владелец профилей доставки, `tax_class`.
13. `PROD-PARITY-001` — матрица паритета админок + цены/скидки/статус в Leptos-админке.
14. `PROD-TESTS-001` — перенос поведенческих тестов владельца в модуль.
15. `PROD-LOCALE-001`, `PROD-IMG-001`, `PROD-META-001`, `PROD-RICHTEXT-001`, `PROD-ENTITIES-001`, `PROD-SEO-001`, `PROD-ERROR-001` — гигиена.

---

## Приложение A. Что проверено и что нет

**Проверено статически (с точными ссылками в тексте):** состав и размер модуля; манифест и кодогенерация монтирования UI обоих хостов; 35 миграций и инварианты БД; команды владельца и их порядок/ограничения; порты и политика вызовов; 36 GraphQL-мутаций и их сигнатуры; REST-маршруты storefront; FBA-регистр; все три Leptos-«админки» и FFA-пакет (включая сверку обязательных аргументов мутаций); обе витрины и их запросы/рендер; интеграции pricing/inventory/tax/media/shipping/search/SEO; покрытие верификаторами; отсутствие вызовов у пяти schema-authoring операций и у команды осей.

**Выполнено в песочнице:** 5 продуктовых JS-верификаторов (`verify-product-{admin-boundary,storefront-boundary,catalog-schema,lifecycle-collaboration,runtime-fallback-smoke}`) — все успешно.

**Не проверено (и не может быть в этом окружении):** компиляция, clippy, любые Rust-тесты (`cargo` отсутствует); рантайм-поведение GraphQL/native-эндпоинтов; конкурентные сценарии; реальные планы запросов; поведение `media_id` при удалении ассета в media; фактический вид смонтированных UI-страниц в браузере. Утверждения о поведении — следствия кода, а не наблюдения.

**Ужесточение после проверки (чтобы не повторять ошибочных выводов).** При аудите специально перепроверялись четыре подозрения: (1) обход publish-валидации через `updateProduct(status=ACTIVE)` — **не подтверждён**, валидация вызывается; (2) отсутствие управления статусом в Leptos-админке — **не подтверждено**, `change_product_status` смонтирован (проблема в отбрасывании результата, `PROD-DUPUI-001`); (3) отсутствие цены в витрине вообще — **не подтверждено**, детальная страница цену показывает (дефект только в формате и в листинге); (4) отсутствие монтирования продуктовой админки — **не подтверждено**, монтирование генерируется сборкой хоста (`/modules/product` → `ui/root::ProductAdmin`), а несмонтированными являются альтернативные реализации внутри пакета.

---

## Часть V. Ход устранения (remediation log)

Часть V ведётся по мере устранения находок и обновляется вместе с кодом; источник истины по статусам — код и верификаторы, а не этот журнал.

### Волна 1 — P1

| ID | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| `PROD-FFA-ATTR-001` | Устранено | `SAVE_ATTRIBUTE_VALUES_MUTATION` получила обязательный `$idempotencyKey`; ключ генерируется на каждый вызов; `saveProductAction` различает частичный успех и откат (создание откатывается, обновление — нет) и возвращает понятный результат вместо исключения | `apps/next-admin/packages/rustok-product/src/api/products.ts`, `apps/next-admin/src/app/dashboard/product/actions.ts`, `scripts/verify/verify-product-storefront-media-price.mjs` |
| `PROD-MEDIA-001` | Устранено | Владелец отдаёт media-снапшот каталога: `StorefrontProductListImage { media_id, url, alt_text, position }` в листинге и `images` в детали; батч-запрос медиа на страницу (`position,id`), alt через `resolve_image_alt_text`; Commerce GraphQL отдаёт `primaryImage`/`images`; Leptos- и Next-витрины запрашивают и рендерят изображения (карточка, галерея, миниатюры). `media_id` валидируется по медиа-владельцу: Product-owned `ProductMediaAssetReadPort` + декоратор `ProductMediaValidatedCommandPort` (валидация до транзакции), хост-адаптер `ProductMediaAssetProvider` над `MediaAssetReadPort`, композиция в `attach_commerce_provider_registries`; без провайдера политика честно помечается как `opaque_references` | `crates/modules/rustok-product/src/media_asset_read_port.rs`, `crates/modules/rustok-product/src/runtime.rs`, `crates/modules/rustok-product/src/services/catalog/{types,queries,projection}.rs`, `crates/modules/rustok-product/storefront/src/{model,core,ui/leptos,transport/*}.rs`, `crates/modules/rustok-commerce/src/graphql/{types,product_catalog}.rs`, `apps/server/src/services/{product_media_asset_validation,commerce_provider_runtime}.rs`, `apps/next-frontend/packages/rustok-product/src/**` |
| `PROD-LIST-001` | Устранено | Снапшот «цены от» в листинге: `StorefrontProductListPrice { currency_code, amount, compare_at_amount, on_sale }`, батч цен по вариантам страницы, приоритет scoped-to-channel → channel-neutral → any, пропуск price-list и количественных тиров, `min_by(amount)`; опциональный фильтр валюты на всём пути (GraphQL `currencyCode`, native `currency_code`, Leptos `?currency=`, Next `?currency=`); рендер «от / from», зачёркнутая compare-at, бейдж «Скидка / Sale» | те же файлы + `apps/next-frontend/src/app/[locale]/products/page.tsx` |

**Верификаторы волны 1.** Добавлен `scripts/verify/verify-product-storefront-media-price.mjs` с регрессионными тестами `verify-product-storefront-media-price.test.mjs` (10/10) — фиксирует цепочку владелец → GraphQL → Leptos-витрина → Next-витрина, обязательный `$idempotencyKey`, частичный результат сохранения атрибутов и валидацию media-ссылок (включая запрет импорта Media-крейтов в Product). Обновлён `verify-product-storefront-catalog-native-error-safety.mjs` и его evidence JSON: контракт каталога и DTO листинга осознанно изменены (валюта, `primaryImage`/`priceFrom`). Все ранее существовавшие падения продуктовых/медийных верификаторов сохранены без изменений (сверено с pristine-деревом `HEAD`): новых падений нет.

### Волна 2 — P2 (начата): `PROD-DUPUI-001` — консолидация админок и транспортная граница

| ID | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| `PROD-DUPUI-001` | Устранено | (1) Все 8 молчаливых отбрасываний (`let _ = transport::…`) заменены разбором результата: ошибки показываются оператору, успех/частичный успех обновляет данные (редактор: изображения, смена статуса, обновление варианта; грид: массовые и быстрые статус/удаление) — `ui/product_editor.rs`, `ui/product_grid.rs`. (2) Собрана каноническая транспортная граница `admin/src/catalog_transport.rs`: публичные обёртки чтений с `GraphqlReadContext` (bounded-диагностика), native-first листинг `adminProductCatalog` с GraphQL-фолбэком, `CatalogSearchOptionsErrorContext` для публичного String-контракта, `GraphqlMutationContext` для четырёх жизненных записей продукта и `GraphqlFallbackMutationContext` (открытая граница) для авторских мутаций; `transport.rs` остался приватным шлюзом. (3) Смонтированные страницы (`ui/root.rs` → `product_grid/product_editor/attributes/categories`) теперь ходят через фасад, а не через приватный шлюз. (4) Смонтированный листинг получил владельческий контракт списка: URL-управляемые фильтры (категория, типизированные атрибуты, `sort_by`/`sort_direction`), `provide_context(ProductAdminListInput)` и native-first загрузка. (5) В смонтированный редактор перенесён типизированный FFA-редактор значений атрибутов (`ProductAttributeValuesSection`: группы, обязательность, disabled, частичные патчи, detached-значения с очисткой) — раньше он существовал только в несмонтированном `ui/leptos.rs` | `crates/modules/rustok-product/admin/src/catalog_transport.rs`, `.../transport/graphql_error_safety.rs`, `.../transport/admin_catalog_{native,graphql}.rs`, `.../ui/{product_grid,product_editor,attributes,categories,catalog_admin,leptos}.rs`, `.../ui/leptos.rs` (`ProductAttributeValuesSection`), `crates/modules/rustok-product/admin/src/core.rs` |

**Верификаторы волны 2.** Добавлен `scripts/verify/verify-product-admin-operation-results.mjs`: запрещает отбрасывание результатов жизненных операций в `ui/*.rs`, фиксирует владельческий контракт списка в смонтированном гриде, монтирование FFA-секции в редакторе и полноту общего Leptos-адаптера. В `package.json` впервые зарегистрированы «забытые» контракты — `verify:product:admin-category-sort`, `verify:product:admin-read-diagnostics` (диагностика чтений + мутаций), `verify:product:admin-mutation-error-safety`, `verify:product:admin-operation-results` — и включены в цепочку `verify:ecommerce:fba`.

**Исправления контрактов (осознанные, чтобы верификатор отражал живой код, а не незавершённый рефакторинг).** (а) `verify-product-admin-category-sort.mjs`: маркер корня Commerce GraphQL переведён с прямого вызова сервиса владельца `list_admin_products_with_query` на санкционированный шов `list_admin_products(` (чтение идёт через `ProductCatalogReadPort`). (б) `verify-product-admin-graphql-read-diagnostic-safety.mjs`: состав UI-файлов расширен с одного `ui/leptos.rs` до набора смонтированных страниц — теперь контракт проверяет живую поверхность, а не только референсный монолит. (в) `verify-product-admin-primary-mutation-error-safety.mjs`: приватная делегация мутаций выражена через retry-слой (`retry::create_product`/… + `retained_caller_key` + `mark_lifecycle_succeeded` + `product_lifecycle_graphql::*`), то есть контракт усилен до идемпотентного контура вместо прежнего прямого вызова `graphql_adapter`. (г) `verify-product-admin-catalog-options-error-safety.mjs` и `verify-search-ui-boundary.mjs` проходят без правок: обёртка доступна как `catalog_transport::fetch_catalog_search_options`, а корневой реэкспорт совместимости сохранён.

**Что осталось по этой находке.** (1) Смонтированные страницы и монолит `ui/leptos.rs` сосуществуют: монолит (и обёртка `ui/catalog_admin.rs`) не смонтированы кодогенерацией хоста, но остаются референсной поверхностью, на которую ссылаются контракты FFA-миграции (`verify-ffa-ui-migration-contract.mjs` требует `ui/leptos.rs` для `core_transport_ui`); удаление/слияние требует отдельного прохода по этим контрактам. (2) Три «незаленденных» контракта указывают точную целевую раскладку файлов, которой ещё нет в дереве (все три падают ENOENT и на pristine-дереве — это не регрессии):
  - `verify-product-admin-lifecycle-retry-consumer.mjs` ожидает `admin/src/catalog_transport_retry.rs` и объявление `#[path = "catalog_transport_retry.rs"] mod transport;` внутри `catalog_transport.rs` (retry-осознанный транспорт как подмодуль фасада);
  - `verify-product-admin-lifecycle-retry-identity.mjs` — **закрыто в волне 3**: идентичность ретраев перенесена из `transport/lifecycle_retry_identity.rs` в `admin/src/lifecycle_retry_identity.rs` и объявлена в корне крейта;
  - `verify-product-admin-fallback-mutation-error-safety.mjs` ожидает выделенные `transport/graphql_fallback_mutations.rs` + `transport/graphql_fallback_mutation_error_safety.rs` (сейчас политика fallback-мутаций реализована в `transport/graphql_error_safety.rs` и применяется в `transport.rs`), а `verify-commerce-product-schema-write-consumer-cutover.mjs` — полный schema-write cutover (`product_schema_graphql.rs`, `schema_retry_identity.rs`).
  Раскладка этих файлов относится к волнам «идемпотентность/ретраи» и «схема записи», а не к слиянию поверхностей; контракты сохранены как есть, чтобы следующая волна land'ила их по спецификации.
  (3) Периодическая сверка «несмонтированная поверхность не растёт» автоматизирована в `verify-product-admin-operation-results.mjs` (запрет корневых алиасов несмонтированных композиций + требование владельческого контракта списка и FFA-секции на смонтированной поверхности).

**Унаследованные падения верификаторов (не регрессии).** На baseline-коммите ветки падают 33 верификатора; все они падают идентично и на pristine-дереве `HEAD`. Три из них относятся к продуктовой админке и описывают целевую раскладку, которой в дереве ещё нет: `verify-product-admin-lifecycle-retry-consumer.mjs` (ожидает `admin/src/catalog_transport_retry.rs` и `#[path = "catalog_transport_retry.rs"] mod transport;`), `verify-product-admin-fallback-mutation-error-safety.mjs` (ожидает `transport/graphql_fallback_mutations{,_error_safety}.rs`), `verify-commerce-product-schema-write-consumer-cutover.mjs` (ожидает полный schema-write cutover). Один — `verify-product-catalog-attribute-filters.mjs` — падает из-за того, что владельческий typed-EAV фильтр (`cannot be used in attribute_filters`, `pub attribute_filters: Vec<String>` в GraphQL-корнях) относится к находке `PROD-STORE-001` и ещё не реализован; это ожидаемое падение до соответствующей волны. Волна 3 закрыла одно из этих падений (`verify-product-admin-lifecycle-retry-identity.mjs`, перенос контракта идентичности ретраев в корень крейта).

**Ограничение проверки.** Rust-код не компилировался и не запускался (`cargo` в песочнице отсутствует); корректность подтверждается статическим анализом, синтаксическими гейтами (tree-sitter разбор Rust и `node --check` для JS/TS) и верификаторами. Требуется `cargo fmt --all -- --check`, `cargo check`/`cargo clippy`/`cargo test` и визуальная проверка витрин владельцем. Для `PROD-VALID-001` добавлена прямая зависимость `regex = "1.12"` в `crates/modules/rustok-product/Cargo.toml` и соответствующее ребро в `Cargo.lock` (пакет уже был в дереве через `rustok-core`); ручная правка lock-файла требует подтверждения обычным `cargo`-запуском.

### Волна 3 — P2: `PROD-AXES-001`, `PROD-SCHEMA-UI-001` и `PROD-VALID-001` — недостижимые команды владельца и неприменяемые правила валидации

| ID | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| `PROD-AXES-001` | Устранено | (1) Транспортный контур оси вариантов: слот `ProductAdminLifecycleOperation::SetVariantAxes` (`"set-variant-axes"`) в ретрай-идентичности, `SET_VARIANT_AXES_MUTATION` (`setProductVariantAxes(idempotencyKey, productId, input)`) и `set_variant_axes` в GraphQL-адаптере с типизированным публичным сообщением, ретрай-обёртка `retry::set_variant_axes` с `retained_caller_key`/`mark_lifecycle_succeeded` и интентом `operation=set-variant-axes;…;axes=<attribute_id>@<position>:<allowed_option_ids>|…`, реэкспорт через `transport.rs` и `catalog_transport.rs`. (2) Mounted Leptos: `ProductVariantAxesSection` (props `product_id`, `locale`, `on_saved`) — сиды из сохранённых осей, иначе из схемы (`defaultVariantAxis && variantAxisPolicy != "forbidden"`), ряды атрибутов с позицией, чекбоксы допустимых опций словаря, сохранение через `catalog_transport::set_variant_axes` (пустой набор = снятие осей), ошибки через `build_product_admin_error_copy(...).save_product_failure`. Секция смонтирована в `ui/product_editor.rs` в ветке `ProductKind::Variable` через `current_edit_id.clone().map(|pid| …)`, без устаревшего read-only инпута `variant_axes_str`. (3) Next-админка: `setVariantAxes` + `SET_VARIANT_AXES_MUTATION` (`crypto.randomUUID()` как ключ идемпотентности на вызов), типы `VariantAxisConfig`/`SetVariantAxesInput`/`ProductDetail.variantAxes`, карточка `ProductVariantAxesCard` (порядок осей, допустимые значения, очистка), проброс `onSetVariantAxes` из `product-editor-page.tsx` в маршрут через `setVariantAxesAction` и `revalidatePath` | `crates/modules/rustok-product/admin/src/lifecycle_retry_identity.rs`, `.../transport/{product_lifecycle_graphql,retry,transport}.rs`, `.../catalog_transport.rs`, `.../ui/{leptos,product_editor}.rs`, `apps/next-admin/packages/rustok-product/src/{api/{products,types}.ts,components/products/product-variant-axes-card.tsx,pages/product-editor-page.tsx,index.ts}`, `apps/next-admin/src/app/dashboard/product/{actions.ts,[productId]/page.tsx}` |
| `PROD-SCHEMA-UI-001` | Устранено | Пять команд авторинга схем (режим схемы категории, группы схемы, группы категории, привязка атрибута к схеме, привязка атрибута к категории) были достижимы только сырым GraphQL/REST. Добавлена смонтированная карточка `ProductSchemaAuthoringCard` (вкладка «Схемы» страницы `ui/attributes.rs`): по одной секции на команду, единый обработчик результата `apply_schema_authoring_result` (принято / отклонено владельцем / ошибка транспорта через `build_product_admin_error_copy`), черновики `SetCategorySchemaModeDraft`, `ProductAttributeSchemaGroupDraft`, `CategoryAttributeGroupDraft`, `BindSchemaAttributeDraft`, `BindCategoryAttributeDraft`, режимы связывания `addition|override|removal` по CHECK-констрейнту владельца, перечитывание собственных владельческих чтений через фасад и рефреш после успеха | `crates/modules/rustok-product/admin/src/ui/{leptos,attributes}.rs` |
| `PROD-VALID-001` | Устранено | (1) Движок правил (`attribute_validation.rs`): закрытая схема из 15 правил — `minLength`/`maxLength`, `pattern` (крейт `regex`, ограничение 512 байт), `min`/`max`, `minDate`/`maxDate`, `minDatetime`/`maxDatetime`, `minSelections`/`maxSelections`, `options`, `required`, `requiredLocales`, `maxJsonBytes`; неизвестные ключи отклоняются на авторинге, некорректные известные — fail-closed, `null` в override снимает унаследованное правило, границы и диапазоны проверяются при разборе. (2) Приоритет источников: базовые правила атрибута (`product_attributes.validation`) ← переопределения схемы и категории (`validation_overrides`); слияние выполняется один раз в `load_effective_form_for_category_in` (`load_effective_attribute_validation` + `merge_product_attribute_validation`), результат лежит в `AttributeBinding.validation`, поэтому чтение и запись используют один и тот же эффективный объект. (3) Применение в записи: `validate_product_value_patch` и `validate_variant_value_patch` получают объединённые правила и вызывают `validate_attribute_value_rules` по типу значения (текст/длина/шаблон, числа, даты, выборки, размер JSON); оба пути записи берут правила из эффективной формы, а загрузчики определений теперь выбирают колонку `validation` (иначе `PROD-VALID-001` вернулся бы молча). (4) Публикация: `required` и `requiredLocales` сделаны обязательными условиями публикации (`validate_product_publish_requirements_in`, `validate_new_product_publish_requirements`), покрытие локалей проверяется отдельным запросом `load_missing_required_locales` и только при наличии правила (нулевые накладные расходы для тенантов без него). (5) Авторинг: три команды (`create_attribute`, оба `bind_*`) отклоняют неизвестные ключи и разбирают правила при создании; GraphQL-входы `CreateProductAttributeInput.validation`, `BindSchemaAttributeInput.validationOverrides`, `BindCategoryAttributeInput.validationOverrides` больше не отбрасываются мутациями (раньше правило нельзя было объявить через API вообще). (6) Чтение: `ProductEffectiveFormAttributeProjection.validation` и GraphQL `GqlProductEffectiveFormAttribute.validation` отдают эффективные правила клиентам. (7) Публичная ошибка: нарушение правила возвращает выделенный код `PRODUCT_ATTRIBUTE_VALIDATION` и копию из закрытого набора сообщений; в диагностике вариант классифицируется как `attribute_validation`, generic-маппинг `PRODUCT_VALIDATION` сохранён. | `crates/modules/rustok-product/src/services/catalog_schema_service/{attribute_validation.rs,effective_forms.rs,values.rs,values/variant.rs}`, `.../services/{catalog_schema.rs,catalog_schema_service.rs}`, `.../catalog_schema_read_port.rs`, `.../public_error.rs`, `crates/modules/rustok-product/Cargo.toml`, `Cargo.lock`, `crates/modules/rustok-commerce/src/graphql/{types.rs,query.rs,mutations/catalog.rs}` |
| `PROD-DUPUI-001` (дополнение) | Устранено | (1) Идентичность ретраев вынесена в корень крейта (`src/lifecycle_retry_identity.rs`, `pub(crate) mod lifecycle_retry_identity;`), `transport` сохраняет путь через `pub(crate) use` — единственный ранее падавший верификатор `verify-product-admin-lifecycle-retry-identity.mjs` теперь проходит. (2) Несмонтированные композиции документированы как референсные (`README.md` пакета + doc-комментарии в `ui/leptos.rs`, `ui/catalog_admin.rs`) с явным правилом «новый функционал — только на смонтированных страницах и общих секциях». (3) Типизированный редактор значений атрибутов больше не теряет ссылки на удалённые опции словаря: для `select`/`multiselect` сохранённый `optionId` вне схемы показывается явной опцией «(нет в словаре)», а для `multiselect` его можно снять; копия вынесена в `core.rs` (`product.attributes.valuesMissingOption`) | `crates/modules/rustok-product/admin/src/{lifecycle_retry_identity.rs,lib.rs,transport.rs,core.rs}`, `.../ui/{leptos,catalog_admin}.rs`, `crates/modules/rustok-product/admin/README.md` |

**Верификаторы волны 3.** Добавлены верификаторы и включены в цепочку `verify:ecommerce:fba`:
- `scripts/verify/verify-product-variant-axes-ui.mjs` (`verify:product:admin-variant-axes`) — запрещает возврат read-only заглушки осей и фиксирует всю цепочку «владелец → Commerce GraphQL → адаптер → ретрай-идентичность → смонтированный Leptos → Next-админка»;
- `scripts/verify/verify-product-admin-schema-authoring.mjs` (`verify:product:admin-schema-authoring`) — фиксирует владельческие команды и порт записи, резолверы Commerce, типизированные `GraphqlFallbackMutationContext` в фасаде (ровно 11 fallback-мапперов), native-first `transport.rs` и монтирование карточки на странице атрибутов.
- `scripts/verify/verify-product-attribute-validation-rules.mjs` (`verify:product:attribute-validation-rules`, тесты — `test:verify:product:attribute-validation-rules`, 10/10) — по одному утверждению на каждое из 15 объявленных правил (объявление + применение + покрытие тестами), приоритет override-слияния, применение в обеих ветках записи, гейт публикации по `required`/`requiredLocales`, авторинг через владельческие команды и GraphQL, выдача эффективных правил в чтении и структурированная публичная ошибка; запрещает любой загрузчик определений без колонки `validation` и потерю правила в `createProductAttribute`/`bind*`.
Прогон волны 3 (движок правил): сплошное сравнение всех 54 верификаторов `verify-product-*.mjs` в рабочем дереве и на pristine-экспорте `HEAD` даёт **единственное** расхождение — новый `verify-product-attribute-validation-rules` (PASS в дереве, отсутствует на pristine); `verify-search-ui-boundary`, `verify-product-catalog-attribute-filters`, `verify-product-admin-lifecycle-retry-consumer`, `verify-product-fallback-mutation-error-safety`, `verify-commerce-product-schema-write-consumer-cutover` и прочие падения остаются унаследованными (идентичны pristine). Прогон по живому дереву: `verify-product-admin-boundary` (+14/14 тестов), `admin-category-sort` (+5/5), `admin-read-diagnostics` (диагностика чтений, primary-read, category-read), `admin-mutation-error-safety` (primary-mutation, catalog-options), `admin-operation-results`, `catalog-controls-plan-sync` (+9/9), `catalog-attribute-filters` (+4/4), `storefront-media-price`, `catalog-schema`, `search-ui-boundary`, `ffa-ui-migration-contract`, оба новых верификатора, а также `verify-product-admin-lifecycle-retry-identity` — PASS. Не прошли (минус один против прошлой волны): `verify-product-admin-lifecycle-retry-consumer` (ожидает незаленденную раскладку `catalog_transport_retry.rs`) и `verify-product-catalog-attribute-filters` — оба падают идентично на pristine-дереве `HEAD`, то есть это унаследованный долг, а не регрессия; причина по `attribute-filters` зафиксирована в Приложении A.

### Волна 5 — P2: `PROD-EVENT-001`, `PROD-IDEM-001`, `PROD-PORT-001` — события жизненного цикла, идемпотентность relations/bundles, граница записи

| ID | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| `PROD-EVENT-001` | Устранено | (1) В закрытый контракт `DomainEvent` добавлены два корневых события схемной версии 1: `ProductUnpublished { product_id }` и `ProductArchived { product_id }` (`event_type` = `product.unpublished` / `product.archived`, классификация `affects_index`, валидация `product_id`, реестр схем, канонический тест-сэмпл). (2) `unpublish_product` больше не публикует generic `ProductUpdated`: событие снятия с публикации стало именованным фактом. (3) Добавлена полноценная команда архивации `CatalogService::archive_product` (статус `Archived`, очистка `published_at`, событие `ProductArchived`); `update_product` при смене статуса публикует соответствующий lifecycle-факт (`Active` → `ProductPublished`, `Archived` → `ProductArchived`, уход из `Active` → `ProductUnpublished`), а `ProductUpdated` остаётся общим «что-то изменилось». (4) Операция `archive_product` проведена через `ProductCatalogCommandPort` (14 операций), декоратор media-валидации, Commerce GraphQL (`unpublishProduct`/`archiveProduct` с обязательным `idempotencyKey`) и REST (`POST /admin/products/{id}/{unpublish,archive}` + `archive_product` в OpenAPI). (5) Потребители обновлены: `rustok-search` (handles/projection/plan), `product_locale_refresh_target`, `ProductIndexRefreshPublication`. (6) Снятие с публикации и архивация гонят локальные и индексные обновления через тот же lifecycle-ledger, что и публикация. (7) Требование регенерации digest-артефакта зафиксировано в документации контракта | `crates/libs/rustok-events/src/{types/domain_event.rs,types/validation.rs,schema.rs}`, `crates/libs/rustok-events/tests/canonical_contracts.rs`, `crates/modules/rustok-product/src/services/catalog/commands.rs`, `crates/modules/rustok-product/src/{catalog_command_port.rs,media_asset_read_port.rs}`, `crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs`, `crates/modules/rustok-commerce/src/controllers/{products.rs,admin/products.rs,admin/mod.rs}`, `crates/modules/rustok-commerce/src/openapi.rs`, `crates/modules/rustok-search/src/ingestion.rs`, `crates/modules/rustok-product/src/services/{index_refresh.rs,index_refresh_publication.rs}`, `crates/modules/rustok-product/docs/product-unpublish-archive-events.md` |
| `PROD-IDEM-001` | Устранено | (1) Relations: новый модуль владельческих receipts (`ProductRelationCommandContext`, `ProductRelationCommandError`) — `admit` в tenant-scope владельца `product_relations`, запись в одной транзакции с `create_relation_in_tx`, `complete` внутри этой транзакции, `fail` для терминальных отказов, декодирование реплея; обычный порт-метод переиспользует тот же транзакционный core (единственная реализация записи). (2) Bundles: то же для `create_bundle_idempotent` и `add_bundle_item_idempotent` (владелец `product_bundles`, операции `create_bundle`/`add_bundle_item`); `create_bundle_in_tx`/`add_bundle_item_in_tx` — общий core для порта и receipt-команды, проверка уникальности слага перенесена внутрь транзакции. (3) Commerce GraphQL: три мутации (`addProductRelation`, `createBundle`, `addBundleItem`) получили обязательный аргумент `idempotencyKey`; ключ скоупится digest-ом (tenant, actor, операция, субъект) и передаётся в receipt; ошибки маппятся на закрытый словарь (`PRODUCT_RELATION_*`/`PRODUCT_BUNDLE_*`, `IDEMPOTENCY_KEY_CONFLICT`, `CATALOG_OPERATION_IN_PROGRESS`, `BAD_USER_INPUT`, `retryable`). (4) Native-админка (FFA): адаптеры relations/bundles больше не отбрасывают уже принимаемый `idempotency_key` — команды `Add`/`Create`/`AddItem` идут через receipt-команды владельца, ошибки возвращаются bounded-копией; GraphQL-адаптер тех же пакетов передаёт `$idempotencyKey` в мутацию, FFA-клиент Next генерирует ключ на каждую запись (`crypto.randomUUID()` в `api/relations.ts` и `api/bundles.ts`) | `crates/modules/rustok-product-relations/src/services/{receipts.rs,relation_service.rs,mod.rs}`, `crates/modules/rustok-product-relations/src/lib.rs`, `crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs`, `crates/modules/rustok-product-bundles/src/services/{receipts.rs,bundle_service.rs,mod.rs}`, `crates/modules/rustok-product-bundles/src/lib.rs`, `crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs`, `crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs` |
| `PROD-PORT-001` | Устранено (решение зафиксировано) | Принято и записано решение о границе записи: Product — единственный писатель агрегата; внешняя запись возможна только через владельческие порты (`ProductCatalogCommandPort` — 14 операций жизненного цикла/вариантов/изображений, включая новую `archive_product`; `ProductCatalogSchemaWritePort` — 13 receipt-связанных операций авторинга EAV и значений; receipt-команды relations/bundles) либо через transport-контракт владельца; прямой вызов владельческих сервисов из чужого крейта запрещён. Значения атрибутов сознательно не переносятся в command port (у них уже есть свой receipt-порт), поэтому минимальное расширение — ровно одна операция `archive_product` | `crates/modules/rustok-product/docs/write-boundary.md`, `crates/modules/rustok-product/src/catalog_command_port.rs`, `crates/modules/rustok-product/src/catalog_schema_write_port.rs` |

**Верификаторы волны 5.** Добавлены и включены в цепочку `verify:ecommerce:fba`:
- `scripts/verify/verify-product-lifecycle-events.mjs` (`verify:product:lifecycle-events`, тесты — `test:verify:product:lifecycle-events`, 7/7) — фиксирует контракт `product.unpublished`/`product.archived` (вариант, `event_type`, `schema_version`, `affects_index`, валидация, реестр схем, канонический сэмпл), эмиссию из `unpublish_product`/`archive_product`/`update_product`, порт записи и декоратор, обязательный `idempotencyKey` в GraphQL и REST, все потребительские маршруты (search, index refresh, publication) и обязательную инструкцию регенерации digest-артефакта;
- `scripts/verify/verify-product-relations-bundles-idempotency.mjs` (`verify:product:relations-bundles-idempotency`, тесты — `test:verify:product:relations-bundles-idempotency`) — фиксирует receipt-протокол обоих владельцев (общий ledger `rustok_outbox::idempotency`, отсутствие параллельной таблицы, `admit`/`complete`/`fail`, общие транзакционные core-функции), обязательные `idempotencyKey` в трёх мутациях, bounded-словарь ошибок, использование ключа в native-админке и документ границы записи.

**Известное ограничение волны 5 (обязательный шаг перед merge).** Добавление двух событий меняет wire-контракт, поэтому закоммиченный `crates/libs/rustok-events/contracts/event-contract-digests.json` устарел: четыре digest-значения (registry/root_event/root_envelope/contract_payload/contract_envelope) должны быть перегенерированы на этой самой ветке командой `cargo run --locked -p rustok-events --example event_contract_digests -- --write` и закоммичены в этом же PR; тест `published_event_contract_matches_committed_release_artifact` до этого шага падает намеренно (это и есть admission-сигнал wire-контрактного изменения).

### Волна 6 — P2: `PROD-CONC-001` — business revision, predecessor-CAS и явные `Keep / Set / Clear`-патчи

| ID | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| `PROD-CONC-001` | Устранено | (1) У агрегата появилась business revision: миграция `m20261007_000035_add_product_revision.rs` добавляет `revision INTEGER NOT NULL DEFAULT 1` и PostgreSQL-инвариант `chk_products_revision_positive CHECK (revision > 0)`; `revision` читается в сущности продукта и во всех ответах владельца. (2) Новый модуль владельческого контроля `services/catalog/concurrency.rs`: зарезервированный класс отказа (`REVISION_CONFLICT_PREFIX`, `revision_conflict`, `revision_conflict_of`), `next_revision` (отказ при исчерпании `i32`), `ensure_expected_revision` (сверка предшественника после блокирующего чтения строки) и `validate_patch_text_length` (границы 100/255/255/64 для `Patch::Set`); 4 юнит-теста. (3) `update_product` переведён на predecessor-CAS: блокирующее чтение (`find_product_for_update_in_tx` → `lock_exclusive`) → `ensure_expected_revision(input.expected_revision)` → безусловный `revision = Set(next_revision(...))` в той же транзакции; публикация/снятие/архивация и смена статуса бампают ревизию, `Patch::Clear` снимает типизированную привязку доставки вместе с legacy-тенью в метаданных. (4) `UpdateProductInput` получил `expected_revision: Option<i32>` и пять полей-идентичностей через `Patch<T>` (`seller_id`, `vendor`, `product_type`, `shipping_profile_slug`, `primary_category_id`) с `changes_document()`/`missing_expected_revision()`; создание пишет `revision: Set(1)`. (5) Граница записи: `ProductCatalogCommandPort` отказывает внешней документной записи без ревизии (`product.revision_required`) и транслирует зарезервированный класс конфликта в `product.revision_conflict` (`retryable=false`). (6) Публичные поверхности: GraphQL `PRODUCT_REVISION_CONFLICT` / `PRODUCT_REVISION_REQUIRED` с действиями для оператора (`retryable`, `correlation_id`), REST `409 commerce_admin_product_revision_conflict` / `422 commerce_admin_product_revision_required`; `i32`-тип ревизии проведён сквозь порт, GraphQL-схему и REST-схему. (7) Известная дыра закрыта: хост-совместимый писатель `apps/server` (Flex attached values) читает строку продукта под `lock_exclusive` и бампает ревизию в той же транзакции. (8) Обе админки ведут ревизию документа: Leptos-админка отправляет `revision: draft.revision` при сохранении документа (смена статуса — lifecycle-only, без ревизии), Next-админка (FFA) читает `revision` в детальном запросе, отправляет его в мутации, возвращает `{ id, revision }` и принимает новую ревизию после сохранения/смены статуса. (9) Пропатчены фикстуры, где сущность/сервис читает `revision` (`rustok-ai`, `rustok-product-bundles`, `rustok-pricing`, apps/server-тесты) | `crates/modules/rustok-product/src/migrations/m20261007_000035_add_product_revision.rs`, `.../src/entities/product.rs`, `.../src/services/catalog/concurrency.rs`, `.../src/services/catalog/{commands,helpers,projection}.rs`, `.../src/dto/product.rs`, `.../src/catalog_command_port.rs`, `.../src/public_error.rs`, `.../src/ports/diagnostics.rs`, `crates/modules/rustok-commerce/src/graphql/{types.rs,mutations/catalog.rs}`, `.../src/controllers/products.rs`, `.../src/controllers/admin/products.rs`, `crates/modules/rustok-commerce/tests/catalog_service_test.rs`, `crates/modules/rustok-product/admin/src/{model.rs,core.rs,transport/graphql_adapter.rs,transport/product_lifecycle_graphql.rs,ui/leptos.rs,ui/product_editor.rs}`, `apps/next-admin/packages/rustok-product/src/{api/types.ts,api/products.ts,pages/product-editor-page.tsx}`, `apps/next-admin/src/app/dashboard/product/actions.ts`, `apps/server/src/services/flex_attached_values.rs` |

**Тесты волны 6.** `catalog_service_test.rs`: `update_product_refuses_a_stale_predecessor_revision` (устаревший предшественник отвергнут, повторный запуск с перечитанной ревизией принят), `lifecycle_transitions_bump_the_revision_and_clear_patches_drop_shadows` (публикация/снятие бампают ревизию; `Patch::Clear` снимает и типизированную привязку, и её метаданные-тень). Postgres-доказательство: три `apps/server/tests/product_*_translation_target_postgres.rs` читают ревизию перед каждой записью документа и проверяют инкремент; `product_product_translation_target_postgres.rs` дополнительно требует отказа по устаревшему предшественнику и неизменности строки после отказа (без отката эта проверка невозможна, поэтому она идёт через реальный Postgres). `port_conformance.rs` фиксирует отсутствие ревизии у unconditional-записей владельца.

**Осознанные решения.** (а) Конфликт предшественника едет зарезервированным классом сообщения `CommerceError::Validation`, а не новым вариантом ошибки: закрытый публичный словарь владельца запинен `verify-product-public-error-diagnostic-safety.mjs` (8 вариантов), а все внешние границы транслируют класс в собственный публичный код (`product.revision_conflict` → `PRODUCT_REVISION_CONFLICT` → `409`). (б) `expected_revision: None` остаётся явной безусловной записью и разрешён только owner-internal вызовам; внешние транспорты (порт, GraphQL, REST) требуют ревизию для документных изменений, а lifecycle-only смена статуса остаётся повторяемым переходом без ревизии — это зафиксировано в документации поля в `dto/product.rs`. (в) Ослабление проверки: `graphql_error_safety.rs` (граница Leptos-админки) намеренно оставляет статичный публичный конверт и не пробрасывает серверные сообщения наружу; Next-админка, наоборот, не фильтрует `extensions.code` и доносит до оператора действие «перечитайте товар и повторите правку». (г) Верификатор `verify-commerce-product-graphql-lifecycle-command-cutover.mjs` перепинен на новую проводку мутации (`expected_revision: input.revision`); падение на pristine-дереве было устаревшим маркером (в базовой ревизии мутация уже передавала `shipping_profile_slug`/`metadata`), а не регрессией.

**Формат-гейт (новое в этой волне).** В песочнице нет `cargo`/`rustup`, но официальные компоненты Rust доступны через npm-зеркало `@rustbin`, поэтому гейт `rustfmt` теперь настоящий, а не эвристический: `npm i @rustbin/rustfmt-1.88.0-x86_64-unknown-linux-gnu @rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu` + `LD_LIBRARY_PATH` на `librustc_driver` (обёртка — `rustfmt_check.sh` в корне рабочего дерева). Проверено: из 24 изменённых/новых `.rs` файлов **ни одна** добавленная строка не переформатируется `rustfmt --edition 2024` (при 278 строках унаследованного дрейфа в тех же файлах, которые сознательно не трогались), и **ноль** ошибок разбора настоящим парсером `rustc`. Для TypeScript-части: `prettier@3.9.9` с настройками `apps/next-admin/.prettierrc` (без плагина tailwind-сортировки) не меняет ни одной добавленной строки; оставшиеся расхождения в этих файлах (`ProductRelationType`-union, список параметров `onUpdateBundle`, хвостовые пустые строки) унаследованы и присутствуют в базовой ревизии.

**Верификаторы волны 6.** Полный прогон `scripts/verify/verify-*product*.mjs` по рабочему дереву — **73 PASS / 46 FAIL**, побайтово тот же список, что и до правок этой волны (все 46 падений унаследованы и падают так же на pristine-дереве `HEAD`); подмножество `commerce|migration|schema` — **69 PASS / 115 FAIL**, тоже без изменений; девять верификаторов, читающих `packages/rustok-product/src`, не затронуты. Проверка allowlist публичных кодов ошибок: `graphql_error_safety.rs` ведёт только приватную диагностику (длины/присутствие), а `apps/next-admin/src/shared/api/graphql.ts` пробрасывает `extensions.code` без фильтрации — новых кодов-исключений не требуется.

**Ограничение проверки.** Rust-код по-прежнему не компилировался и не запускался (`cargo` в песочнице отсутствует): корректность подтверждена разбором настоящим парсером `rustc`, `rustfmt`-гейтом, верификаторами и сверкой с семантикой `sea-orm 2.0.3` (`lock_exclusive` — `QuerySelect`, `ignore` не требуется). Требуется владельцу перед merge: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test -p rustok-commerce --test catalog_service_test` и Postgres-прогоны `product_{product,image,variant}_translation_target_postgres` (они выполняются только с переменной `RUSTOK_MIGRATION_SMOKE_ADMIN_URL`). Следующая волна по плану Части IV — `PROD-STORE-001` и `PROD-ATTR-STORE-001` (фасеты витрины и значения атрибутов в детальной карточке).

### Волна 7 (в работе) — `PROD-STORE-001` и `PROD-ATTR-STORE-001`: фасеты и значения атрибутов на витрине

**Архитектурное решение по переиспользованию (ответ на вопрос «не должна ли общая часть уехать в библиотеки таблиц»).** Разделено по слоям:

- **Контракт и семантика фасетов → библиотека таблиц `crates/ui/rustok-grid`** (её уже используют админские крейты модулей: auth, blog, brand, product). Новый модуль `facet`: `GridFacet`, `FacetValue`, `FacetDomain { Dictionary { multi } | Boolean | Open }`, лимиты `MAX_GRID_FACETS` / `MAX_GRID_FACET_VALUES`, правило drill-down (`selection_except` / `GridFacet::other_selection` — фасет считается по всем *остальным* активным фильтрам) и перевод бакетов в собственный словарь `FilterOption`. Теперь **любая** таблица платформы может показать счётчики фасетов, как только её владелец начнёт возвращать counts, и это чистая, покрытая тестами логика без DOM/HTTP/БД. `FilterOption` намеренно не менялся: добавление поля `count` сломало бы 69 структурных литералов по репо, а счётчик принадлежит фасету, не списку выбранных опций.
- **Подсчёт (SQL) остаётся у владельца данных** — `crates/modules/rustok-product/src/services/catalog/facets.rs`. Доменные крейты (`rustok-product`, витрина) на UI-библиотеку не зависят, и это сохраняется: генерализовать счёт можно только через абстракцию над произвольными таблицами/EAV-моделью, что для одной реализации было бы спекулятивным усложнением. Вместо этого переиспользуется то, что действительно общее: позиционные плейсхолдеры (`catalog::helpers::sql_placeholder`, три дубля сведены в один; копии в неизменяемых миграциях не трогались) и сам словарь `rustok-grid`.
- **Граница между ними** — `ProductCatalogReadPort::load_storefront_catalog_facets` (владелец) → `storefrontProductCatalogFacets` (Commerce GraphQL) → адаптеры витрин; админский маппинг владельческого ответа в `GridFacet` делается одной функцией на стороне админки, когда появится её UI-часть.

**Сделано (три коммита).**
1. `fix(product): close the attribute-filter contract for typed EAV filters` — единый предикат `AttributeValueType::is_attribute_filterable()` (json нельзя за `code=value`), явный отказ в исполнителе до построения SQL, GraphQL-входы `attribute_filters` как `#[graphql(default)] Vec<String>` (совместимость подтверждена по всем потребителям: Rust-адаптеры шлют массив, Next-витрина опускает поле). Верификатор `verify-product-catalog-attribute-filters.mjs`, падавший с базовой ревизии, **стал зелёным** (74 PASS / 45 FAIL, дельта ровно в один верификатор).
2. `feat(product): compute storefront catalog facets in the owner` — `CatalogService::storefront_catalog_facets`: на каждый запрошенный код атрибута счётчики бакетов при полном наборе фильтров витрины (tenant + active + published + видимость канала + категория + поиск + все *остальные* атрибутные фильтры), бакеты — id опций словаря и `true`/`false`, лимиты и точный `is_truncated`, локализованные метки (`facet_label` → `label` → код; локаль запроса → fallback), `is_enumerable=false` для неограниченных доменов. Сборка на sea_query с bound-параметрами; статус продукта сравнивается инлайн-литералом, потому что в Postgres это `product_status_enum` (не резолвится против текстового параметра).
3. `feat(product,grid): share the facet contract with every grid, expose facets over GraphQL` — модуль `facet` в `rustok-grid` (+ README-раздел «Facets» и строка инвариантов), порт-метод с fail-closed дефолтом, `StorefrontCatalogFacetsRequest`, GraphQL-типы `GqlStorefrontCatalogFacet{Value}` и корень `storefrontProductCatalogFacets(locale, filter, facetCodes)`.

**Осталось по волне.** Витрина Leptos: модель фасетов, native/GraphQL-адаптеры, рендер чекбоксов со счётчиками при сохранении совместимости параметра `attribute_filters=code=value`; витрина Next: фасетная панель в `product-filters.tsx`; админская таблица: маппинг владельческого ответа в `GridFacet`; `PROD-ATTR-STORE-001`: storefront-проекция значений атрибутов (`show_on_storefront` + локализованные метки/опции) и таблица характеристик в детальной карточке обеих витрин; верификатор на цепочку «владелец → порт → GraphQL/native → витрина».

**Ограничение проверки.** Rust-код снова не компилировался (`cargo` в песочнице отсутствует): подтверждено настоящим парсером `rustc` и `rustfmt --edition 2024` (ноль переформатированных строк этого изменения, ноль ошибок разбора), верификаторами (74 PASS / 45 FAIL без регрессий) и сверкой API по исходникам `sea-query 1.0.2` / `sea-orm 2.0.3` (механизм перенумерации плейсхолдеров в `cust_with_values`, `join_as`/`from`/`group_by_col`/`expr_as`, `Values: IntoIterator`). Фасетный SQL требует прогона владельцем: `cargo test -p rustok-product` (юнит-тесты фасетов) плюс новый Postgres-тест «счётчики фасетов под активными фильтрами», который появится вместе с витринным рендером.


**Волна 7, продолжение: сплошная перепроверка ветки и витринная панель фасетов.**

4. `fix(product,admin): repair two compile blockers found by the branch-wide audit` — сплошная перепроверка всего, что сделано в этой сессии, настоящим парсером `rustc` и структурным сканером объявлений нашла **два реальных блокера компиляции**, оба исправлены: (а) `services/catalog_schema_service/values.rs` — `struct ProductAttributeValueLocaleRow` и свободная `async fn load_missing_required_locales` оказались **внутри** `impl ProductCatalogSchemaService` (rustc: «struct is not supported in `trait`s or `impl`s»); перенесены в область модуля после `impl`, рядом с `canonical_value_locale`; (б) `admin/src/transport/graphql_error_safety.rs` — `const PRODUCT_ADMIN_MUTATION_GRAPHQL_BOUNDARY` объявлялся дважды (E0428); оставлена копия внутри блока контекста мутаций, потому что именно её пинит `verify-product-admin-primary-mutation-error-safety.mjs`. Метод проверки зафиксирован: по всему диапазону ветки — разбор `rustc` + отчёт о переформатируемых добавленных строках и сканер дублей объявлений уровня модуля (два дубля, оба устранены, остаток — ноль).
5. `feat(product): render facet buckets from typed columns, default the facet-codes argument` — бакеты читаются нативными колонками (`option_id`, `value_boolean`) и рендерятся в Rust вместо `CAST(... AS TEXT)`: приведение зависело бы от способа хранения в конкретной СУБД, а один и тот же SQL исполняется и на PostgreSQL, и на портируемой SQLite-схеме тестов; булев фильтр теперь тоже едет bound-параметром-`bool`. Вызовы билдера сверены с исходниками sea-query 1.0.2 (`order_by_col` в 1.0.2 не существует — исправлено на `order_by`).
6. **Витрина Leptos (этот коммит).** Панель фасетов собрана по слоям, каждый — в своём доме:
   - **Общий помощник → UI-библиотека** `crates/ui/rustok-ui-core/src/ui.rs`: `apply_ui_query_pairs(base, &[(key, Option<String>)])` — накладывает пары на путь с query-строкой, заменяя/удаляя ключи в их исходной позиции (стабильные ссылки при повторных переключениях) и кодируя значения по правилам браузерной формы. Дублировать сборку URL в каждой витрине больше не нужно; 4 юнит-теста покрывают замену, удаление, сохранение чужих ключей и пустой результат.
   - **Модель** `storefront/src/model.rs`: `ProductCatalogFacet { code, label, valueType, isLocalized, isEnumerable, isTruncated, totalProducts, values }` и `ProductCatalogFacetValue { value, label, count }` — serde-имена совпадают и с GraphQL-ответом (`storefrontProductCatalogFacets`), и с native-ответом серверной функции.
   - **Контролы** `catalog_controls.rs`: `parse_attribute_filter`, `is_attribute_filter_selected`, `toggle_attribute_filter` (переключение одного `code=value` с сохранением порядка), `has_attribute_filter_for_code`, `clear_attribute_filter_code` и копия панели `CatalogFacetLabels` (заголовок, подсказка неограниченного домена, подсказка усечения, «очистить», маркеры выбора, шаблон счётчика).
   - **Политика отображения** `core.rs`: `build_catalog_facet_codes` (коды берутся из тех же catalog search options, что предлагаются как фильтры; пустые и дубли отбрасываются — владелец считает только запрошенные фасеты), `build_catalog_facet_toggle_query` / `build_catalog_facet_clear_code_query` / `build_catalog_facet_clear_query` (полный URL текущей страницы: поиск, категория, сортировки, валюта) и `build_catalog_facet_filters_view_model` (счётчики, признак выбора, ссылки, `clear_href` на фасет и на всю панель, `show_empty_state`). Здесь же 4 юнит-теста, включая проверку точной закодированной ссылки.
   - **Транспорт**: новый native-путь `transport/catalog_facets_native.rs` (endpoint `product/storefront/catalog-facets`, владельческий `CatalogService::storefront_catalog_facets`, приватные статические конверты ошибок и bounded-диагностика как у листинга), GraphQL-адаптер (`STOREFRONT_CATALOG_FACETS_QUERY`, `facetCodes`, фильтр без пагинации — счётчики не страничные) и общая `transport::fetch_catalog_facets` с третьим маппингом ошибок через `GraphqlCallContext::fetch_catalog_facets` (в контекст добавлен bounded-факт `facet_code_count`, в лог — только количество кодов, никогда сами коды).
   - **UI** `ui/leptos.rs`: компонент `CatalogFacetFilters` (реэкспортирован из корня крейта) — бакеты как ссылки-переключатели с `aria-pressed`, счётчик, подсказка неограниченного домена, подсказка усечения, «очистить» на фасет и на всю панель; состояние выбора живёт в URL, поэтому панель не держит клиентского состояния, а ссылки на товары остаются разделяемыми. Опции и фасеты грузятся одним ресурсом, поэтому коды для счётчиков берутся из уже загруженных опций без лишнего запроса.
7. **Верификаторы и доказательства.** `verify-product-storefront-graphql-error-safety.mjs` переведён на три контекстных маппинга и получил маркеры фасетной операции (`GraphqlCallContext::fetch_catalog_facets(`, `catalog_facets_native::fetch_catalog_facets(`, `STOREFRONT_CATALOG_FACETS_QUERY`, `pub(super) fn fetch_catalog_facets`, `facet_code_count`); evidence/review JSON расширены честно (`covered_operations` + `facet_operation_added`, `facet_codes_defaulted`, `facet_code_count_logged`), документация границы дополнена абзацем о фасетном чтении. `verify-product-storefront-boundary.mjs` получил маркеры фасетной поверхности (`build_catalog_facet_codes`, `build_catalog_facet_filters_view_model`, `mod catalog_facets_native;`, `fetch_catalog_facets`, `<CatalogFacetFilters`), а его фикстурный тест — соответствующие строки в синтетических источниках и подмену **обоих** мест маппинга поиска (листинг и фасеты), `12/12`. `verify-product-catalog-attribute-filters.mjs` (гейт волны) дополнен маркерами фасетов в контролах, core, UI, native, GraphQL и в общем помощнике `rustok-ui-core`. Обновлены `storefront/README.md`, `docs/storefront-graphql-error-safety.md` и `implementation-plan.md` (отметка о фасетной поверхности + абзац контракта, `Recheck on 2026-07-29` сохранён одной строкой, потому что его пинит `verify-product-catalog-controls-plan-sync.mjs`).
8. **Проверки.** Полный прогон верификаторов — **74 PASS / 45 FAIL**, побайтово тот же список, что и на вершине ветки до этих правок (новых падений нет, фасетный гейт зелёный); `verify-product-storefront-boundary.test.mjs` — 12/12; разбор всех изменённых `.rs` настоящим парсером `rustc` — ноль ошибок; формат-гейт — только добавленные мной строки, унаследованный дрейф не тронут; URL-семантика (`apply_ui_query_pairs` + переключение выбора) продублирована JS-эмуляцией и совпала с ожиданиями Rust-тестов 4/4. Компиляции по-прежнему нет (в песочнице нет `cargo`) — требуются `cargo check -p rustok-product-storefront --features ssr|hydrate`, `cargo check -p rustok-ui-core`, `cargo test -p rustok-product-storefront` и `cargo fmt --all -- --check` на стороне владельца.

**Осталось по волне.** Витрина Next (`product-filters.tsx`: фасетная панель); админская таблица: маппинг владельческого ответа в `GridFacet`; `PROD-ATTR-STORE-001` (проекция значений атрибутов в детальной карточке обеих витрин); кросс-слойный верификатор «владелец → порт → GraphQL/native → витрина»; Postgres-тест счётчиков фасетов.

### Волна 8 — фасетная панель витрины Next (пересборка утерянного коммита)

**Контекст.** Артефакты прошлой сессии (`/home/user/rustok-waves-8-9.bundle`, каталог `patches/`, `run_verifiers.sh`) в песочнице отсутствуют, а коммиты `512b6ba` (Next facet panel) и `a8c1b19` (admin facets + `rustok-grid/facet_panel`) недостижимы ни локально (`git cat-file -t`), ни на origin (`gh api search/commits` находит только `7895d7f`). Ветка-предшественник `arena/00d5b72d-rustok` (`1abd6492c`) содержит волну 7, но не волны 8–9. Волны 8–9 пересобраны заново поверх `7895d7f` строго по спецификации «Осталось по волне»; SHA и объём изменений отличаются от утерянных коммитов, содержание — нет.

| Пункт | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| Витрина Next: фасетная панель | Устранено | Панель собрана как чистая функция состояния маршрута: новый framework-free модуль `catalog/facets.ts` (`parseAttributeFilter(s)`, `serializeAttributeFilters`, `is/isAttributeFilterSelected`, `hasAttributeFilterForCode`, `toggleAttributeFilter` с сохранением порядка, `clearAttributeFilterCode`, `applyQueryPairs` — побайтовый аналог `rustok_ui_core::apply_ui_query_pairs`, `buildCatalogFacetCodes`, `buildCatalogFacet{Clear,ClearCode,Toggle}Query`, `buildCatalogFacetFiltersView`, `buildCatalogFacetLabels`). GraphQL-клиент получил `STOREFRONT_CATALOG_FACETS_QUERY` + `fetchStorefrontCatalogFacets` и общий `toCatalogFilterVariables(filter, paginate)`, где фасетный запрос **не пагинируется** (владелец считает весь отфильтрованный каталог); типы `ProductCatalogFacet{,Value}` — с serde-именами GraphQL. `product-filters.tsx` рендерит панель ссылками (`aria-pressed`, маркер, счётчик, подсказки неограниченного домена и усечения, «очистить» на фасет и на всю панель), `product-grid.tsx` протаскивает `attribute_filters`/валюту в ссылки пагинации, `products/page.tsx` парсит `attribute_filters`, грузит фасеты (fail-soft: недоступный счётчик не роняет каталог) и передаёт их в грид. Локализация: ключи `product-list-facets*` добавлены в `storefront/locales/{en,ru}.ftl`, копия Next сверяется с ними тестом | `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts`, `.../src/api/{products,types}.ts`, `.../src/components/{product-filters,product-grid}.tsx`, `.../src/index.tsx`, `apps/next-frontend/src/app/[locale]/products/page.tsx`, `crates/modules/rustok-product/storefront/locales/{en,ru}.ftl` |
| Фикстура гейта `verify-product-catalog-attribute-filters.mjs` | Устранено | Companion-тест падал на `main` (1/4): его синтетическая фикстура не создавала `storefront/src/core.rs`, `storefront/src/transport/catalog_facets_native.rs` и `crates/ui/rustok-ui-core/src/ui.rs` и не содержала фасетных маркеров, которые верификатор требует с волны 7. Фикстура достроена, добавлены 4 негативных кейса (view model, панель, native-путь, GraphQL-путь), объединены в 7 тестов | `scripts/verify/verify-product-catalog-attribute-filters.test.mjs` (`# tests 7 / # pass 7 / # fail 0`) |
| Верификатор волны | Добавлено | `verify-product-storefront-next-facets.mjs` — source-lock всей цепочки (модуль → GraphQL-клиент → типы → панель → грид → страница → экспорты пакета) плюс сверка словаря `attribute_filters` с Rust-витриной и владельческим GraphQL-рутом; запрет `react`/`next`/`useState` в framework-free модуле. `verify-product-storefront-next-facets.test.mjs` исполняет сам TS-модуль через `node --experimental-strip-types` (7/7): словарь и round-trip фильтров, порядок тоггла, точные закодированные ссылки, модель панели (выбор, счётчики, подсказки, clear), синхронность копии с `.ftl` | `scripts/verify/verify-product-storefront-next-facets.mjs`, `scripts/verify/verify-product-storefront-next-facets.test.mjs` |

**Проверки.** Свип собран заново (`run_verifiers.sh`: все non-test `scripts/verify/*product*.mjs`, 119 файлов) и на `7895d7f` воспроизводит прежние **74 PASS / 45 FAIL**; после волны — **75 PASS / 45 FAIL** при побайтово том же списке падений (дельта ровно в новый верификатор). Точечно: гейт атрибутов — passed и 7/7, `verify-product-storefront-boundary.test.mjs` — 12/12, `verify-product-admin-boundary.test.mjs` — 14/14. Изменённые TS/TSX впервые проверены настоящим компилятором: `tsc 5.6.3 --noEmit` по модулю, клиенту, панели, гриду, индексу пакета и странице каталога (внешние пакеты — заглушками) — **ноль ошибок**; `cargo` в песочнице по-прежнему нет.

### Волна 9 — счётчики фасетов в админской таблице (пересборка утерянного коммита)

**Контекст.** Та же пересборка, что и в волне 8: коммит `a8c1b19` (admin facets + `rustok-grid/facet_panel`) недостижим, поэтому волна собрана заново поверх `7895d7f` по спецификации «Осталось по волне». Содержание — как в утерянном коммите; SHA и объём изменений отличаются.

| Пункт | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| Общая панель фасетов в `rustok-grid` | Устранено | Логика панели вынесена из витрин в общий крейт: `facet_panel.rs` с `FacetPanelLabels` (+ `english()`), `FacetPanel::build(&[GridFacet], &[String], &FacetPanelLabels)` (не более `MAX_GRID_FACETS`, порядок владельца), моделями `FacetPanelFacet`/`FacetPanelValue` (счётчик, `count_label`, маркер выбора, `selection_entry`) и словарной функцией выбора: `selection_entry`/`split_selection` (ключи без учёта регистра), `is_selection_selected`, `has_selection_for_key`, `selection_for_key`, `selection_after_toggle` (первое точное совпадение удаляется, иначе добавление в конец), `selection_after_clear_key`, `selection_after_clear`, `count_label` (текстовая подстановка `{count}`). Панель не знает ни Leptos, ни i18n: строки приходят снаружи. Экспорт из `lib.rs`/prelude и раздел README «Facet panel» | `crates/ui/rustok-grid/src/facet_panel.rs`, `crates/ui/rustok-grid/src/{lib.rs,README.md}` |
| Владелец: счётчики под админский фильтр | Устранено | `CatalogService::admin_catalog_facets(tenant_id, locale, fallback_locale, &AdminProductListQuery, facet_codes)`: те же бакеты, что у витрины, но выборка описывается `CatalogFacetScope::Admin { status }` — админка считает и черновики, и архив (то, что показывает список), при заданном `status` — ровно выбранный жизненный статус. `CatalogFacetScope::{Storefront,Admin}` и `CatalogFacetFilters` объединили общий загрузчик `load_catalog_facets`; сравнение статуса — инлайн-литерал (`facet_status_literal`), потому что в Postgres это enum `product_status_enum` и текстовый параметр против него не резолвится. Пагинация списка на счётчики не влияет | `crates/modules/rustok-product/src/services/catalog/facets.rs` |
| Порт | Устранено | `AdminCatalogFacetsRequest { locale, fallback_locale, query, facet_codes }`, операция `LOAD_ADMIN_CATALOG_FACETS_OPERATION` и метод `load_admin_catalog_facets`; дефолт fail-closed (`PortError::unavailable("product.admin_catalog_facets_unavailable", …)`), поэтому отсутствие возможности у владельца не ломает список | `crates/modules/rustok-product/src/ports/{types.rs,catalog_read.rs}` |
| Commerce GraphQL | Устранено | `GqlAdminCatalogFacet{,Value}` и корень `adminProductCatalogFacets(tenantId, locale, filter, facetCodes)` — те же поля, что у витринного корня; требует `PRODUCTS_LIST|PRODUCTS_READ`, bounded-контекст 2 c, фасетный фильтр без пагинации (счётчики не страничные) | `crates/modules/rustok-commerce/src/graphql/product_catalog.rs` |
| Админ-поверхность (Leptos) | Устранено | `admin/src/facets.rs` — маппинг владельческого ответа в `GridFacet` (`is_enumerable` → `FacetDomain::Open`, `boolean` → `Boolean`, `multiselect` → `Dictionary { multi }`), копия панели из FTL (`product.list.facets*`, ключи добавлены в оба админских каталога), коды фасетов — из тех же catalog search options, что предлагаются оператору, и сборка view-модели: ссылки-тоглы и «очистить» строятся через `apply_ui_query_pairs` + `serialize_attribute_filters`, то есть панель говорит на том же `attribute_filters=code=value`, что и URL-контролы. `model.rs` получил общие для обоих транспортов camelCase-типы (`totalProducts`/`isEnumerable`/`isTruncated`/`isLocalized`). Транспорт: native server-fn `product/admin/catalog-facets` (permissions, tenant-check) + GraphQL-фолбэк, фасад `catalog_transport::fetch_admin_catalog_facets` деградирует в `None` вместо ошибки. `ui/catalog_facets.rs` — компонент `AdminCatalogFacetPanel`, `ui/product_grid.rs` грузит фасеты `LocalResource` поверх контролов и `refresh_nonce` и не рендерит панель, когда считать нечего | `crates/modules/rustok-product/admin/src/{facets.rs,model.rs,catalog_transport.rs}`, `.../src/transport/admin_catalog_{native,graphql}.rs`, `.../src/ui/{mod.rs,product_grid.rs,catalog_facets.rs}`, `.../admin/locales/{en,ru}.ftl` |
| Шаблон счётчика — код, а не Fluent | Устранено | `product-list-facetsCount` убран из всех четырёх каталогов (обе витрины): `{count}` — не переменная Fluent, а подстановка панели, поэтому запись была невалидным сообщением (ссылка на несуществующее сообщение `count`). Числовой аффикс локален и оставлен кодовым дефолтом `({count})`; i18n-гейт `verify-ui-i18n-parity.mjs` по продуктовым каталогам стал чистым | `crates/modules/rustok-product/{storefront,admin}/locales/{en,ru}.ftl` |
| Верификаторы волны | Добавлено | `verify-product-admin-facets.mjs` — source-lock всей цепочки (общая панель → владелец → порт → GraphQL → модель/транспорты/панель/грид админки) с запретом считать продукты в админских адаптерах и сравнивать статус текстовым параметром; `verify-product-admin-facets.test.mjs` (6/6) пинет кросс-файловые инварианты: копия есть в обоих каталогах и совпадает с Rust-дефолтами, переводимые строки переведены, а маркеры и шаблон счётчика — нет, копия лейблов панели совпадает с общей поле-в-поле, GraphQL-селекция и serde-имена модели не разъезжаются, панель не держит состояния | `scripts/verify/verify-product-admin-facets.mjs`, `scripts/verify/verify-product-admin-facets.test.mjs` |

**Осознанное сужение волны.** Таблица товаров Next-админки (FFA, `@rustok/product-admin`) панель не получила: её список ходит в корень `products(ProductsFilter)`, у которого нет ни `attribute_filters`, ни фасетов, — счётчики там были бы декорацией. Панель появится вместе с переводом списка на `adminProductCatalog`; до этого фасетная поверхность у админки одна — смонтированный Leptos-грид.

**Проверки.** Свип — **76 PASS / 45 FAIL**: список падений побайтово тот же, что на `7895d7f` и после волны 8, дельта — ровно новый гейт. Точечно: `verify-product-admin-facets.mjs` — passed и 6/6; `verify-product-catalog-attribute-filters.mjs` — passed; `verify-product-storefront-boundary.test.mjs` — 12/12; `verify-product-admin-boundary.test.mjs` — 14/14; `verify-product-storefront-next-facets.test.mjs` — 7/7 (обновлён под кодовый шаблон счётчика); `verify-ui-i18n-parity.mjs` — по продуктовым каталогам чисто (остаются унаследованные падения `apps/next-admin/messages` по plural-правилу, вне продукта). Все 12 изменённых `.rs` разобраны настоящей грамматикой Rust (`tree-sitter-rust`); структурный сканер не нашёл ни разбалансированных скобок, ни дублей объявлений, ни новых неиспользуемых импортов. Компиляции по-прежнему нет: владельцу нужны `cargo check -p rustok-product-admin`, `cargo check -p rustok-product --features ssr`, `cargo check -p rustok-commerce` и `cargo test -p rustok-product`.

**Осталось по волне.** Очередь Части IV в прежнем порядке: `PROD-ATTR-STORE-001` (проекция значений атрибутов и таблица характеристик в обеих витринах), кросс-слойный верификатор «владелец → порт → GraphQL/native → витрина» (фасеты и атрибуты), Postgres-тест счётчиков фасетов, три незаленденных контракта (`lifecycle-retry-consumer`, `fallback-mutation-error-safety`, `commerce-product-schema-write-consumer-cutover`), далее P2-находки (`PROD-SHIP-001`, `PROD-DIGITAL-001`, `PROD-TESTS-001`).

### Волна 10 — фасетная логика уезжает в общие библиотеки (Rust-таблица + Next-хост)

**Задача.** Проверить, можно ли переиспользовать фасетный код нашей библиотекой таблиц, и отделить общую часть в хост, где живут остальные UI-компоненты. Проверка дала три вывода: (1) на Rust-стороне панель уже жила в `rustok-grid`, но витрина Leptos с волны 7 держала **свою копию** словаря выбора и сборки view-модели; (2) на Next-стороне framework-free фасетный модуль лежал **внутри пакета продукта**, поэтому готовый хост-табличный виджет (`apps/next-admin/src/widgets/data-table`) физически не мог его переиспользовать; (3) место для общего кода — хост-уровень `packages/` (там уже `@rustok/richtext`, `@rustok/ui-auth`, оба Next-приложения ставят их как `file:`-зависимости), а не `UI/next/components` (это React-only поверхность, подключённая алиасом `@iu/*` только в next-admin и пока не используемая).

| Пункт | Статус | Что сделано | Доказательство в дереве |
|---|---|---|---|
| Host-пакет `@rustok/ui-grid` (TS-двойник `crates/ui/rustok-grid`) | Добавлено | Новый framework-free пакет: `facet.ts` (контракт `FacetDomain`/`facetFromBuckets`/`facetFilterOptions`, лимиты `MAX_GRID_FACETS`/`MAX_GRID_FACET_VALUES`, словарь `selectionEntry`/`splitSelection`/`parseSelection`/`serializeSelection`/`isSelectionSelected`/`hasSelectionForKey`/`selectionForKey`/`selectionExcept`/`selectionAfter{Toggle,ClearKey,Clear}`), `panel.ts` (`FacetPanelLabels` + `ENGLISH_FACET_PANEL_LABELS`, `countLabel`, `buildFacetPanel`, переходы панели), `url.ts` (`applyQueryPairs`/`queryParam` — тот же контракт, что `rustok_ui_core::apply_ui_query_pairs`). Панель не строит ссылок и не знает локали: адаптер отдаёт копию и превращает `selectionEntry` в href. | `packages/rustok-ui-grid/{package.json,README.md,src/{index,facet,panel,url}.ts}` |
| Продуктовая склейка | Устранено | `catalog/facets.ts` больше не держит реализаций: словарь реэкспортируется/делегируется в общий пакет, на месте остаётся только продуктовое — имя параметра маршрута (`CATALOG_ATTRIBUTE_FILTERS_PARAM`), пары URL каталога, FTL-копия, `buildCatalogFacetCodes` и маппинг ответа владельца (`catalogFacetToSource`). Публичный API и поведение сохранены. | `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts` |
| Готовый Next data-table в хосте | Устранено | `data-table-faceted-filter.tsx` переведён на общие переходы (`selectionAfterToggle`, `selectionAfterClear`, `hasSelectionForKey`): у этого виджета тот же контракт выбора, что у счётных панелей, и теперь одна реализация на оба. Импорт `@rustok/ui-grid` (алиас + `file:`-зависимость + `transpilePackages`). | `apps/next-admin/src/widgets/data-table/data-table-faceted-filter.tsx`, `apps/next-admin/{package.json,tsconfig.json,next.config.mjs}` |
| Обвязка хостов | Устранено | Оба Next-приложения получают пакет ровно так, как остальные workspace-пакеты: `file:../../packages/rustok-ui-grid` в зависимостях, `paths` в `tsconfig.json`, `transpilePackages` в `next.config.mjs`. | `apps/next-frontend/{package.json,tsconfig.json,next.config.mjs}`, `apps/next-admin/{package.json,tsconfig.json,next.config.mjs}` |
| Rust-витрина переиспользует библиотеку таблиц | Устранено | Витрина Leptos перестала держать копию: `catalog_controls.rs` делегирует `parse/is/toggle/has/clear` в `rustok_grid::facet_panel` и отдаёт копию через `CatalogFacetLabels::to_grid_labels()`; `core.rs` строит панель `FacetPanel::build` над `GridFacet` (маппинг `catalog_facet_to_grid`) и рендерит её view-модель, а `count_label` стал обёрткой над общей подстановкой. Крейт получил зависимость `rustok-grid`. Публичные имена и маркеры гейтов сохранены, поведение — как у админской панели (ключи без учёта регистра, значения точно). | `crates/modules/rustok-product/storefront/{Cargo.toml,src/catalog_controls.rs,src/core.rs}` |
| Дефект, найденный тестом волны 8 | Устранено | При маппинге ответа владельца терялся флаг усечения владельца: `from_buckets` пересчитывал `is_truncated` только по своему лимиту. Теперь флаг объединяется (`mapped.is_truncated |= facet.is_truncated` / `isTruncated: source.isTruncated || facet.isTruncated`) во всех трёх маппингах — админском, витринном Rust и Next. | `crates/modules/rustok-product/admin/src/facets.rs`, `.../storefront/src/core.rs`, `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts` |
| Верификаторы волны | Добавлено | `verify-product-ui-grid-facets.mjs` — гейт общей библиотеки: форма пакета и его публичный контракт, отсутствие React/Next/fetch внутри, **числовое** совпадение лимитов и списка имён с Rust-оригиналом, обвязка обоих хостов, делегирование вместо копий (TS-склейка и Rust-витрина), и единственная точка подстановки `{count}` на язык. `verify-product-ui-grid-facets.test.mjs` (9/9) исполняет общий пакет через Node type stripping: словарь и round-trip, регистронезависимые ключи, порядок тоггла, лимиты и усечение, модель панели, `applyQueryPairs`, и — отдельным тестом — совпадение ответов продуктовой склейки и общего пакета. Харнесс свипа получил новый шаблон (`*product*` + `*ui-grid*`); вспомогательный резолвер для тестов — `scripts/verify/lib/workspace-package-resolver.mjs`. | `scripts/verify/verify-product-ui-grid-facets.mjs`, `scripts/verify/verify-product-ui-grid-facets.test.mjs`, `scripts/verify/lib/workspace-package-resolver.mjs` |

**Проверки.** Свип — **77 PASS / 45 FAIL**, список падений побайтово тот же, что на `7895d7f`; дельта — новый гейт (baseline пересчитан тем же харнессом с расширенным шаблоном, на `main` файла нет). Точечно: `verify-product-ui-grid-facets.test.mjs` — 9/9, `verify-product-storefront-next-facets{,.test}.mjs` — passed и 7/7 (маркеры переведены на общий пакет), `verify-product-catalog-attribute-filters.mjs` — passed, `verify-product-storefront-boundary.test.mjs` — 12/12, `verify-product-admin-boundary.test.mjs` — 14/14, `verify-ui-i18n-parity.mjs` — по продуктовым каталогам чисто. Впервые проверены типами новые и изменённые TS-файлы: `tsc --noEmit` по общему пакету (4 файла), продуктовой склейке с потребителями и хост-виджету `data-table-faceted-filter.tsx` — **ноль ошибок**. Все изменённые `.rs` проходят настоящую грамматику Rust и структурный сканер (баланс, дубли объявлений, неиспользуемые импорты). Компиляции нет: владельцу — `cargo check -p rustok-product-storefront --features ssr|hydrate`, `cargo check -p rustok-grid`, `cargo test -p rustok-product`, `cargo fmt --all -- --check`.

**Осталось по волне.** Очередь Части IV в прежнем порядке: `PROD-ATTR-STORE-001` (проекция значений атрибутов и таблица характеристик в обеих витринах), кросс-слойный верификатор «владелец → порт → GraphQL/native → витрина» (фасеты и атрибуты), Postgres-тест счётчиков фасетов, три незаленденных контракта (`lifecycle-retry-consumer`, `fallback-mutation-error-safety`, `commerce-product-schema-write-consumer-cutover`), далее P2-находки (`PROD-SHIP-001`, `PROD-DIGITAL-001`, `PROD-TESTS-001`). Отдельным пунктом: счётная панель в Next-админке появится вместе с переводом её списка на `adminProductCatalog` — общий пакет для этого уже готов.
