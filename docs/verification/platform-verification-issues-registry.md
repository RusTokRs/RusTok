# Реестр проблем платформенной верификации

- **Статус:** Исторический и актуализируемый реестр
- **Назначение:** Хранить найденные проблемы, статусы исправления и детальные follow-up notes отдельно от master-checklist.
- **Источник:** Выделено из глобального verification-плана для уменьшения шума в периодических прогонах.

---

## Фаза 21: Реестр найденных проблем

Этот раздел — живой трекер всех `[!]` пунктов из других фаз.  
Добавляем сюда при обнаружении, обновляем при исправлении.

| № | Приоритет | Статус | Описание | Файлы | Фаза |
|---|-----------|--------|----------|-------|------|
| 1 | ?? Критический | ? Исправлено | `content` был помечен `required = true` в `modules.toml`, но `ContentModule::kind()` возвращает `ModuleKind::Optional`. Несоответствие приводило к ошибке `validate_registry_vs_manifest()` при старте. | `modules.toml` | 1.1 |
| 2 | ?? Критический | ? Исправлено | `rustok-blog` и `rustok-forum` используют `event_bus.publish()` вместо `publish_in_tx()` — нарушение атомарности. Все сервисы переведены на `publish_in_tx()` с открытой транзакцией. | `crates/rustok-blog/src/services/post.rs`, `crates/rustok-forum/src/services/{topic,reply,moderation}.rs` | 6.2, 7.3, 7.4 |
| 3 | ?? Высокий | ? Исправлено | `iggy` версия `0.9.2` не существует на crates.io. CI-сборка падала. Исправлено на `0.9.0`. | `Cargo.toml`, `crates/rustok-iggy-connector/Cargo.toml` | 0.6 |
| 4 | ?? Критический | ? Исправлено | Контроллеры `blog/posts.rs`, `forum/topics.rs`, `forum/replies.rs`, `forum/categories.rs`, `pages.rs` использовали только `CurrentUser` без RBAC-проверок. Добавлены RBAC-экстракторы (`RequireBlogPostsCreate`, `RequireForumTopicsCreate`, и т.д.). Добавлена матрица Blog/Forum permissions для всех ролей в `rbac.rs`. | `apps/server/src/controllers/blog/posts.rs`, `forum/topics.rs`, `forum/replies.rs`, `forum/categories.rs`, `pages.rs`, `crates/rustok-core/src/rbac.rs`, `apps/server/src/extractors/rbac.rs` | 4.4, 18.2, 19.2 |
| 5 | ?? Критический | ? Исправлено | `content/nodes.rs` использовал `CurrentUser` без RBAC-проверок для всех 5 endpoints. Заменён на RBAC extractors (`RequireNodesList`, `RequireNodesRead`, `RequireNodesCreate`, `RequireNodesUpdate`, `RequireNodesDelete`). OpenAPI 403 добавлен. | `apps/server/src/controllers/content/nodes.rs` | 4.4, 9.4, 18.2 |
| 6 | ?? Критический | ? Исправлено | `admin_events.rs` (DLQ просмотр/replay) использовал `CurrentUser` без RBAC — доступен любому аутентифицированному пользователю. Заменён на `RequireLogsRead` (Admin/SuperAdmin only). Добавлен `Logs::Read` и `Logs::List` в `ADMIN_PERMISSIONS`. | `apps/server/src/controllers/admin_events.rs`, `crates/rustok-core/src/rbac.rs`, `apps/server/src/extractors/rbac.rs` | 4.4, 9.8, 18.2 |
| 7 | ?? Высокий | ? Исправлено | GraphQL Blog mutations (`create_post`, `update_post`, `delete_post`, `publish_post`, `unpublish_post`, `archive_post`) имели только auth check, но не проверяли конкретные RBAC permissions. Добавлены проверки через `RbacService::has_any_permission()` для каждой операции. | `apps/server/src/graphql/blog/mutation.rs` | 4.3, 8.4 |
| 8 | ?? Критический | ? Исправлено | GraphQL Commerce mutations (`create_product`, `update_product`, `publish_product`, `delete_product`) — без auth/RBAC. Добавлены проверки `RbacService::has_any_permission()` для PRODUCTS_CREATE/UPDATE/DELETE. | `apps/server/src/graphql/commerce/mutation.rs` | 4.3, 8.3 |
| 9 | ?? Критический | ? Исправлено | GraphQL Content mutations (`create_node`, `update_node`, `delete_node`) — только auth check, без RBAC. Добавлены NODES_CREATE/UPDATE/DELETE через `RbacService::has_any_permission()`. Параметр `tenant_id` добавлен. | `apps/server/src/graphql/content/mutation.rs` | 4.3, 8.2 |
| 10 | ?? Высокий | ? Исправлено | GraphQL Forum — stub реализация. Реализованы полноценные queries и mutations через TopicService, ReplyService, CategoryService с RBAC. | `apps/server/src/graphql/forum/mutation.rs`, `query.rs`, `types.rs` | 4.3, 8.5 |
| 11 | ?? Высокий | ? Исправлено | GraphQL Pages mutations — без RBAC, использовали SecurityContext::system(). Добавлены PAGES_CREATE/UPDATE/DELETE через `RbacService::has_any_permission()`. | `apps/server/src/graphql/pages/mutation.rs` | 4.3, 8.7 |
| 12 | ?? Высокий | ? Исправлено | RBAC extractors RequirePagesCreate/Read/Update/Delete использовали NODES_* permissions вместо PAGES_*. Исправлено. Добавлены константы PAGES_* и permissions для Manager/Customer. | `extractors/rbac.rs`, `permissions.rs`, `rbac.rs` | 4.1, 4.4 |
| 13 | ?? Критический | ? Исправлено | REST контроллер `variants.rs`: `create_variant`, `update_variant`, `delete_variant` публиковали события `VariantCreated/Updated/Deleted` через `event_bus_from_context().publish()` **после** коммита транзакции. При сбое между commit и publish событие терялось. Исправлено: все три операции переведены на `publish_in_tx()` внутри транзакции до `commit()`. Update/Delete-операции получили обёртку в транзакцию. | `apps/server/src/controllers/commerce/variants.rs` | 6.2, 19.1 |
| 14 | ?? Высокий | ? Исправлено | Миграция таблицы `sys_events` (outbox pattern) не была зарегистрирована в главном сервере. Создан файл `m20260211_000002_create_sys_events.rs` и добавлен в `apps/server/migration/src/lib.rs`. | `apps/server/migration/src/` | 2.2 |
| 15 | ?? Высокий | ? Исправлено | Rate limiting middleware существовал в `middleware/rate_limit.rs` но **не был подключён** к роутеру. Все auth endpoints были уязвимы к брутфорс-атакам. Исправлено: добавлен `axum_middleware::from_fn` с per-IP sliding window limiter (20 req/60 сек) для `/api/auth/login`, `/api/auth/register`, `/api/auth/reset/*` в `app.rs::after_routes()`. | `apps/server/src/app.rs`, `apps/server/src/middleware/rate_limit.rs` | 9.10, 18.1 |
| 16 | ?? Высокий | ? Исправлено | Entity `product_variant` имел поля `tenant_id, inventory_policy, inventory_management, inventory_quantity, weight_unit, option1/2/3, position`, которых не было в миграции `m20250130_000014_create_commerce_variants`. Добавлена миграция `m20260301_000001_alter_product_variants_add_fields`. | `apps/server/migration/src/m20260301_000001_alter_product_variants_add_fields.rs` | 7.2, 20.5 |
| 17 | ?? Высокий | ? Исправлено | Entity `node` имел поля `deleted_at` и `version`, которых не было в миграции `m20250130_000005_create_nodes`. Добавлена миграция `m20260301_000002_alter_nodes_add_soft_delete`. | `apps/server/migration/src/m20260301_000002_alter_nodes_add_soft_delete.rs` | 7.1, 20.5 |
| 18 | ?? Высокий | ? Исправлено | `ContentIndexer::build_index_content()` и `ProductIndexer::index_one()` были stub-реализациями (return Ok(None/0)). Реализованы с реальными JOIN-запросами к БД и UPSERT в index_content/index_products. | `crates/rustok-index/src/content/indexer.rs`, `crates/rustok-index/src/product/indexer.rs` | 6.3, 7.7 |
| 19 | ?? Низкий | ? Исправлено | `ModuleLifecycleService` использовал hardcoded `CORE_MODULE_SLUGS` массив вместо `registry.is_core()`. При добавлении нового Core-модуля нужно было обновлять два места. Исправлено: `validate_core_toggle()` удалён, проверка инлайнена через `registry.is_core()`. Тесты обновлены. | `apps/server/src/services/module_lifecycle.rs` | 5.4 |
| 20 | ?? Высокий | ? Исправлено | `ProductIndexer::reindex_all()` содержал SQL-запрос `WHERE tenant_id = $1 AND deleted_at IS NULL` — но таблица `products` не имеет поля `deleted_at` (продукты удаляются через hard delete). Убрана несуществующая колонка из WHERE. | `crates/rustok-index/src/product/indexer.rs` | 7.7, 20.5 |
| 21 | ?? Низкий | ? Исправлено | `ProductIndexer::build_index_product()` использовал небезопасный `as i32` cast для `agg.total_inventory` и `agg.variant_count` (тип `i64`). Исправлено: используется `i32::try_from().unwrap_or(i32::MAX)`. | `crates/rustok-index/src/product/indexer.rs` | 20.1 |
| 22 | ?? Высокий | ? Исправлено | `crates/rustok-tenant` содержал пустые stub-файлы (entities/mod.rs, services/mod.rs, dto/mod.rs — только комментарии). Реализованы SeaORM entities (`tenant.rs`, `tenant_module.rs`), DTOs (`CreateTenantInput`, `UpdateTenantInput`, `TenantResponse`, `ToggleModuleInput`) и `TenantService` с полным CRUD и toggle_module. | `crates/rustok-tenant/src/` | 7.9 |
| 23 | ?? Высокий | ? Исправлено | `controllers/swagger.rs` содержал только `ApiDoc` struct без endpoint для отдачи OpenAPI spec. Добавлены handlers `openapi_json()` и `openapi_yaml()` для эндпоинтов `GET /api/openapi.json` и `GET /api/openapi.yaml`, маршруты зарегистрированы в `app.rs`. | `apps/server/src/controllers/swagger.rs`, `apps/server/src/app.rs` | 9.9 |
| 24 | ?? Высокий | ? Исправлено | Миграции `ScriptsMigration` и `ScriptExecutionsMigration` из `alloy-scripting` не были включены в главный Migrator — таблицы `scripts` и `script_executions` не создавались при деплое. Добавлены wrapper-файлы `m20260302_000001_create_scripts.rs` и `m20260302_000002_create_script_executions.rs`, подключена зависимость `alloy-scripting` в `migration/Cargo.toml`. | `apps/server/migration/src/lib.rs`, `apps/server/migration/src/m20260302_*.rs`, `apps/server/migration/Cargo.toml` | 7.6, 0.3 |
| 25 | ?? Высокий | ? Исправлено | Миграция `search_index` существовала в `apps/server/migration/src/` (использует `PgSearchEngine`) но не была зарегистрирована в Migrator. Также конфликт имён модулей (`m20250130_000010` дважды). Файл переименован в `m20250130_000010a_create_search_index.rs`, добавлен в Migrator. | `apps/server/migration/src/lib.rs`, `apps/server/migration/src/m20250130_000010a_create_search_index.rs` | 0.3, 7.7 |
| 26 | ?? Средний | ? Исправлено | Commerce DTOs (`CreateProductInput`, `UpdateProductInput`, `CreateVariantInput`, `UpdateVariantInput`, `PriceInput`) не имели `#[derive(Validate)]` и вызовы валидации в сервисах. Добавлены: `validator` в `rustok-commerce/Cargo.toml`, `#[derive(Validate)]` с аннотациями полей в DTO, вызов `.validate()?` в `CatalogService::create_product()` и `update_product()`. Также добавлены вызовы валидации в `NodeService` для `create_node_in_tx` и `update_node_in_tx`. | `crates/rustok-commerce/src/dto/product.rs`, `dto/variant.rs`, `services/catalog.rs`, `crates/rustok-content/src/services/node_service.rs`, `crates/rustok-commerce/Cargo.toml` | 18.4, 19.7 |
| 27 | ?? Средний | ? Исправлено | Observability/Grafana конфигурация содержала дрейф от реального стека: datasources не имели стабильных `uid`, а Jaeger datasource ссылался на несуществующий Loki (`tracesToLogs.datasourceUid = loki`). Это ломало привязку dashboard > datasource и вводило в заблуждение операторов. Исправлено: добавлены `uid: prometheus` и `uid: jaeger`, удалена битая `tracesToLogs` ссылка, а в Compose-файлах убран устаревший ключ `version`, чтобы `docker compose config` проходил без предупреждений. | `grafana/datasources/datasources.yml`, `docker-compose.full-dev.yml`, `docker-compose.observability.yml`, `docs/verification/PLATFORM_VERIFICATION_PLAN.md` | 0.5, 15.4 |

### 21.1 Детали: Проблема #2 — Небезопасная публикация событий в blog/forum

**Корневая причина:**  
`PostService` и `TopicService`/`ReplyService`/`ModerationService` принимают `TransactionalEventBus` и передают его в `NodeService` (который корректно использует `publish_in_tx()`). Но затем сами дополнительно вызывают `self.event_bus.publish()` для публикации модуль-специфичных событий (`BlogPostCreated`, `ForumTopicCreated`, etc.) — это происходит **вне транзакции**.

**Риск:**
1. `NodeService` выполняет операцию + `publish_in_tx()` в транзакции — всё атомарно.
2. `PostService.create_post()` вызывает `NodeService.create_node()` (успешно).
3. Затем вызывает `self.event_bus.publish(BlogPostCreated{...})` — это отдельная операция.
4. Если шаг 3 фейлится — основные данные уже в БД, но blog-специфичное событие потеряно.

**Рекомендуемое исправление:**
- Рефакторинг: вместо делегирования в NodeService с последующим отдельным publish — 
  использовать паттерн открытой транзакции: создать транзакцию в `PostService`, передать её в NodeService и в последующий `publish_in_tx()`.
- Или: убрать дублирующие события в blog/forum — NodeService уже публикует `NodeCreated`/`NodeUpdated`/etc., а IndexService может слушать их напрямую.

**Чеклист исправления:**
- [x] Рефакторинг `PostService::create_post()` > `publish_in_tx()`
- [x] Рефакторинг `PostService::update_post()` > `publish_in_tx()`
- [x] Рефакторинг `PostService::publish_post()` > `publish_in_tx()`
- [x] Рефакторинг `PostService::unpublish_post()` > `publish_in_tx()`
- [x] Рефакторинг `PostService::delete_post()` > `publish_in_tx()`
- [x] Рефакторинг `TopicService` > `publish_in_tx()`
- [x] Рефакторинг `ReplyService::create_reply()` > `publish_in_tx()`
- [x] Рефакторинг `ModerationService` (3 вызова) > `publish_in_tx()`
- [x] Добавить integration тест: проверить что BlogPostCreated публикуется атомарно
  - Добавлены тесты в `crates/rustok-blog/tests/integration.rs`: happy-path для `MemoryTransport` и failure-path для не-transactional/failing transport (событие не наблюдается при ошибке публикации)

---

## Прогон platform-core-integrity — 2026-03-26

**Branch:** `claude/add-verification-plan-DLNgT`
**Тип проверок:** только Static (🔧); Runtime (🌐) пропущены — PostgreSQL/server не запущены в данной среде.

### Итоги по фазам

| Фаза | Статус | Примечания |
|------|--------|------------|
| 0 Prerequisites (static) | ✅ | docker compose config — не выполнялся в данной среде |
| 1 Состав ядра | ✅ | `rustok-search` отсутствовал в списке — добавлен |
| 2 Инварианты ядра | ✅ | `git grep` нашёл 0 нарушений изоляции импортов |
| 3 Boot без optional (static) | ✅ | `cargo build --no-default-features` проходит |
| 4 Auth в изоляции (static) | ✅ | Тесты 14/14; runtime-проверки пропущены |
| 5 Multi-tenancy (static) | ✅ | Тесты 1/1; runtime-проверки пропущены |
| 6 Admin-панели (static) | ✅ | `cargo build -p rustok-admin` ✅; `npm run build` ✅ (после Ф31); `npm run lint` ✅ (после Ф32) |
| 7 i18n (static) | ✅ | `next-intl` + `createNextIntlPlugin`, локали en/ru, messages/*.json |
| 8 UI core модулей (static) | ⚠️ | auth/users/oauth/modules/tenant — есть; roles/email/cache — ⚠️ WIP (Ф33) |
| 9 GraphQL (static) | ✅ | Schema компилируется без паники |
| Тесты core crates | ✅ | rustok-core 252/252, rustok-auth 14/14, rustok-rbac 45/45, rustok-tenant 1/1, rustok-outbox 1/1 |

### Найденные расхождения (Ф28–Ф30)

| № | Приоритет | Статус | Описание | Файлы |
|---|-----------|--------|----------|-------|
| 28 | Средний | Исправлено | `rustok-search` зарегистрирован как Core-модуль (`required = true` в `modules.toml`, `SearchModule` в `build_registry()`), но отсутствовал в списке core crates в плане верификации (секция 1.2). | `docs/verification/platform-core-integrity-verification-plan.md` |
| 29 | Средний | Исправлено | `cargo build -p rustok-server` без предварительной сборки `apps/admin/dist` завершается ошибкой (`no function named 'get' found for struct 'AdminAssets'`): default feature `embed-admin` использует `RustEmbed` с `#[folder = "../../apps/admin/dist"]`. Исправлено: создан `apps/admin/dist/.gitkeep`, добавлено исключение в `.gitignore`. | `apps/server/src/services/app_router.rs`, `apps/server/Cargo.toml`, `apps/admin/dist/.gitkeep`, `.gitignore` |
| 30 | Низкий | Исправлено | Команда `npm run typecheck` в плане верификации (секция 10.1) не существует в `apps/next-admin/package.json`. Исправлено: заменено на `npx tsc --noEmit` в плане. | `apps/next-admin/package.json`, `docs/verification/platform-core-integrity-verification-plan.md` |
| 31 | Высокий | Исправлено | `apps/next-admin`: `npm run build` падал из-за зависимости `next/font/google` от сети — Node.js не мог резолвить DNS в среде верификации. Исправлено: все Google Fonts заменены на `next/font/local` с пакетами `@fontsource-variable/*`, `@fontsource/*` и `geist` (npm). Сборка теперь работает полностью offline. | `apps/next-admin/src/shared/lib/themes/font.config.ts`, `apps/next-admin/package.json` |
| 32 | Средний | Исправлено | `apps/next-admin/eslint.config.mjs` содержал пустой конфиг (`export default []`) — `npm run lint` завершался с кодом 0, не проверяя ничего. Исправлено: настроен `@next/eslint-plugin-next` с `core-web-vitals` flat config. Устаревший `.eslintignore` удалён, `ignores` перенесены в конфиг. | `apps/next-admin/eslint.config.mjs`, `apps/next-admin/.eslintignore` |
| 33 | Низкий | Открыто | UI для RBAC (roles, permissions), email settings и cache management отсутствует в обеих admin-панелях. Соответствует отметке ⚠️ WIP в плане (фаза 8). Не блокирует сборку. Подлежит закрытию по мере реализации UI. | `apps/admin/src/`, `apps/next-admin/src/` |

